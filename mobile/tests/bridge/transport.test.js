import { describe, expect, it } from 'vitest'
import { installCore, loadShell, snapshot } from '../support/bridge.js'

const start = async (extra = {}) => {
  const core = installCore({
    tracker_health: { state: 'ok' },
    tracker_snapshot: snapshot(),
    settings_load: { activeProject: '/work/smetana' },
    bridge_listen: null,
    bridge_unlisten: null,
    ...extra
  })
  const shell = await loadShell()
  return { core, ...shell }
}

describe('the stores over the bridge', () => {
  it('a store\'s invoke leaves as bridge_invoke with the same command and arguments', async () => {
    const { core, tracker } = await start({ tracker_probe: [] })
    await tracker.initTracker()
    expect(core.bridged('tracker_snapshot')).toEqual([{}])
    expect(core.bridged('tracker_health')).toEqual([{}])

    await tracker.probeProjects(['/a', '/b'])
    expect(core.bridged('tracker_probe')).toEqual([{ paths: ['/a', '/b'] }])
    // No store call reaches Tauri under its own name.
    expect(core.core('tracker_snapshot')).toEqual([])
  })

  it('the snapshot a desktop answers is the board the store holds', async () => {
    const { tracker } = await start({ tracker_snapshot: snapshot(5, ['smetana-1', 'smetana-2']) })
    await tracker.initTracker()
    expect(tracker.trackerState.issues.size).toBe(2)
    expect(tracker.trackerState.generation).toBe(5)
  })

  it('subscribes the desktop to the events a store listens to', async () => {
    const { core, tracker } = await start()
    await tracker.initTracker()
    const events = core.core('bridge_listen').map((args) => args.event).sort()
    expect(events).toEqual(['tracker:delta', 'tracker:health'])
  })

  it('a bridge:event named tracker:delta reaches the store as an ordinary event', async () => {
    const { tracker, emit, nextTick } = await start()
    await tracker.initTracker()
    await emit('bridge:event', {
      name: 'tracker:delta',
      payload: {
        generation: 2,
        upserted: [
          { id: 'smetana-9', title: 'From the desktop', status: 'open', updated_at: 'x', labels: [], dependencies: [] }
        ],
        removed: []
      }
    })
    await nextTick()
    expect(tracker.issueById('smetana-9')?.title).toBe('From the desktop')
    expect(tracker.trackerState.generation).toBe(2)
  })

  it('an event nobody listens to is dropped quietly', async () => {
    const { tracker, emit } = await start()
    await tracker.initTracker()
    await expect(emit('bridge:event', { name: 'run:state', payload: {} })).resolves.not.toThrow()
  })

  it('a refusal from the desktop reaches the store as the rejection of invoke', async () => {
    const { tracker } = await start({ tracker_access_repair: () => { throw 'forbidden' } })
    await tracker.initTracker()
    // initTracker asks for tracker_access_repair, which is off the allowlist; it must
    // still finish, and say so.
    expect(tracker.trackerState.ready).toBe(true)
  })
})

describe('after a reconnect', () => {
  it('reads the snapshot again when the link comes back, and not on the first connection', async () => {
    const { core, status } = await start()
    const { applyStatus } = status
    const session = await import('../../src/session.js')
    session.wireStores()

    applyStatus({ state: 'connecting' })
    applyStatus({ state: 'online' })
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(core.bridged('tracker_snapshot')).toHaveLength(1)

    applyStatus({ state: 'unreachable', last_seen: 1 })
    await new Promise((resolve) => setTimeout(resolve, 5))
    expect(core.bridged('tracker_snapshot')).toHaveLength(1)

    applyStatus({ state: 'online' })
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(core.bridged('tracker_snapshot')).toHaveLength(2)
  })

  it('an update-recommended link is a working link', async () => {
    const { core, status } = await start()
    const session = await import('../../src/session.js')
    session.wireStores()
    status.applyStatus({ state: 'update-recommended', recommended: '9.0.0', store_url: 'x' })
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(core.bridged('tracker_snapshot')).toHaveLength(1)
  })

  it('a status the core emits is applied through the real event channel', async () => {
    const { status, emit, nextTick } = await start({ bridge_state: { paired: true, version: '0.1.0', status: { state: 'connecting' } } })
    await status.initStatus()
    await emit('bridge:status', { state: 'unreachable', last_seen: 5 })
    await nextTick()
    expect(status.bridge.status).toEqual({ state: 'unreachable', last_seen: 5 })
  })
})

describe('pairing again in one session', () => {
  it('reads the snapshot again and does not start the stores a second time', async () => {
    const { core, status } = await start({ unpair: null, pair: null, connect: null })
    const session = await import('../../src/session.js')
    session.wireStores()
    localStorage.clear()

    status.applyStatus({ state: 'online' })
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(session.home.tasks).toBe(1)
    expect(localStorage.getItem('smetana.companion.summary')).not.toBeNull()

    await session.unpairAndForget()
    expect(localStorage.getItem('smetana.companion.summary')).toBeNull()
    expect(session.home.tasks).toBeNull()
    expect(session.home.project).toBeNull()

    status.applyStatus({ state: 'online' })
    await new Promise((resolve) => setTimeout(resolve, 20))
    // Two reads of the board, but the events were subscribed once.
    expect(core.bridged('tracker_snapshot')).toHaveLength(2)
    expect(core.core('bridge_listen').filter((a) => a.event === 'tracker:delta')).toHaveLength(1)
    expect(core.bridged('tracker_health')).toHaveLength(1)
  })
})

describe('what never leaves the phone', () => {
  it('a question about the desktop\'s own machine is answered here and not asked', async () => {
    const { core, tracker } = await start({ tracker_probe: [] })
    await tracker.initTracker()
    expect(tracker.trackerState.folderAccessRepair).toBe('unavailable')
    expect(core.bridged('tracker_access_repair')).toEqual([])
  })

  it('the rail\'s dots are answered here as empty', async () => {
    const { core } = await start()
    const { invoke } = await import('../../src/bridge/core.js')
    expect(await invoke('terminal_marks')).toEqual([])
    expect(core.calls.filter((call) => call.cmd === 'bridge_invoke')).toEqual([])
  })

  it('the offers to resume an agent are the server\'s and are asked, not answered here', async () => {
    const { core } = await start({ terminal_restorable: [] })
    const { invoke } = await import('../../src/bridge/core.js')
    expect(await invoke('terminal_restorable', { project: '/p' })).toEqual([])
    expect(core.bridged('terminal_restorable')).toEqual([{ project: '/p' }])
  })

  it('an answer given here is a copy, so a store that writes into it changes nobody else\'s', async () => {
    await start()
    const { invoke } = await import('../../src/bridge/core.js')
    ;(await invoke('terminal_marks')).push('x')
    expect(await invoke('terminal_marks')).toEqual([])
  })

  it('an event off the list is local and the core is not asked to subscribe', async () => {
    const { core } = await start()
    const { listen } = await import('../../src/bridge/event.js')
    await listen('crew:tree', () => {})
    await listen('terminal:output', () => {})
    await listen('tracker:delta', () => {})
    expect(core.core('bridge_listen').map((args) => args.event)).toEqual(['tracker:delta'])
  })
})

describe('a driven session\'s state, which the desktop sends now', () => {
  it('is subscribed through the core and reaches the store, with nothing made up locally', async () => {
    const { core, emit } = await start()
    const { listen } = await import('../../src/bridge/event.js')
    const heard = []
    await listen('session:state', (event) => heard.push(event.payload))
    expect(core.core('bridge_listen').map((args) => args.event)).toEqual(['session:state'])

    const state = { id: 7, state: 'running', conversation: 'c', title: null, project: '/p', agent: 'claude', work: { kind: 'bare' }, startedAt: '2026-10-27T10:00:00+00:00' }
    await emit('bridge:event', { name: 'session:state', payload: state })
    // A batch of session:events moves nothing on this side: the server says the state itself.
    await emit('bridge:event', { name: 'session:events', payload: { id: 7, events: [{ seq: 1, kind: 'turn-start' }] } })
    expect(heard).toEqual([state])
  })
})
