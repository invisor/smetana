import { describe, expect, it } from 'vitest'
import { writeSummary } from '../../../src/components/git/writeSummary.js'

/* What Rust measured, in the shape `MergeOutcome::Clean` and `vcs_push` carry
   it. Every field is optional there, so a fixture that left one out by accident
   would be testing a different case than it reads as — hence the four written
   out every time. */
const landed = (commits, files, insertions, deletions) => ({
  commits,
  files,
  insertions,
  deletions
})

const nothing = landed(0, 0, 0, 0)
const unmeasured = landed(null, null, null, null)

describe('what the corner says after a merge', () => {
  it('names the branch and counts what came with it', () => {
    expect(
      writeSummary({
        op: 'merge',
        ours: 'main',
        theirs: 'feature/x',
        landed: landed(3, 7, 41, 12)
      })
    ).toEqual({ title: 'Merged feature/x', description: '3 commits · 7 files · +41 −12' })
  })

  /* The case the whole feature exists for: git answered "Already up to date"
     and the panel looks exactly as it does after a merge that brought three
     commits. */
  it('says so when the branch was already in, and where it already is', () => {
    expect(
      writeSummary({ op: 'merge', ours: 'main', theirs: 'feature/x', landed: nothing })
    ).toEqual({ title: 'Nothing to merge', description: 'feature/x is already in main' })
  })
})

describe('what the corner says after a rebase, a pull and a push', () => {
  it('a rebase names what it was replayed onto', () => {
    expect(
      writeSummary({ op: 'rebase', ours: 'feature/x', theirs: 'main', landed: landed(2, 3, 9, 4) })
    ).toEqual({ title: 'Rebased onto main', description: '2 commits · 3 files · +9 −4' })
    expect(
      writeSummary({ op: 'rebase', ours: 'feature/x', theirs: 'main', landed: nothing })
    ).toEqual({
      title: 'Nothing to replay',
      description: 'main has nothing this branch does not'
    })
  })

  it('a pull names the upstream it came from', () => {
    expect(
      writeSummary({ op: 'pull', ours: 'main', theirs: 'origin/main', landed: landed(1, 2, 5, 0) })
    ).toEqual({ title: 'Pulled origin/main', description: '1 commit · 2 files · +5' })
    expect(
      writeSummary({ op: 'pull', ours: 'main', theirs: 'origin/main', landed: nothing })
    ).toEqual({ title: 'Nothing to pull', description: 'main is level with origin/main' })
  })

  it('a push names the upstream it went to', () => {
    expect(
      writeSummary({ op: 'push', ours: 'x', theirs: 'origin/x', landed: landed(4, 6, 20, 3) })
    ).toEqual({ title: 'Pushed to origin/x', description: '4 commits · 6 files · +20 −3' })
    expect(writeSummary({ op: 'push', ours: 'x', theirs: 'origin/x', landed: nothing })).toEqual({
      title: 'Nothing to push',
      description: 'origin/x already has this branch'
    })
  })

  /* A branch nobody had pushed has no upstream to be measured against, so the
     record is empty by construction and there is nothing to count. What
     happened is that the remote has heard of the branch at all. */
  it('a branch with no upstream is published, with nothing counted', () => {
    expect(
      writeSummary({
        op: 'push',
        ours: 'feature/x',
        theirs: null,
        published: true,
        landed: unmeasured
      })
    ).toEqual({ title: 'Published feature/x', description: '' })
  })
})

describe('a measurement that could not be taken', () => {
  /* A repository with no HEAD, a `rev-list` git declined — none of it may turn
     a merge that worked into a merge that says nothing happened. The title
     stands and the counters are simply absent. */
  it('leaves the title standing with nothing under it', () => {
    expect(
      writeSummary({ op: 'merge', ours: 'main', theirs: 'feature/x', landed: unmeasured })
    ).toEqual({ title: 'Merged feature/x', description: '' })
    expect(writeSummary({ op: 'merge', ours: 'main', theirs: 'feature/x' })).toEqual({
      title: 'Merged feature/x',
      description: ''
    })
  })

  /* One unknown number is not evidence of an empty merge, so the phrase falls
     back to the plain title rather than announcing that nothing came in. */
  it('one number missing beside a zero claims neither thing', () => {
    expect(
      writeSummary({
        op: 'merge',
        ours: 'main',
        theirs: 'feature/x',
        landed: landed(0, null, null, null)
      })
    ).toEqual({ title: 'Merged feature/x', description: '' })
    expect(
      writeSummary({
        op: 'merge',
        ours: 'main',
        theirs: 'feature/x',
        landed: landed(null, 0, 0, 0)
      })
    ).toEqual({ title: 'Merged feature/x', description: '' })
  })
})

describe('how the counters are worded', () => {
  it('one of a thing is singular', () => {
    expect(
      writeSummary({ op: 'merge', ours: 'main', theirs: 'feature/x', landed: landed(1, 1, 1, 1) })
    ).toEqual({ title: 'Merged feature/x', description: '1 commit · 1 file · +1 −1' })
  })

  /* git's own `--shortstat` prints only the half that moved, and so does this.
     A `+41 −0` would be a number about nothing. */
  it('the zero half of the line counter falls out', () => {
    expect(
      writeSummary({ op: 'merge', ours: 'main', theirs: 'feature/x', landed: landed(2, 3, 41, 0) })
        .description
    ).toBe('2 commits · 3 files · +41')
    expect(
      writeSummary({ op: 'merge', ours: 'main', theirs: 'feature/x', landed: landed(2, 3, 0, 12) })
        .description
    ).toBe('2 commits · 3 files · −12')
    expect(
      writeSummary({ op: 'merge', ours: 'main', theirs: 'feature/x', landed: landed(2, 3, 0, 0) })
        .description
    ).toBe('2 commits · 3 files')
  })

  /* The minus is U+2212 and never a hyphen, which is the one thing about this
     string a reader cannot check by eye. */
  it('the minus is the one this design system draws', () => {
    const said = writeSummary({
      op: 'merge',
      ours: 'main',
      theirs: 'feature/x',
      landed: landed(1, 1, 1, 2)
    })
    expect(said.description).toContain('−')
    expect(said.description).not.toContain('-')
  })
})

describe('the writes this corner says nothing about', () => {
  /* Every one of these shows its own result in the same moment — the row names
     the new branch, the change list empties, a row appears or goes — so a
     phrase would add nothing. The list lives here and nowhere else. */
  it('answers nothing for every write that is its own report', () => {
    for (const op of ['checkout', 'commit', 'create', 'rename', 'delete', 'abort', 'fetch']) {
      expect(writeSummary({ op, ours: 'main', theirs: 'main', landed: landed(1, 1, 1, 1) })).toBe(
        null
      )
    }
  })

  it('answers nothing when there has been no write at all', () => {
    expect(writeSummary(null)).toBe(null)
    expect(writeSummary(undefined)).toBe(null)
  })

  /* A side with no name is a caller that went round the button: every one of
     the four has a name on screen. */
  it('answers nothing about an operation it cannot name', () => {
    expect(writeSummary({ op: 'merge', ours: 'main', theirs: null, landed: nothing })).toBe(null)
  })
})
