import { describe, expect, it } from 'vitest'
import { needsReady, promotesToReady, readyPromoteNote } from '../../../src/components/run/readyPromote.js'

describe('which statuses a run has to move first', () => {
  it('leaves ready alone, since that is where a run takes its work from', () => {
    expect(needsReady('open')).toBe(false)
  })

  it('leaves the two held statuses alone, because the run recovers those itself', () => {
    // snapshot in runs/queue.rs puts both into `unfinished`, and writing either
    // back to `open` would pull a live session's claim out from under it.
    //
    // Two things rest on this false, not one. The write above, and the card
    // menu: `runnableTask` refuses a blocked card only where this answers true,
    // because snapshot matches these two in an arm above the `OPEN` one and so
    // takes them whatever they wait on — a `ready_to_merge` task blocked by a
    // sibling that has not merged is exactly what a run is asked to merge.
    expect(needsReady('in_progress')).toBe(false)
    expect(needsReady('ready_to_merge')).toBe(false)
  })

  it('moves the built-in statuses the card menu still offers a run on', () => {
    expect(needsReady('deferred')).toBe(true)
    expect(needsReady('pinned')).toBe(true)
    expect(needsReady('hooked')).toBe(true)
  })

  it('moves this project\'s own working statuses', () => {
    expect(needsReady('human-check')).toBe(true)
  })

  it('moves a status it has never heard of, which is the defect it was written for', () => {
    expect(needsReady('marinating')).toBe(true)
    expect(needsReady('')).toBe(true)
    expect(needsReady(undefined)).toBe(true)
  })
})

describe('whether the move would land the task where a run can take it', () => {
  it('agrees with needsReady while nothing blocks the task', () => {
    expect(promotesToReady('deferred')).toBe(true)
    expect(promotesToReady('human-check', false)).toBe(true)
    expect(promotesToReady('open')).toBe(false)
    expect(promotesToReady('in_progress')).toBe(false)
  })

  it('refuses a task with an unfinished blocker, whatever its status', () => {
    // Blocked is a column and not a status: bd keeps a dependent issue at its
    // own status, so a `deferred` card with an unfinished blocker is never
    // bucketed under Blocked. Writing `open` would land it there and
    // `runs/queue.rs` would still leave it out of the ready set — the promote
    // would have spent the card's status on nothing.
    expect(promotesToReady('deferred', true)).toBe(false)
    expect(promotesToReady('pinned', true)).toBe(false)
    expect(promotesToReady('marinating', true)).toBe(false)
  })

  it('refuses a blocked task that needs no move either, so the two agree', () => {
    expect(promotesToReady('open', true)).toBe(false)
  })
})

describe('what the run dialog says about the move', () => {
  const scope = { kind: 'task', id: 'smetana-wbix', title: 'Run this' }

  it('names the task and what starting will do to it', () => {
    expect(readyPromoteNote(scope, 'deferred')).toBe(
      'Starting moves smetana-wbix to ready — a run takes its work from that column and nowhere else.'
    )
  })

  it('says nothing about a task already in ready', () => {
    expect(readyPromoteNote(scope, 'open')).toBe('')
  })

  it('says nothing about the two statuses a run recovers on its own', () => {
    expect(readyPromoteNote(scope, 'in_progress')).toBe('')
    expect(readyPromoteNote(scope, 'ready_to_merge')).toBe('')
  })

  it('says nothing for an epic or for the queue, which move no statuses', () => {
    expect(readyPromoteNote({ kind: 'epic', id: 'smetana-9', title: 'All of it' }, 'deferred')).toBe('')
    expect(readyPromoteNote({ kind: 'queue' }, 'deferred')).toBe('')
    expect(readyPromoteNote(null, 'deferred')).toBe('')
  })

  it('says nothing without a status, since the start would write nothing either', () => {
    expect(readyPromoteNote(scope, '')).toBe('')
    expect(readyPromoteNote(scope, undefined)).toBe('')
  })

  it('promises no move on a blocked task, since the start makes none', () => {
    // The line and the write read one rule, so a sentence cannot outlive the
    // write it describes: this card would land in Blocked, not in Ready.
    expect(readyPromoteNote(scope, 'deferred', true)).toBe('')
  })
})
