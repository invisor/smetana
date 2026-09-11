import { describe, expect, it } from 'vitest'
import { notesForDisplay, splitNoteEntries } from '../../../src/components/kanban/noteEntries.js'

describe('splitNoteEntries', () => {
  it('reads two parked lines as two separate records', () => {
    // The gallery's own `FULL_ISSUE.notes` fixture, and the bug's reproduction
    // case: two `bd note` calls, joined by bd with a single newline.
    const notes = 'parked: needs a decision on where the strip sits\nparked: still waiting on the design call'
    expect(splitNoteEntries(notes)).toEqual([
      'parked: needs a decision on where the strip sits',
      'parked: still waiting on the design call'
    ])
  })

  it('separates every marker the app itself writes', () => {
    const notes = [
      'parked: which storage format',
      'resolved: sqlite, decided 2026-08-01',
      'merged: feature/x-a769 is in main',
      'digest: [high] a stray screenshot in the repo root — from smetana-9zz (depth 1)',
      'closed with follow-up smetana-2ab',
      'live check skipped: nothing user-facing',
      'epic child — live check owed to the parent',
      'batch 3 (smetana-run-10) ended without finishing this; it is open again and unclaimed'
    ].join('\n')
    expect(splitNoteEntries(notes)).toEqual([
      'parked: which storage format',
      'resolved: sqlite, decided 2026-08-01',
      'merged: feature/x-a769 is in main',
      'digest: [high] a stray screenshot in the repo root — from smetana-9zz (depth 1)',
      'closed with follow-up smetana-2ab',
      'live check skipped: nothing user-facing',
      'epic child — live check owed to the parent',
      'batch 3 (smetana-run-10) ended without finishing this; it is open again and unclaimed'
    ])
  })

  it('keeps a record a person broke across lines whole, rather than as two records', () => {
    // What a marker cannot see: a continuation line carries no marker of its
    // own, so it stays folded onto the record before it.
    const notes = 'parked: checked with prod on this — looks fine\njust double check the migration before merge'
    expect(splitNoteEntries(notes)).toEqual([
      'parked: checked with prod on this — looks fine\njust double check the migration before merge'
    ])
  })

  it('reads the very first line as a record even without a marker', () => {
    // An issue's first-ever note may be ordinary prose with no marker at all
    // — it still has to stand on its own rather than vanish into nothing.
    const notes = 'Talked to Ann about this.\nparked: who owns the migration'
    expect(splitNoteEntries(notes)).toEqual(['Talked to Ann about this.', 'parked: who owns the migration'])
  })

  it('tolerates the case and whitespace a hand-written note carries', () => {
    const notes = 'parked: one thing\n  Resolved:   done'
    expect(splitNoteEntries(notes)).toEqual(['parked: one thing', '  Resolved:   done'])
  })

  it('finds nothing on a task with no notes at all', () => {
    for (const notes of ['', null, undefined]) {
      expect(splitNoteEntries(notes)).toEqual([])
    }
  })

  it('reads a note with no marker and no continuation as one record', () => {
    expect(splitNoteEntries('Nothing to say.')).toEqual(['Nothing to say.'])
  })
})

describe('notesForDisplay', () => {
  it('joins records with a blank line, the way markdown reads a new paragraph', () => {
    const notes = 'parked: needs a decision on where the strip sits\nparked: still waiting on the design call'
    expect(notesForDisplay(notes)).toBe(
      'parked: needs a decision on where the strip sits\n\nparked: still waiting on the design call'
    )
  })

  it('leaves a record’s own internal line break as a single newline', () => {
    const notes = 'parked: checked with prod on this\njust double check the migration'
    expect(notesForDisplay(notes)).toBe('parked: checked with prod on this\njust double check the migration')
  })

  it('is the empty string for a task with no notes', () => {
    expect(notesForDisplay('')).toBe('')
    expect(notesForDisplay(null)).toBe('')
  })
})
