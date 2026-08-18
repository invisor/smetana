import { describe, expect, it } from 'vitest'
import { folderBehind, pullAction, pushAction, trackingMark } from '../../../src/components/git/tracking.js'

const free = { allowed: true, reason: null }
const held = { allowed: false, reason: 'A run is going in this project.' }
const behind = { branch: 'main', upstream: 'origin/main', ahead: 0, behind: 3, gone: false }
const ahead = { branch: 'main', upstream: 'origin/main', ahead: 2, behind: 0, gone: false }
const both = { branch: 'main', upstream: 'origin/main', ahead: 2, behind: 3, gone: false }
const level = { branch: 'main', upstream: 'origin/main', ahead: 0, behind: 0, gone: false }
const orphan = { branch: 'spike', upstream: null, ahead: 0, behind: 0, gone: false }
const gone = { branch: 'old', upstream: 'origin/old', ahead: 0, behind: 0, gone: true }

describe('what a branch row draws for its upstream', () => {
  /* The whole of what was asked for: commits waiting in the upstream, and
     nothing else, is what turns a row orange. */
  it('only a branch that is behind is orange', () => {
    expect(trackingMark(behind).orange).toBe(true)
    expect(trackingMark(both).orange).toBe(true)
    expect(trackingMark(ahead).orange).toBe(false)
    expect(trackingMark(level).orange).toBe(false)
  })

  /* Neither is something to pull: one has never had an upstream, the other has
     lost it. A colour on either would send somebody to a button that refuses. */
  it('no upstream and a deleted upstream are not orange', () => {
    expect(trackingMark(orphan).orange).toBe(false)
    expect(trackingMark(gone).orange).toBe(false)
  })

  /* A branch the tracking read has not answered for — the two lists are
     merged by name and can be one refresh apart. */
  it('a branch with no record draws nothing at all', () => {
    expect(trackingMark(undefined)).toEqual({ behind: 0, ahead: 0, orange: false })
  })

  /* Both marks on one row, and only one of them colours it: what was asked for
     is a branch with something to pull, and a row orange for either would leave
     the two indistinguishable at a glance. */
  it('a branch both ahead and behind carries both counts', () => {
    expect(trackingMark(both)).toEqual({ behind: 3, ahead: 2, orange: true })
  })
})

describe('what a folded folder stands in for', () => {
  const branches = [
    { name: 'main', current: true },
    { name: 'fix/legacy/depot-import', current: false },
    { name: 'feature/one', current: false }
  ]
  const tracking = {
    'fix/legacy/depot-import': {
      branch: 'fix/legacy/depot-import',
      upstream: 'origin/fix/legacy/depot-import',
      ahead: 0,
      behind: 2,
      gone: false
    },
    'feature/one': { branch: 'feature/one', upstream: 'origin/feature/one', ahead: 4, behind: 0, gone: false }
  }

  /* The mark reaches every heading above the branch it is about, however deep
     the fold is closed: a list folded at `fix` hides the row entirely, and that
     is the case this exists for. */
  it('a heading answers for everything below it, at any depth', () => {
    expect(folderBehind('fix', branches, tracking)).toBe(true)
    expect(folderBehind('fix/legacy', branches, tracking)).toBe(true)
  })

  /* A folder holding only branches that are ahead has nothing to pull, and a
     mark on it would send somebody to a button that refuses. */
  it('a folder with nothing behind it draws nothing', () => {
    expect(folderBehind('feature', branches, tracking)).toBe(false)
  })

  /* The separator is the slash, so a folder is never confused with a branch
     whose name merely starts the same way. */
  it('a folder is its own path and a slash, never a prefix of a name', () => {
    const named = [{ name: 'fixture', current: false }]
    const marked = { fixture: { branch: 'fixture', upstream: 'origin/fixture', ahead: 0, behind: 1, gone: false } }

    expect(folderBehind('fix', named, marked)).toBe(false)
  })

  /* A repository with no tracking answer yet — the two lists are merged by
     name, and a heading is drawn before the counts land. */
  it('no tracking at all marks no folder', () => {
    expect(folderBehind('fix', branches, {})).toBe(false)
  })
})

describe('the two buttons in the section header', () => {
  it('pull carries the behind count and push the ahead count', () => {
    expect(pullAction(behind, free).count).toBe(3)
    expect(pushAction(ahead, free).count).toBe(2)
  })

  /* Pressing either while a run is going is what `gitActions.js` refuses, and
     its sentence is the one that must arrive — not a second copy written
     here. */
  it('a run refuses both, in the run rule own words', () => {
    expect(pullAction(behind, held)).toMatchObject({ allowed: false, reason: held.reason })
    expect(pushAction(ahead, held)).toMatchObject({ allowed: false, reason: held.reason })
  })

  /* There is nothing to pull into a branch that has no upstream, and the
     sentence has to say which of the two reasons it is. */
  it('pull is refused with its own sentence when there is no upstream', () => {
    expect(pullAction(orphan, free).allowed).toBe(false)
    expect(pullAction(orphan, free).reason).toMatch(/upstream/i)
    expect(pullAction(gone, free).reason).toMatch(/deleted|gone/i)
  })

  /* The ordinary state of a branch cut in this panel, and the one case where
     push runs a different command and says a different word. */
  it('push publishes a branch that has no upstream', () => {
    expect(pushAction(orphan, free)).toMatchObject({
      allowed: true,
      label: 'Publish branch',
      setUpstream: true
    })
    expect(pushAction(gone, free).setUpstream).toBe(true)
  })

  /* Nothing to send is not a refusal to explain — it is a button with nothing
     to do, and the panel simply does not draw it. */
  it('push is refused when the branch is level with its upstream', () => {
    expect(pushAction(level, free).allowed).toBe(false)
  })

  /* A pull on a branch that is only ahead still runs: fetching and finding
     nothing is a legitimate thing to ask for, and the count is what says so. */
  it('pull stays live on a branch with nothing to pull', () => {
    expect(pullAction(level, free).allowed).toBe(true)
    expect(pullAction(level, free).count).toBe(0)
  })

  /* The counts are on the labels, because a button that says how much it is
     about to move is the one thing on screen saying the marks and the verbs are
     the same fact. */
  it('the counts reach the labels', () => {
    expect(pullAction(behind, free).label).toBe('Pull 3')
    expect(pushAction(ahead, free).label).toBe('Push 2')
    expect(pullAction(level, free).label).toBe('Pull')
  })

  /* A tracking read that has not landed — the branch list arrives first, and
     the header is drawn from what is known. Push publishes, since a branch this
     app knows no upstream for is one `git push` alone would refuse. */
  it('a branch with no record at all still answers for both buttons', () => {
    expect(pullAction(undefined, free).allowed).toBe(false)
    expect(pushAction(undefined, free)).toMatchObject({ allowed: true, setUpstream: true })
  })
})
