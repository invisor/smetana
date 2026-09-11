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

  it('splits two ordinary sentences with no marker, no indentation and no list syntax', () => {
    // What the marker vocabulary got wrong: a real board carries plenty of
    // notes with no marker at all, and each is still its own record.
    const notes = [
      'the contract disagrees with what the Rust side actually returns',
      'the epic is waiting on smetana-5ijg before it can close',
      'the last task under the epic was closed by hand'
    ].join('\n')
    expect(splitNoteEntries(notes)).toEqual([
      'the contract disagrees with what the Rust side actually returns',
      'the epic is waiting on smetana-5ijg before it can close',
      'the last task under the epic was closed by hand'
    ])
  })

  it('keeps an indented continuation folded onto the record above it', () => {
    const notes = 'checked with prod on this\n  and it looks fine, just double check the migration'
    expect(splitNoteEntries(notes)).toEqual([
      'checked with prod on this\n  and it looks fine, just double check the migration'
    ])
  })

  it('keeps a numbered enumeration folded onto the header line above it', () => {
    // The genuine multi-line case acceptance criterion #2 exists for: a
    // header sentence followed immediately by the list explaining it, with
    // no blank line between them.
    const notes = [
      'seven files carry the same rename, paired below',
      '1) old-a.js -> new-a.js',
      '2) old-b.js -> new-b.js',
      '3) old-c.js -> new-c.js'
    ].join('\n')
    expect(splitNoteEntries(notes)).toEqual([
      [
        'seven files carry the same rename, paired below',
        '1) old-a.js -> new-a.js',
        '2) old-b.js -> new-b.js',
        '3) old-c.js -> new-c.js'
      ].join('\n')
    ])
  })

  it('folds a bullet or a quote line the same way as a numbered one', () => {
    const notes = 'three things to check\n- the migration\n* the seed data\n> already flagged once'
    expect(splitNoteEntries(notes)).toEqual([
      'three things to check\n- the migration\n* the seed data\n> already flagged once'
    ])
  })

  it('keeps a blank line inside one record rather than reading it as another', () => {
    // A checklist a person wrote as a single `bd note` call, its items
    // separated by their own blank lines — still one record.
    const notes = ['- [ ] first item', '', '- [ ] second item', '', '- [ ] third item'].join('\n')
    expect(splitNoteEntries(notes)).toEqual([
      ['- [ ] first item', '', '- [ ] second item', '', '- [ ] third item'].join('\n')
    ])
  })

  it('reads the very first line as a record even when it looks like a continuation', () => {
    // There is no record above the field's first line to fold onto, whatever
    // shape that first line has.
    const notes = '- an issue filed straight as a bulleted note\nsomething ordinary after it'
    expect(splitNoteEntries(notes)).toEqual([
      '- an issue filed straight as a bulleted note',
      'something ordinary after it'
    ])
  })

  it('does not read list syntax sitting mid-line as an opener', () => {
    // The marker is what a line *opens* with; a hyphen or a number later in
    // the sentence is not that, and folding on it would glue two unrelated
    // records together on the strength of a stray character.
    const notes = 'do this by Tuesday - not before\nthe other task can wait'
    expect(splitNoteEntries(notes)).toEqual([
      'do this by Tuesday - not before',
      'the other task can wait'
    ])
  })

  it('finds nothing on a task with no notes at all', () => {
    for (const notes of ['', null, undefined, '   ', '\n\n']) {
      expect(splitNoteEntries(notes)).toEqual([])
    }
  })

  it('reads a lone note with no marker and no continuation as one record', () => {
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

  it('leaves a record’s own indented continuation as a single newline', () => {
    const notes = 'checked with prod on this\n  just double check the migration'
    expect(notesForDisplay(notes)).toBe('checked with prod on this\n  just double check the migration')
  })

  it('is the empty string for a task with no notes', () => {
    expect(notesForDisplay('')).toBe('')
    expect(notesForDisplay(null)).toBe('')
  })
})
