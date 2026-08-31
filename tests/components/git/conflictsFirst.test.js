import { describe, expect, it } from 'vitest'
import { conflictsFirst } from '../../../src/components/git/conflictsFirst.js'

/* The shape `stores/vcs.js` holds, cut down to the two fields this rule reads.
   The order here is git's, which is the thing being preserved. */
const change = (path, kind) => ({ path, origPath: null, kind, staged: false, unstaged: true })

const MIXED = [
  change('src/stores/vcs.js', 'modified'),
  change('axios.js', 'conflicted'),
  change('notes.txt', 'untracked'),
  change('src/main.js', 'conflicted'),
  change('README.md', 'modified')
]

const paths = (rows) => rows.map((row) => row.path)

describe('where a conflicted file sits in the change list', () => {
  it('lifts every conflict above the rest', () => {
    expect(paths(conflictsFirst(MIXED))).toEqual([
      'axios.js',
      'src/main.js',
      'src/stores/vcs.js',
      'notes.txt',
      'README.md'
    ])
  })

  /* Git's order is what the list showed before this rule existed, and it is
     kept inside both groups: a sort by name would throw it away silently. */
  it('keeps git order inside each group', () => {
    const sorted = conflictsFirst(MIXED)
    expect(paths(sorted.slice(0, 2))).toEqual(['axios.js', 'src/main.js'])
    expect(paths(sorted.slice(2))).toEqual(['src/stores/vcs.js', 'notes.txt', 'README.md'])
  })

  it('returns a list with no conflict in it as it stands', () => {
    const clean = [change('a.js', 'modified'), change('b.js', 'untracked'), change('c.js', 'added')]
    expect(paths(conflictsFirst(clean))).toEqual(['a.js', 'b.js', 'c.js'])
  })

  /* The caller is a computed reading the store's own array, so reordering in
     place would rewrite what the store holds on the way past. */
  it('leaves the array it was given alone', () => {
    const given = [...MIXED]
    const sorted = conflictsFirst(given)
    expect(paths(given)).toEqual(paths(MIXED))
    expect(sorted).not.toBe(given)
  })

  it('gives an empty list back for an empty one', () => {
    const given = []
    const sorted = conflictsFirst(given)
    expect(sorted).toEqual([])
    expect(sorted).not.toBe(given)
  })
})
