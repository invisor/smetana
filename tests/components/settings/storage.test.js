import { describe, expect, it } from 'vitest'
import {
  canClear,
  countFiles,
  formatBytes,
  removalLine,
  removedLine,
  scopeLine,
  storageLine
} from '../../../src/components/settings/storage.js'

const survey = (over = {}) => ({
  store: { files: 12, bytes: 5 * 1024 * 1024 },
  project: '/Users/you/Projects/smetana',
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
