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

  /* The board's period setting is measured on this date, so it has to reach the
     card at all — a card without it is a card the rule always shows. */
  it("carries bd's updated_at through to the card", async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1', updated_at: '2026-08-01T10:00:00Z' })] }))

    expect(tracker.boardColumns.value.flatMap((c) => c.tasks)[0].updatedAt).toBe(
      '2026-08-01T10:00:00Z'
    )
  })

  /* The done column is ordered on this date, so the card has to carry it —
     ordering off the issue instead would mean holding cards in the bucket and
     looking the key up somewhere else. */
  it("carries bd's closed_at through to the card", async () => {
    await start(
      snapshot({
        issues: [
          issue({ id: 'bd-1', status: 'closed', closed_at: '2026-08-01T10:00:00Z' }),
          issue({ id: 'bd-2' })
        ]
      })
    )

    const tasks = tracker.boardColumns.value.flatMap((c) => c.tasks)
    expect(tasks.find((t) => t.id === 'bd-1').closedAt).toBe('2026-08-01T10:00:00Z')
    expect(tasks.find((t) => t.id === 'bd-2').closedAt).toBeNull()
  })

  it('orders the done column by closing time, newest first', async () => {
    await start(
      snapshot({
        issues: [
          issue({ id: 'bd-1', status: 'closed', closed_at: '2026-08-01T11:10:00Z' }),
          issue({ id: 'bd-2', status: 'closed', closed_at: '2026-08-01T15:06:00Z' }),
          issue({ id: 'bd-3', status: 'closed', closed_at: '2026-07-28T09:00:00Z' })
        ]
      })
    )

    expect(
      tracker.boardColumns.value.find((c) => c.status === 'done').tasks.map((t) => t.id)
    ).toEqual(['bd-2', 'bd-1', 'bd-3'])
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

  it('names the blockers on both sides, and the counts are those names counted', async () => {
    // The card's hint says which task blocks it, so the ids have to survive the
    // trip. Asserting the count against the list's own length is the point: the
    // two are one fact projected twice and must not be able to disagree.
    await start(
      snapshot({
        issues: [
          issue({ id: 'bd-1' }),
          issue({ id: 'bd-9' }),
          issue({
            id: 'bd-2',
            dependencies: [
              edge({ issue_id: 'bd-2', depends_on_id: 'bd-1' }),
              edge({ issue_id: 'bd-2', depends_on_id: 'bd-9' })
            ]
          })
        ]
      })
    )

    const tasks = tracker.boardColumns.value.flatMap((column) => column.tasks)
    const blocked = tasks.find((task) => task.id === 'bd-2')
    const blocker = tasks.find((task) => task.id === 'bd-1')

    expect(blocked.blockedByIds).toEqual(['bd-1', 'bd-9'])
    expect(blocked.blockedBy).toBe(blocked.blockedByIds.length)
    expect(blocker.blockingIds).toEqual(['bd-2'])
    expect(blocker.blocks).toBe(blocker.blockingIds.length)
    expect(blocked.blockingIds).toEqual([])
  })

  it('an open task with an unfinished blocker sits in the blocked column, and closing the blocker releases it', async () => {
    // The whole of the unblocking mechanism: it is worked out here on every
    // snapshot, the way `bd ready` does it, so there is no stored status and
    // nothing that has to be written for the card to move.
    const blocked = () =>
      tracker.boardColumns.value.find((c) => c.status === 'blocked')?.tasks.map((t) => t.id) ?? []
    const ready = () =>
      tracker.boardColumns.value.find((c) => c.status === 'ready')?.tasks.map((t) => t.id) ?? []

    await start(
      snapshot({
        issues: [
          issue({ id: 'bd-1' }),
          issue({ id: 'bd-2', dependencies: [edge({ issue_id: 'bd-2', depends_on_id: 'bd-1' })] })
        ]
      })
    )

    expect(blocked()).toEqual(['bd-2'])
    expect(ready()).toEqual(['bd-1'])

    await emit('tracker:delta', delta({ upserted: [issue({ id: 'bd-1', status: 'closed' })] }))

    expect(blocked()).toEqual([])
    expect(ready()).toEqual(['bd-2'])
    const card = tracker.boardColumns.value.flatMap((c) => c.tasks).find((t) => t.id === 'bd-2')
    expect(card.blockedBy).toBe(0)
  })

  it('carries bd\'s own status beside the column the card is drawn in', async () => {
    // Blocked is a column, not a status anybody writes: the issue is `open`
    // with an unfinished blocker. A menu offering to move it needs the word bd
    // would accept back, so the two travel separately.
    await start(
      snapshot({
        issues: [
          issue({ id: 'bd-1' }),
          issue({ id: 'bd-2', dependencies: [edge({ issue_id: 'bd-2', depends_on_id: 'bd-1' })] })
        ]
      })
    )

    const card = tracker.boardColumns.value.flatMap((c) => c.tasks).find((t) => t.id === 'bd-2')

    expect(card.status).toBe('blocked')
    expect(card.bdStatus).toBe('open')
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

/* The merge lock is coordination between two leads, not work, so nothing on
   screen draws it — while the store keeps it whole, because the dependency
   reasoning here and `queue.rs` both need it to be there. */
describe('the merge lock', () => {
  const lock = (over = {}) =>
    issue({
      id: 'bd-lock',
      title: 'Merge lock',
      issue_type: 'chore',
      labels: ['smetana-lock'],
      ...over
    })

  const ids = () => tracker.boardColumns.value.flatMap((column) => column.tasks).map((t) => t.id)

  it('is in no column while it is free', async () => {
    await start(snapshot({ issues: [lock(), issue({ id: 'bd-1' })] }))

    expect(ids()).toEqual(['bd-1'])
  })

  /* Two columns, not one: a free lock is `open` and a held one is
     `in_progress`, which is why the skip is before the bucketing rather than
     inside a column. */
  it('is in no column while it is held, nor under a status nobody expected', async () => {
    await start(
      snapshot({
        issues: [
          lock({ status: 'in_progress', assignee: 'smetana-run-7' }),
          issue({ id: 'bd-1', status: 'in_progress' })
        ]
      })
    )
    expect(ids()).toEqual(['bd-1'])

    await emit('tracker:delta', delta({ upserted: [lock({ status: 'awaiting-review' })] }))
    expect(ids()).toEqual(['bd-1'])
  })

  it('an issue without the label is drawn as it always was', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1', labels: ['chore', 'smetana'] })] }))

    expect(ids()).toEqual(['bd-1'])
  })

  /* The filter is on the way out to the interface, never on the way in. A lock
     dropped from the store would read as a blocker that is not on the board,
     which `holds` treats as satisfied — so anything wired to depend on it would
     quietly become ready. */
  it('stays in the store, and an unfinished lock still blocks what depends on it', async () => {
    await start(
      snapshot({
        issues: [
          lock({ status: 'in_progress' }),
          issue({
            id: 'bd-2',
            dependencies: [edge({ issue_id: 'bd-2', depends_on_id: 'bd-lock' })]
          })
        ]
      })
    )

    expect(tracker.issueById('bd-lock')).toMatchObject({ id: 'bd-lock', title: 'Merge lock' })

    const board = tracker.boardColumns.value
    expect(board.find((c) => c.status === 'blocked').tasks.map((t) => t.id)).toEqual(['bd-2'])
    expect(board.flatMap((c) => c.tasks).find((t) => t.id === 'bd-2').blockedByIds).toEqual([
      'bd-lock'
    ])
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

  /* `Map.set` on a key it already has leaves the entry where it stood, so a
     task closed during a session used to keep the slot it held while it was
     open — the one card on the board a person is most likely to be looking
     for, anywhere but the top. */
  it('a task closed by a delta becomes the first card in done', async () => {
    await start(
      snapshot({
        generation: 5,
        issues: [
          issue({ id: 'bd-1', status: 'closed', closed_at: '2026-08-01T10:00:00Z' }),
          issue({ id: 'bd-2' })
        ]
      })
    )

    const done = () => tracker.boardColumns.value.find((c) => c.status === 'done').tasks.map((t) => t.id)
    expect(done()).toEqual(['bd-1'])

    await emit('tracker:delta', delta({
      generation: 6,
      upserted: [issue({ id: 'bd-2', status: 'closed', closed_at: '2026-08-05T09:30:00Z' })]
    }))

    expect(done()).toEqual(['bd-2', 'bd-1'])
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
    ipc.on('tracker_resync', snapshot({ generation: 9, issues: [issue({ id: 'bd-9' })] }))

    await tracker.initTracker()

    expect(tracker.trackerState.generation).toBe(9)
    expect(tracker.trackerState.issues.has('bd-9')).toBe(true)
    expect(tracker.trackerState.issues.has('bd-old')).toBe(false)
  })

  /* Discarding the stale snapshot leaves the board holding only what that one
     delta carried — every issue the watcher had no reason to mention is
     missing. Generations run consecutively from there, so the gap check never
     fires, and the back end's own sweep sees no discrepancy to report. The
     moment the snapshot is discarded is the only place left to recover. */
  it('a snapshot discarded as stale is recovered by a resync', async () => {
    ipc.on('tracker_health', { state: 'ok' })
    ipc.on('tracker_snapshot', async () => {
      await emit('tracker:delta', delta({ generation: 9, upserted: [issue({ id: 'bd-9' })] }))
      return snapshot({ generation: 5, issues: [issue({ id: 'bd-old' })] })
    })
    ipc.on('tracker_resync', snapshot({
      generation: 9,
      issues: [issue({ id: 'bd-1' }), issue({ id: 'bd-9' })]
    }))

    await tracker.initTracker()

    expect(ipc.calls('tracker_resync')).toHaveLength(1)
    /* initTracker does not return until the board is whole: the issue the
       delta never mentioned is there. */
    expect([...tracker.trackerState.issues.keys()].sort()).toEqual(['bd-1', 'bd-9'])
    expect(tracker.trackerState.generation).toBe(9)
    expect(tracker.trackerState.ready).toBe(true)
  })

  /* A fresh snapshot is the ordinary path, and it must not pay for the
     recovery with an extra bd call — one costs about two seconds. */
  it('a snapshot that is still current costs no resync', async () => {
    await start(snapshot({ generation: 5, issues: [issue({ id: 'bd-1' })] }))

    expect(ipc.calls('tracker_resync')).toHaveLength(0)
    expect(tracker.trackerState.issues.has('bd-1')).toBe(true)
  })

  /* The recovery is a read, and a read that fails is already handled the way
     every other one is: the board keeps what it has and the error is
     remembered. What must not happen is initTracker throwing — the app would
     come up with no board at all. */
  it('a failed recovery leaves the board up and remembers the read error', async () => {
    ipc.on('tracker_health', { state: 'ok' })
    ipc.on('tracker_snapshot', async () => {
      await emit('tracker:delta', delta({ generation: 9, upserted: [issue({ id: 'bd-9' })] }))
      return snapshot({ generation: 5, issues: [issue({ id: 'bd-old' })] })
    })
    ipc.fail('tracker_resync', new Error('bd did not start'))

    await expect(tracker.initTracker()).resolves.toBeUndefined()

    expect(tracker.trackerState.ready).toBe(true)
    expect(tracker.trackerState.issues.has('bd-9')).toBe(true)
    expect(tracker.trackerState.lastError.title).toBe('Could not read the tracker')
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

  /* smetana-a5b: the patch's `assignee` is bd's `-a` and belongs on the issue's
     own `assignee`, not on `owner`. While it landed on `owner`, an assignee edit
     painted over the owner on screen — the wrong name, under the wrong label,
     until a delta arrived and quietly corrected it. */
  it('an assignee edit shows on the assignee and leaves the owner alone', async () => {
    await start(
      snapshot({
        issues: [issue({ id: 'bd-1', owner: 'merazent@gmail.com', assignee: null })]
      })
    )
    ipc.on('tracker_update', () =>
      issue({ id: 'bd-1', owner: 'merazent@gmail.com', assignee: 'smetana-run-7' })
    )

    const pending = tracker.updateIssue('bd-1', { assignee: 'smetana-run-7' })
    expect(tracker.trackerState.issues.get('bd-1')).toMatchObject({
      owner: 'merazent@gmail.com',
      assignee: 'smetana-run-7'
    })

    await pending
    expect(tracker.trackerState.issues.get('bd-1')).toMatchObject({
      owner: 'merazent@gmail.com',
      assignee: 'smetana-run-7'
    })
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

describe('the semantic tier', () => {
  it('hands the agent a query and keeps the ids it answered with', async () => {
    ipc.on('tracker_search_semantic', () => ['smetana-a1a', 'smetana-b2b'])

    await tracker.searchSemantic('the bell is silent')

    expect(ipc.calls('tracker_search_semantic')).toEqual([{ query: 'the bell is silent' }])
    expect(tracker.searchState.ids).toEqual(['smetana-a1a', 'smetana-b2b'])
    expect(tracker.searchState.pending).toBe(false)
    expect(tracker.searchState.error).toBe(null)
  })

  it('keeps the refusal as a sentence and stops spinning', async () => {
    ipc.fail('tracker_search_semantic', {
      kind: 'noAgent',
      message: 'Smetana looked for claude on your PATH and found nothing.'
    })

    await tracker.searchSemantic('anything')

    expect(tracker.searchState.error).toBe(
      'Smetana looked for claude on your PATH and found nothing.'
    )
    expect(tracker.searchState.pending).toBe(false)
    expect(tracker.searchState.ids).toEqual([])
  })

  /* An empty answer is what NONE comes back as, and it is not a failure: the
     agent looked and nothing matched. Drawing it as one would put a red
     sentence under a question that was answered properly. */
  it('an empty answer is an answer, not a refusal', async () => {
    ipc.on('tracker_search_semantic', () => [])

    await tracker.searchSemantic('nothing like this exists')

    expect(tracker.searchState.ids).toEqual([])
    expect(tracker.searchState.error).toBe(null)
  })

  /* The case the guard exists for, and the one that was broken: the component
     resets on every keystroke, but a request already sent is neither cancelled
     nor tagged, so without the guard the old answer lands under the new query
     and is drawn as an answer to it. */
  it('drops an answer whose question moved on while it was out', async () => {
    let release
    ipc.on('tracker_search_semantic', () => new Promise((resolve) => { release = resolve }))

    const inFlight = tracker.searchSemantic('the bell is silent')
    // A keystroke: this is what the field emits on every one of them.
    tracker.clearSemantic()
    release(['smetana-a1a'])
    await inFlight

    expect(tracker.searchState.ids).toEqual([])
    expect(tracker.searchState.answered).toBe(false)
  })

  it('drops a refusal whose question moved on while it was out', async () => {
    let reject
    ipc.on('tracker_search_semantic', () => new Promise((_, no) => { reject = no }))

    const inFlight = tracker.searchSemantic('the bell is silent')
    tracker.clearSemantic()
    reject({ kind: 'timeout', message: 'The agent did not answer within 90 seconds.' })
    await inFlight

    expect(tracker.searchState.error).toBe(null)
  })

  /* The flag is cleared whether or not the answer was wanted: it is what the
     ask row spins on, and a stale answer that left it up would leave the person
     watching a spinner about a question that is over, unable to ask again. */
  it('a stale answer still frees the field for the next question', async () => {
    let release
    ipc.on('tracker_search_semantic', () => new Promise((resolve) => { release = resolve }))

    const inFlight = tracker.searchSemantic('one thing')
    tracker.clearSemantic()
    release(['smetana-a1a'])
    await inFlight
    expect(tracker.searchState.pending).toBe(false)

    ipc.on('tracker_search_semantic', () => ['smetana-b2b'])
    await tracker.searchSemantic('another thing')

    expect(tracker.searchState.ids).toEqual(['smetana-b2b'])
    expect(tracker.searchState.answered).toBe(true)
  })

  /* `NONE` and "nothing has been asked" are both an empty list, and the list on
     screen draws them differently, so the store has to tell them apart. */
  it('says an answer arrived even when it named nothing', async () => {
    ipc.on('tracker_search_semantic', () => [])

    expect(tracker.searchState.answered).toBe(false)
    await tracker.searchSemantic('nothing like this exists')

    expect(tracker.searchState.answered).toBe(true)
    expect(tracker.searchState.error).toBe(null)
  })

  /* The one thing in this store that speaks for the agent, so it must never say
     something the agent did not say. An answer that outlived its project leaves
     the list drawing "Nothing matched" about a folder nobody asked about. */
  it('forgets the answer when the project underneath it changes', async () => {
    ipc.on('tracker_search_semantic', () => ['smetana-a1a'])
    ipc.on('tracker_set_project', snapshot())

    await tracker.searchSemantic('the bell is silent')
    expect(tracker.searchState.answered).toBe(true)

    await tracker.setProject('/Users/you/dev/notes')

    expect(tracker.searchState.ids).toEqual([])
    expect(tracker.searchState.answered).toBe(false)
  })

  /* Cleared before the switch is attempted rather than after it succeeds: the
     question is over whatever the tracker goes on to answer. */
  it('forgets it even when the switch itself fails', async () => {
    ipc.on('tracker_search_semantic', () => ['smetana-a1a'])
    ipc.fail('tracker_set_project', new Error('bd failed'))

    await tracker.searchSemantic('the bell is silent')
    await tracker.setProject('/Users/you/dev/notes')

    expect(tracker.searchState.answered).toBe(false)
  })

  it('clears the last answer, so it cannot be read under a different question', async () => {
    ipc.on('tracker_search_semantic', () => ['smetana-a1a'])

    await tracker.searchSemantic('one thing')
    tracker.clearSemantic()

    expect(tracker.searchState.ids).toEqual([])
    expect(tracker.searchState.error).toBe(null)
    expect(tracker.searchState.answered).toBe(false)
  })

  /* Two questions never overlap: the answer to the second would arrive under a
     query nobody can see any more, and the first is already on its way. */
  it('drops a second question while one is still out', async () => {
    let release
    ipc.on('tracker_search_semantic', () => new Promise((resolve) => { release = resolve }))

    const first = tracker.searchSemantic('one thing')
    await tracker.searchSemantic('another thing')

    expect(ipc.calls('tracker_search_semantic')).toEqual([{ query: 'one thing' }])

    release(['smetana-a1a'])
    await first
    expect(tracker.searchState.ids).toEqual(['smetana-a1a'])
  })

  it('an empty query asks nothing at all', async () => {
    ipc.on('tracker_search_semantic', () => ['smetana-a1a'])

    await tracker.searchSemantic('   ')

    expect(ipc.calls('tracker_search_semantic')).toEqual([])
  })
})
