import { describe, expect, it } from 'vitest'
import {
  canClear,
  countFiles,
  formatBytes,
  removalLine,
  projectBytes,
  removedLine,
  scopeLine,
  storageLine
} from '../../../src/components/settings/storage.js'

const survey = (over = {}) => ({
  store: { files: 12, bytes: 5 * 1024 * 1024 },
  project: '/Users/you/Projects/smetana',
  board: 'ok',
  kept: { files: 4, bytes: 2 * 1024 * 1024 },
  removable: { files: 3, bytes: 1024 * 1024 },
  ...over
})

describe('sizes as a person reads them', () => {
  it('counts small things in whole bytes and larger ones to one decimal', () => {
    expect(formatBytes(0)).toBe('0 bytes')
    expect(formatBytes(512)).toBe('512 bytes')
    expect(formatBytes(1024)).toBe('1.0 KiB')
    expect(formatBytes(1536)).toBe('1.5 KiB')
    expect(formatBytes(8 * 1024 * 1024)).toBe('8.0 MiB')
  })

  it('says nothing rather than guessing when there is no number', () => {
    expect(formatBytes(null)).toBe('—')
    expect(formatBytes(undefined)).toBe('—')
    expect(formatBytes(-1)).toBe('—')
  })

  it('counts one file as one file', () => {
    expect(countFiles(1)).toBe('1 file')
    expect(countFiles(0)).toBe('0 files')
    expect(countFiles(7)).toBe('7 files')
  })
})

describe('what the storage section says about the store', () => {
  it('names the whole store, which is more than the button can reach', () => {
    expect(storageLine(survey())).toBe('5.0 MiB in 12 files')
  })

  it('is counting rather than empty while the answer is on its way', () => {
    // Zero is a fact about the disk, and drawing it before anything has been
    // read would tell a person their pictures are gone.
    expect(storageLine(null)).toBe('Counting…')
    expect(storageLine({})).toBe('Counting…')
  })

  it('says so when there is nothing stored at all', () => {
    expect(storageLine(survey({ store: { files: 0, bytes: 0 } }))).toBe('Nothing stored')
  })
})

describe('what a person reads before pressing the button', () => {
  it('names both the count and the size, and that the press cannot be undone', () => {
    const line = removalLine(survey())
    expect(line).toContain('3 files')
    expect(line).toContain('1.0 MiB')
    expect(line).toContain('cannot be undone')
  })

  it('explains the empty case by which of the two empties it is', () => {
    // Nothing to delete because everything is in use, against nothing to delete
    // because nothing was ever attached here: the same disabled button, two
    // different things to know.
    expect(removalLine(survey({ removable: { files: 0, bytes: 0 } }))).toContain(
      'belongs to a task that is still open'
    )
    expect(
      removalLine(survey({ removable: { files: 0, bytes: 0 }, kept: { files: 0, bytes: 0 } }))
    ).toContain('no stored images')
  })

  it('says why there is nothing it can decide when no project is open', () => {
    const line = removalLine(survey({ project: null }))
    expect(line).toContain('No project is open')
  })

  it('waits rather than claiming a number it has not been given', () => {
    expect(removalLine(null)).toContain('Counting')
    expect(removalLine(survey({ removable: undefined }))).toContain('could not be read')
  })

  it('says the board could not be read instead of counting off an empty one', () => {
    // An unreadable board reaches this front end as an empty one, so a sentence
    // about counts would be read as a fact about the pictures. Rust already
    // refuses the press; this is the same refusal said before it is pressed.
    for (const board of ['error', 'bd-version-mismatch']) {
      const line = removalLine(survey({ board, removable: { files: 0, bytes: 0 } }))
      expect(line).toContain('could not be read')
      expect(line).toContain('Nothing will be deleted')
    }
  })

  it('names a folder with no tracker as itself, since that one can be acted on', () => {
    expect(removalLine(survey({ board: 'not-a-beads-repo' }))).toContain('no bd tracker')
  })

  it('names the one project the button reaches', () => {
    expect(scopeLine(survey())).toContain('smetana')
    expect(scopeLine(survey({ project: null }))).toBe('')
  })
})

describe('whether the button may be pressed at all', () => {
  it('is live only with a project and something to take', () => {
    expect(canClear(survey())).toBe(true)
    expect(canClear(survey({ removable: { files: 0, bytes: 0 } }))).toBe(false)
    expect(canClear(survey({ project: null }))).toBe(false)
    expect(canClear(null)).toBe(false)
  })

  it('is dead whenever the board was not read, whatever the counts say', () => {
    // The counts are zero in these states, so this looks redundant and is not:
    // a build that ever offered files off an unread board would delete the
    // attachments of every open task in the project.
    for (const board of ['error', 'not-a-beads-repo', 'bd-version-mismatch', 'no-project']) {
      expect(canClear(survey({ board }))).toBe(false)
    }
    // A word this build has never heard of, and the field missing altogether:
    // both are "not read", never "assume it was".
    expect(canClear(survey({ board: 'something-new' }))).toBe(false)
    expect(canClear(survey({ board: undefined }))).toBe(false)
  })
})

describe('how much of the store belongs to this project', () => {
  it('is what stays and what could go, together', () => {
    // 2 MiB kept and 1 MiB removable: the folder the button reaches is 3 MiB,
    // whichever side of the rule each file falls.
    expect(projectBytes(survey())).toBe(3 * 1024 * 1024)
  })

  it('has no answer at all when the board was not read', () => {
    // Both tallies are zero in that state by design, so a sum would report an
    // empty folder for one that may hold hundreds of megabytes — and the bell
    // would re-arm its ladder off a number nobody measured.
    for (const board of ['error', 'not-a-beads-repo', 'bd-version-mismatch', 'something-new']) {
      expect(projectBytes(survey({ board }))).toBe(null)
    }
    expect(projectBytes(survey({ board: undefined }))).toBe(null)
  })

  it('has no answer with no project, and none before the survey lands', () => {
    expect(projectBytes(survey({ project: null }))).toBe(null)
    expect(projectBytes(null)).toBe(null)
    expect(projectBytes({})).toBe(null)
  })

  it('tells an empty folder apart from a folder nobody counted', () => {
    expect(projectBytes(survey({ kept: { files: 0, bytes: 0 }, removable: { files: 0, bytes: 0 } })))
      .toBe(0)
    expect(projectBytes(survey({ removable: undefined }))).toBe(null)
  })
})

describe('what the press turned out to have done', () => {
  it('reports what actually went, which need not be what was offered', () => {
    expect(removedLine({ removed: { files: 3, bytes: 1024 * 1024 }, failed: 0 })).toBe(
      'Deleted 3 files (1.0 MiB).'
    )
  })

  it('says plainly when nothing went', () => {
    expect(removedLine({ removed: { files: 0, bytes: 0 }, failed: 0 })).toBe('Nothing was deleted.')
  })

  it('names the files the disk would not give up rather than swallowing them', () => {
    const line = removedLine({ removed: { files: 2, bytes: 2048 }, failed: 1 })
    expect(line).toContain('Deleted 2 files')
    expect(line).toContain('1 file could not be deleted')
  })

  it('says nothing at all until a press has happened', () => {
    expect(removedLine(null)).toBe('')
  })
})
