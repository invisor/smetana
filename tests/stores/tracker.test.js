import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'
import { delta, edge, issue, snapshot } from '../support/fixtures.js'

let ipc
let emit
let tracker

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  emit = loaded.emit
  tracker = loaded.stores.tracker
})

/* Start the tracker with a given snapshot. Returns once initTracker has run to
   completion. */
const start = async (snap = snapshot()) => {
  ipc.on('tracker_health', { state: 'ok' })
  ipc.on('tracker_snapshot', snap)
  await tracker.initTracker()
}

describe('status translation', () => {
  it('three bd statuses turn into design system statuses', () => {
    expect(tracker.toUiStatus('open')).toBe('ready')
    expect(tracker.toUiStatus('in_progress')).toBe('running')
    expect(tracker.toUiStatus('closed')).toBe('done')
  })

  it('everything else passes through — and that is the intent', () => {
    expect(tracker.toUiStatus('blocked')).toBe('blocked')
    expect(tracker.toUiStatus('awaiting-review')).toBe('awaiting-review')
  })
})

describe('boardColumns', () => {
  it('lays the issues out into the snapshot\'s columns', async () => {
    await start(
      snapshot({
        issues: [issue({ id: 'bd-1' }), issue({ id: 'bd-2', status: 'in_progress' })]
      })
    )

    const board = tracker.boardColumns.value

    expect(board.map((column) => column.status)).toEqual(['ready', 'running', 'done'])
    expect(board[0].tasks.map((task) => task.id)).toEqual(['bd-1'])
    expect(board[1].tasks.map((task) => task.id)).toEqual(['bd-2'])
  })

  it('a status absent from bd\'s set gets its own column rather than vanishing', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-9', status: 'awaiting-review' })] }))

    const board = tracker.boardColumns.value

    expect(board.map((column) => column.status)).toContain('awaiting-review')
  })

  /* The card draws the type where it used to draw the status, so bd's own word
     has to survive the trip untranslated — a custom type would otherwise reach
     the board as undefined and the badge would simply not be there. */
  it('carries bd\'s type through to the card, custom ones included', async () => {
    await start(
      snapshot({
        issues: [issue({ id: 'bd-1', issue_type: 'bug' }), issue({ id: 'bd-2', issue_type: 'tech-debt' })]
      })
    )

    const tasks = tracker.boardColumns.value.flatMap((column) => column.tasks)

    expect(tasks.find((t) => t.id === 'bd-1').type).toBe('bug')
    expect(tasks.find((t) => t.id === 'bd-2').type).toBe('tech-debt')
  })

  it('an issue bd never typed leaves the card with nothing to draw', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1', issue_type: null })] }))

    expect(tracker.boardColumns.value.flatMap((c) => c.tasks)[0].type).toBeUndefined()
  })

  it('counts blockers from the edges on both sides', async () => {
    await start(
      snapshot({
        issues: [
          issue({ id: 'bd-1' }),
          issue({ id: 'bd-2', dependencies: [edge({ issue_id: 'bd-2', depends_on_id: 'bd-1' })] })
        ]
      })
    )

    const tasks = tracker.boardColumns.value.flatMap((column) => column.tasks)
    const first = tasks.find((task) => task.id === 'bd-1')
    const second = tasks.find((task) => task.id === 'bd-2')

    expect(second.blockedBy).toBe(1)
    expect(first.blocks).toBe(1)
    expect(first.blockedBy).toBe(0)
  })

  it('parentage does not count as a blocker: otherwise every child would get a false "blocked"', async () => {
    await start(
      snapshot({
        issues: [
          issue({
            id: 'bd-2',
            parent: 'bd-1',
            dependencies: [edge({ issue_id: 'bd-2', depends_on_id: 'bd-1', type: 'parent-child' })]
          })
        ]
      })
    )

    const task = tracker.boardColumns.value.flatMap((column) => column.tasks)[0]

    expect(task.blockedBy).toBe(0)
    expect(task.spawnedFrom).toBe('bd-1')
  })
})

describe('deltas', () => {
  it('add and remove issues and move the generation', async () => {
    await start(snapshot({ generation: 5, issues: [issue({ id: 'bd-1' })] }))

    await emit('tracker:delta', delta({
      generation: 6,
      upserted: [issue({ id: 'bd-2' })],
      removed: ['bd-1']
    }))

    expect(tracker.trackerState.issues.has('bd-1')).toBe(false)
    expect(tracker.trackerState.issues.has('bd-2')).toBe(true)
    expect(tracker.trackerState.generation).toBe(6)
  })

  it('a gap in the generation means a lost event — the board is taken in full through resync', async () => {
    await start(snapshot({ generation: 5, issues: [issue({ id: 'bd-1' })] }))
    ipc.on('tracker_resync', snapshot({ generation: 9, issues: [issue({ id: 'bd-9' })] }))

    await emit('tracker:delta', delta({ generation: 8, upserted: [issue({ id: 'bd-8' })] }))

    await vi.waitFor(() => expect(ipc.calls('tracker_resync')).toHaveLength(1))
    /* The snapshot replaces the state in full: the former issue is gone, the one it brought is there. */
    expect(tracker.trackerState.generation).toBe(9)
    expect(tracker.trackerState.issues.has('bd-9')).toBe(true)
    expect(tracker.trackerState.issues.has('bd-1')).toBe(false)
    /* A delta with a gap is not applied at all — its issue never reached the board. */
    expect(tracker.trackerState.issues.has('bd-8')).toBe(false)
  })

  it('during a move deltas are ignored: they may be about the old folder', async () => {
    await start(snapshot({ generation: 5 }))
    tracker.trackerState.switching = true

    await emit('tracker:delta', delta({ generation: 6, upserted: [issue({ id: 'bd-6' })] }))

    expect(tracker.trackerState.issues.has('bd-6')).toBe(false)
    expect(tracker.trackerState.generation).toBe(5)
  })

  it('a snapshot that went stale in flight is not rolled out', async () => {
    ipc.on('tracker_health', { state: 'ok' })
    /* While the answer to tracker_snapshot flies back, the watcher manages to
       send a delta and advance the generation. The snapshot is the past by
       then. */
    ipc.on('tracker_snapshot', async () => {
      await emit('tracker:delta', delta({ generation: 9, upserted: [issue({ id: 'bd-9' })] }))
      return snapshot({ generation: 5, issues: [issue({ id: 'bd-old' })] })
    })

    await tracker.initTracker()

    expect(tracker.trackerState.generation).toBe(9)
    expect(tracker.trackerState.issues.has('bd-9')).toBe(true)
    expect(tracker.trackerState.issues.has('bd-old')).toBe(false)
  })
})

describe('resync and switching project', () => {
  it('resync replaces the state in full rather than adding to it', async () => {
    await start(snapshot({ generation: 5, issues: [issue({ id: 'bd-1' })] }))
    ipc.on('tracker_resync', snapshot({ generation: 7, issues: [issue({ id: 'bd-2' })] }))

    await tracker.resync()

    expect(tracker.trackerState.issues.has('bd-1')).toBe(false)
    expect(tracker.trackerState.generation).toBe(7)
  })

  it('a failed resync leaves the board as it was and remembers the read error', async () => {
    await start(snapshot({ generation: 5, issues: [issue({ id: 'bd-1' })] }))
    ipc.fail('tracker_resync', new Error('bd did not start'))

    await tracker.resync()

    expect(tracker.trackerState.issues.has('bd-1')).toBe(true)
    expect(tracker.trackerState.lastError.title).toBe('Could not read the tracker')
  })

  it('setProject clears the switching flag even after a refusal', async () => {
    await start()
    ipc.fail('tracker_set_project', new Error('no such folder'))

    await tracker.setProject('/another')

    expect(tracker.trackerState.switching).toBe(false)
    expect(tracker.trackerState.lastError.title).toBe('Could not read the tracker')
  })
})

describe('writes', () => {
  it('the optimistic value shows at once, before the back end answers', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1', title: 'the old one' })] }))
    ipc.on('tracker_update', () => issue({ id: 'bd-1', title: 'the new one' }))

    const pending = tracker.updateIssue('bd-1', { title: 'the new one' })
    expect(tracker.trackerState.issues.get('bd-1').title).toBe('the new one')

    await pending
    expect(tracker.trackerState.issues.get('bd-1').title).toBe('the new one')
  })

  it('a refusal rolls the edit back if nobody touched it', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1', title: 'the old one' })] }))
    ipc.fail('tracker_update', new Error('bd failed'))

    await expect(tracker.updateIssue('bd-1', { title: 'the new one' })).rejects.toThrow()

    expect(tracker.trackerState.issues.get('bd-1').title).toBe('the old one')
    expect(tracker.trackerState.lastError.title).toBe('Could not save to the tracker')
  })

  it('a refusal does not roll back if somebody else changed the value in flight', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1', title: 'the old one' })] }))
    ipc.on('tracker_update', () => {
      /* The watcher brought somebody else's edit while our write was in flight. */
      tracker.trackerState.issues.set('bd-1', issue({ id: 'bd-1', title: "somebody else's" }))
      throw new Error('bd failed')
    })

    await expect(tracker.updateIssue('bd-1', { title: 'the new one' })).rejects.toThrow()

    expect(tracker.trackerState.issues.get('bd-1').title).toBe("somebody else's")
  })

  it('close and reopen send their commands and move the status optimistically', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1' })] }))
    ipc.on('tracker_close', () => issue({ id: 'bd-1', status: 'closed' }))
    ipc.on('tracker_reopen', () => issue({ id: 'bd-1', status: 'open' }))

    await tracker.closeIssue('bd-1', 'done')
    expect(ipc.calls('tracker_close')).toEqual([{ id: 'bd-1', reason: 'done' }])
    expect(tracker.trackerState.issues.get('bd-1').status).toBe('closed')

    await tracker.reopenIssue('bd-1')
    expect(tracker.trackerState.issues.get('bd-1').status).toBe('open')
  })

  it('a deletion takes the card off the board before bd answers', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1' }), issue({ id: 'bd-2' })] }))
    ipc.on('tracker_delete', () => null)

    const pending = tracker.deleteIssue('bd-1')
    expect(tracker.trackerState.issues.has('bd-1')).toBe(false)

    await pending
    expect(ipc.calls('tracker_delete')).toEqual([{ id: 'bd-1' }])
    expect([...tracker.trackerState.issues.keys()]).toEqual(['bd-2'])
  })

  it('a refused deletion puts the card back', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1', title: 'still here' })] }))
    ipc.fail('tracker_delete', new Error('bd failed'))

    await expect(tracker.deleteIssue('bd-1')).rejects.toThrow()

    expect(tracker.trackerState.issues.get('bd-1').title).toBe('still here')
    expect(tracker.trackerState.lastError.title).toBe('Could not save to the tracker')
  })

  /* The restore has to lose to anything that arrived while bd was working: the
     copy taken before the delete is stale by definition, and writing it over a
     newer one would undo somebody else's change with no error anywhere. */
  it('a refused deletion does not overwrite a version that arrived in flight', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1', title: 'the old one' })] }))
    ipc.on('tracker_delete', () => {
      tracker.trackerState.issues.set('bd-1', issue({ id: 'bd-1', title: "somebody else's" }))
      throw new Error('bd failed')
    })

    await expect(tracker.deleteIssue('bd-1')).rejects.toThrow()

    expect(tracker.trackerState.issues.get('bd-1').title).toBe("somebody else's")
  })
})

describe('health and probing folders', () => {
  it('health arrives both as an event and as a command\'s answer', async () => {
    ipc.on('tracker_health', { state: 'not-a-beads-repo' })
    ipc.on('tracker_snapshot', snapshot())

    await tracker.initTracker()
    expect(tracker.trackerState.health.state).toBe('not-a-beads-repo')

    await emit('tracker:health', { state: 'ok' })
    expect(tracker.trackerState.health.state).toBe('ok')
  })

  it('a failed folder probe counts them as tracked: a warning is worse than silence', async () => {
    ipc.fail('tracker_probe', new Error('could not do it'))

    await expect(tracker.probeProjects(['/a', '/b'])).resolves.toEqual([
      { path: '/a', tracked: true },
      { path: '/b', tracked: true }
    ])
  })
})
