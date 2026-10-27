import { describe, expect, it } from 'vitest'
import { installCore, loadShell } from './support/bridge.js'
import { promotedNote } from '../../src/components/run/promotedNote.js'

/* The sheet's writes through the real stores and the real bridge modules, with the Rust core as
   the only thing replaced. */

const issue = (id, extra = {}) => ({
  id,
  title: `Task ${id}`,
  status: 'open',
  updated_at: '2026-10-15T10:00:00Z',
  labels: [],
  dependencies: [],
  ...extra
})
const board = (issues) => ({
  generation: 1,
  columns: [
    { name: 'open', category: 'active' },
    { name: 'closed', category: 'done' }
  ],
  issues
})

const deferred = () => {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

const start = async (issues, extra = {}) => {
  const core = installCore({
    tracker_health: { state: 'ok' },
    tracker_snapshot: board(issues),
    tracker_update: (args) => ({ ...issues.find((one) => one.id === args.id), ...args.patch }),
    tracker_delete: {},
    session_start: 9,
    session_attach: { events: [], seq: 0, state: 'running', cwd: '/work/a', agent: 'claude' },
    bridge_listen: null,
    bridge_unlisten: null,
    ...extra
  })
  const shell = await loadShell()
  const actions = await import('../src/taskActions.js')
  await shell.tracker.initTracker()
  return { core, actions, tracker: shell.tracker }
}
const sent = (core, cmd) => core.bridged(cmd)

describe('the refusal as the sheet shows it', () => {
  it('is the tracker\'s title and description in one line, and nothing when there is none', async () => {
    const { actions, tracker } = await start([issue('x-1')])
    expect(actions.refusal()).toBe('')
    tracker.trackerState.lastError = { title: 'Could not save to the tracker', description: 'Nothing was written.' }
    expect(actions.refusal()).toBe('Could not save to the tracker. Nothing was written.')
  })
})

describe('writing a status', () => {
  it('goes through tracker_update with a promoted note on a promote to ready', async () => {
    const { core, actions } = await start([issue('x-1', { status: 'deferred' })])
    expect(await actions.setStatus('x-1', 'open')).toBe(true)
    expect(sent(core, 'tracker_update')).toEqual([
      { id: 'x-1', patch: { status: 'open', append_notes: promotedNote('status') } }
    ])
    expect(actions.writingIds.size).toBe(0)
  })

  it('sends no note for a move to done', async () => {
    const { core, actions } = await start([issue('x-1')])
    await actions.setStatus('x-1', 'closed')
    expect(sent(core, 'tracker_update')[0].patch).toEqual({ status: 'closed' })
  })

  it('keeps the card busy while the write is out', async () => {
    const gate = deferred()
    const { actions } = await start([issue('x-1')], { tracker_update: () => gate.promise })
    const done = actions.setStatus('x-1', 'closed')
    expect(actions.writingIds.has('x-1')).toBe(true)
    gate.resolve(issue('x-1', { status: 'closed' }))
    await done
    expect(actions.writingIds.has('x-1')).toBe(false)
  })

  it('answers false and leaves the message in the tracker state when bd refuses', async () => {
    const { actions, tracker } = await start([issue('x-1')], {
      tracker_update: () => {
        throw 'bd said no'
      }
    })
    expect(await actions.setStatus('x-1', 'closed')).toBe(false)
    expect(tracker.trackerState.lastError?.title).toBe('Could not save to the tracker')
    expect(tracker.trackerState.issues.get('x-1').status).toBe('open')
    expect(actions.writingIds.size).toBe(0)
  })
})

describe('locking', () => {
  const epic = [
    issue('e-1'),
    issue('e-1.1', { parent: 'e-1' }),
    issue('e-1.2', { parent: 'e-1', status: 'in_progress' }),
    issue('e-1.1.1', { parent: 'e-1.1' })
  ]

  it('locks an epic and its open descendants in lockIds order, greying them all first', async () => {
    const gates = []
    const { core, actions } = await start(epic, {
      tracker_update: (args) => {
        const gate = deferred()
        gates.push({ gate, args })
        return gate.promise
      }
    })
    const done = actions.toggleLock('e-1')
    expect([...actions.writingIds].sort()).toEqual(['e-1', 'e-1.1', 'e-1.1.1'])
    for (let i = 0; i < 3; i++) {
      await new Promise((resolve) => setTimeout(resolve, 0))
      expect(gates).toHaveLength(i + 1)
      gates[i].gate.resolve(issue(gates[i].args.id, { status: 'blocked' }))
    }
    expect(await done).toBe(true)
    expect(sent(core, 'tracker_update').map((call) => call.id)).toEqual(['e-1.1.1', 'e-1.1', 'e-1'])
    expect(sent(core, 'tracker_update').every((call) => call.patch.status === 'blocked')).toBe(true)
    expect(actions.writingIds.size).toBe(0)
  })

  it('unlocks the epic first and its locked descendants after, with the unblock note', async () => {
    const locked = epic.map((one) => (one.status === 'open' ? { ...one, status: 'blocked' } : one))
    const { core, actions } = await start(locked)
    await actions.toggleLock('e-1')
    expect(sent(core, 'tracker_update').map((call) => call.id)).toEqual(['e-1', 'e-1.1', 'e-1.1.1'])
    expect(sent(core, 'tracker_update')[0].patch).toEqual({ status: 'open', append_notes: promotedNote('unblock') })
  })

  it('stops the cascade on the first refusal and clears writingIds', async () => {
    let calls = 0
    const { core, actions } = await start(epic, {
      tracker_update: (args) => {
        calls += 1
        if (calls === 2) throw 'refused'
        return issue(args.id, { status: 'blocked' })
      }
    })
    expect(await actions.toggleLock('e-1')).toBe(false)
    expect(sent(core, 'tracker_update')).toHaveLength(2)
    expect(actions.writingIds.size).toBe(0)
  })

  it('does nothing for a card that is neither ready nor locked', async () => {
    const { core, actions } = await start(epic)
    expect(await actions.toggleLock('e-1.2')).toBe(false)
    expect(sent(core, 'tracker_update')).toEqual([])
  })
})

describe('deleting', () => {
  it('goes through tracker_delete and takes the card off the board', async () => {
    const { core, actions, tracker } = await start([issue('x-1'), issue('x-2')])
    expect(await actions.remove('x-1')).toBe(true)
    expect(sent(core, 'tracker_delete')).toEqual([{ id: 'x-1' }])
    expect(tracker.trackerState.issues.has('x-1')).toBe(false)
  })

  it('rolls the card back when bd refuses', async () => {
    const { actions, tracker } = await start([issue('x-1')], {
      tracker_delete: () => {
        throw 'refused'
      }
    })
    expect(await actions.remove('x-1')).toBe(false)
    expect(tracker.trackerState.issues.has('x-1')).toBe(true)
    expect(tracker.trackerState.lastError?.title).toBe('Could not save to the tracker')
  })
})

describe('an issue that is no longer on the board', () => {
  it('does nothing for any action', async () => {
    const { core, actions } = await start([issue('x-1')])
    expect(await actions.setStatus('gone', 'closed')).toBe(false)
    expect(await actions.toggleLock('gone')).toBe(false)
    expect(await actions.remove('gone')).toBe(false)
    expect(await actions.startOn('/work/a', 'editTask', 'gone')).toBeNull()
    expect(core.calls.filter((call) => call.cmd === 'bridge_invoke').map((call) => call.args.cmd)).not.toEqual(
      expect.arrayContaining(['tracker_update', 'tracker_delete', 'session_start'])
    )
  })
})

describe('starting an agent on the issue', () => {
  it('sends the desktop intent for Edit, Answer questions and Fix this', async () => {
    const { core, actions } = await start([issue('x-1', { title: 'Fix the board' })])
    for (const kind of ['editTask', 'resolveTask', 'fixTask']) {
      expect(await actions.startOn('/work/a', kind, 'x-1')).toBe(9)
    }
    expect(sent(core, 'session_start')).toEqual(
      ['editTask', 'resolveTask', 'fixTask'].map((kind) => ({
        project: '/work/a',
        intent: { kind, id: 'x-1', title: 'Fix the board' }
      }))
    )
  })
})
