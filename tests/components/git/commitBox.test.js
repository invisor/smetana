import { describe, it, expect } from 'vitest'
import {
  DEFAULT_ROWS,
  MAX_ROWS,
  MIN_ROWS,
  canCommit,
  canSuggest,
  clampRows,
  commitHint,
  commitLabel,
  messagePlaceholder,
  resolveDragRows
} from '../../../src/components/git/commitBox.js'

const ready = { message: 'fix: the thing', changes: 3, allowed: true, reason: null, busy: false }

describe('canCommit', () => {
  it('takes a message, something to commit, and permission', () => {
    expect(canCommit(ready)).toBe(true)
  })

  it('refuses a message that is only whitespace', () => {
    expect(canCommit({ ...ready, message: '   \n  ' })).toBe(false)
    expect(canCommit({ ...ready, message: '' })).toBe(false)
  })

  it('refuses a clean tree', () => {
    expect(canCommit({ ...ready, changes: 0 })).toBe(false)
  })

  it('refuses while a run holds the repository', () => {
    expect(canCommit({ ...ready, allowed: false })).toBe(false)
  })

  it('refuses while git is already busy with something', () => {
    expect(canCommit({ ...ready, busy: true })).toBe(false)
  })
})

describe('canSuggest', () => {
  /* Reading, not writing: a run holding the three writes has no say over it,
     which is the same line `BranchList` draws for a folder heading. */
  it('is offered while a run blocks every write', () => {
    expect(canSuggest({ changes: 3, allowed: false, suggesting: false })).toBe(true)
  })

  it('is not offered with nothing to describe', () => {
    expect(canSuggest({ changes: 0, allowed: true, suggesting: false })).toBe(false)
  })

  it('is not offered twice at once', () => {
    expect(canSuggest({ changes: 3, allowed: true, suggesting: true })).toBe(false)
  })
})

describe('commitLabel', () => {
  it('counts the files it is about to commit', () => {
    expect(commitLabel(14)).toBe('Commit 14 files')
  })

  it('says file rather than files for one', () => {
    expect(commitLabel(1)).toBe('Commit 1 file')
  })

  /* The button is not drawn over a clean tree, so this is the shape of the
     word rather than a state to design for — but a label reading "Commit 0
     files" would be worse than a plain verb if it ever showed. */
  it('drops the count entirely when there is nothing', () => {
    expect(commitLabel(0)).toBe('Commit')
  })
})

describe('messagePlaceholder', () => {
  it('names the branch the commit would land on', () => {
    expect(messagePlaceholder({ branch: 'develop', mac: true })).toBe(
      'Message (⌘Enter to commit on "develop")'
    )
  })

  it('names the key the platform actually has', () => {
    expect(messagePlaceholder({ branch: 'develop', mac: false })).toBe(
      'Message (Ctrl+Enter to commit on "develop")'
    )
  })

  /* A detached HEAD is a tree somebody can still commit to, so the field still
     invites it — it just has no branch to name, and "on nothing" would be worse
     than saying nothing. */
  it('says nothing about a branch when there is none', () => {
    expect(messagePlaceholder({ branch: null, mac: true })).toBe('Message (⌘Enter to commit)')
  })
})

describe('commitHint', () => {
  it('says nothing when the button works', () => {
    expect(commitHint(ready)).toBe(null)
  })

  it('passes on the run’s own sentence, which is the only one that names a cause', () => {
    expect(commitHint({ ...ready, allowed: false, reason: 'A run is going in this project.' })).toBe(
      'A run is going in this project.'
    )
  })

  it('has nothing to add when a run blocks it and says why nowhere', () => {
    expect(commitHint({ ...ready, allowed: false, reason: null })).toBe(null)
  })

  it('asks for a message before it asks for anything else', () => {
    expect(commitHint({ ...ready, message: ' ' })).toBe('Write a message first.')
  })

  /* Order matters here: with no message *and* no changes, the sentence that
     helps is the one about the thing a person can do something about. */
  it('says a clean tree before it asks for a message', () => {
    expect(commitHint({ ...ready, changes: 0, message: '' })).toBe('Nothing to commit.')
  })

  it('says nothing while git is working', () => {
    expect(commitHint({ ...ready, busy: true, message: '' })).toBe(null)
  })
})

describe('clampRows', () => {
  it('leaves a height inside the range alone', () => {
    expect(clampRows(5)).toBe(5)
  })

  it('keeps a field there at all', () => {
    expect(clampRows(0)).toBe(MIN_ROWS)
    expect(clampRows(-4)).toBe(MIN_ROWS)
  })

  it('will not let the field bury the list under it', () => {
    expect(clampRows(900)).toBe(MAX_ROWS)
  })

  /* A count is what a `<textarea>`'s `rows` takes, and half a row is not one. */
  it('answers a whole number of rows', () => {
    expect(clampRows(4.4)).toBe(4)
    expect(clampRows(4.6)).toBe(5)
  })

  it('takes the shipped height for anything that is not a number', () => {
    expect(clampRows('tall')).toBe(DEFAULT_ROWS)
    expect(clampRows(undefined)).toBe(DEFAULT_ROWS)
  })
})

describe('resolveDragRows', () => {
  it('follows the pointer down and up', () => {
    expect(resolveDragRows({ base: 2, delta: 3 })).toBe(5)
    expect(resolveDragRows({ base: 5, delta: -2 })).toBe(3)
  })

  it('measures every delta from the height the drag began at', () => {
    // Two moves of one gesture, the first past the floor. Both are answered
    // against `base`, so pulling back hands the rows back rather than
    // compounding what the clamp already swallowed.
    expect(resolveDragRows({ base: 4, delta: -40 })).toBe(MIN_ROWS)
    expect(resolveDragRows({ base: 4, delta: -1 })).toBe(3)
  })

  it('stops at the ceiling however far the pointer goes', () => {
    expect(resolveDragRows({ base: 2, delta: 400 })).toBe(MAX_ROWS)
  })
})
