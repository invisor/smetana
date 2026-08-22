import { describe, expect, it } from 'vitest'
import {
  NO_VISIT,
  answeredCount,
  changesVisible,
  enterGitTab,
  gitAnswered,
  toggleChanges
} from '../../../src/components/git/changesFold.js'

/* The three shapes `dirtyCount` in `stores/vcs.js` hands over: a tree with
   something in it, a clean one, and one nobody has managed to read. */
const DIRTY = 6
const CLEAN = 0
const UNREAD = null

describe('what the Changes section is drawn as', () => {
  it('is the stored fold while no visit has forced anything', () => {
    expect(changesVisible(true, NO_VISIT)).toBe(true)
    expect(changesVisible(false, NO_VISIT)).toBe(false)
  })

  /* Every gallery frame hands the panel a hole where the settings should be,
     and `GitPanel`'s own `fold` is what fills it. This file must not turn that
     hole into a folded section on the way past. */
  it('passes a missing stored value through rather than folding it shut', () => {
    expect(changesVisible(undefined, NO_VISIT)).toBe(undefined)
  })

  it('is the override where a visit has one', () => {
    expect(changesVisible(false, { override: true, armed: false })).toBe(true)
  })
})

/* `stores/vcs.js` as this rule reads it: which project the store is about,
   whether it is still mid-load, and its own `dirtyCount`. */
const HERE = '/Users/you/dev/smetana'
const THERE = '/Users/you/dev/notes'
const settled = (over) => ({ project: HERE, loading: false, count: DIRTY, ...over })

describe('which count a visit may be decided from', () => {
  it('is the store\'s own count once it is about this project and settled', () => {
    expect(answeredCount(settled(), HERE)).toBe(DIRTY)
    expect(answeredCount(settled({ count: CLEAN }), HERE)).toBe(CLEAN)
  })

  /* The defect this rule was written for. `moveTo` sets the active project
     synchronously and only reaches `loadRepos` after an awaited layout read,
     and `loadRepos` deliberately leaves the tree standing — so a switch arrives
     here with the departing project's changes still in the store. Believing
     them draws Changes open over an empty list one way and, the other way,
     spends the visit on a clean tree that the arriving project's changes can
     then never open. */
  it('is nothing while the store is still about the project being left', () => {
    expect(answeredCount({ project: THERE, loading: false, count: DIRTY }, HERE)).toBe(null)
  })

  /* The narrower window behind the same defect, and it is not hypothetical:
     `loadRepos` claims the new project before its first `await` and holds
     `loading` across `vcs_repos` → `selectRepo` → `loadStatus`, so the project
     matches here while the tree in hand is still the previous one. Guarded on
     the project alone, this case fails exactly as the unguarded version does. */
  it('is nothing while the store is mid-load, even about the right project', () => {
    expect(answeredCount(settled({ loading: true }), HERE)).toBe(null)
  })

  /* A read that failed leaves `vcsState.tree` at `null`, which reaches this as
     a `null` count — the `null`-and-never-`0` opposition the store keeps. Not
     knowing is not a clean tree, so the visit is owed its opening still. */
  it('is nothing when the tree could not be read', () => {
    expect(answeredCount(settled({ count: null }), HERE)).toBe(null)
    expect(enterGitTab(answeredCount(settled({ count: null }), HERE)).armed).toBe(true)
  })

  /* Before anything has been read at all — the app starting with Git already
     the open tab, where the store is about no project yet. */
  it('is nothing before the store is about any project', () => {
    expect(answeredCount({ project: null, loading: false, count: null }, HERE)).toBe(null)
  })

  /* End to end over the switch, which is what the three above add up to: the
     visit arms instead of spending itself, and the arriving project's own
     answer is what opens the section. */
  it('arms the visit on a switch, and the arriving answer settles it', () => {
    const waiting = enterGitTab(answeredCount({ project: THERE, loading: true, count: CLEAN }, HERE))
    expect(changesVisible(false, waiting)).toBe(false)
    expect(changesVisible(false, gitAnswered(waiting, DIRTY))).toBe(true)
  })
})

describe('arriving on the Git tab', () => {
  /* The whole of what was asked for: come back to the tab with uncommitted
     work in the tree and the list is on screen, whatever was folded away. */
  it('opens the section when the tree has changes', () => {
    const visit = enterGitTab(DIRTY)
    expect(changesVisible(false, visit)).toBe(true)
    expect(visit.armed).toBe(false)
  })

  /* A clean tree owes the visit nothing: there is no list to come back to, so
     the preference stands as it was left, open or folded. */
  it('leaves the stored fold alone when the tree is clean', () => {
    const visit = enterGitTab(CLEAN)
    expect(changesVisible(false, visit)).toBe(false)
    expect(changesVisible(true, visit)).toBe(true)
  })

  /* The ordinary case rather than the edge one: the tab is on screen before
     `vcs_status` comes back, so the visit waits instead of answering from a
     tree nobody has read. */
  it('opens nothing while the tree is still unknown, and opens when it lands', () => {
    const waiting = enterGitTab(UNREAD)
    expect(changesVisible(false, waiting)).toBe(false)
    expect(waiting.armed).toBe(true)

    const answered = gitAnswered(waiting, DIRTY)
    expect(changesVisible(false, answered)).toBe(true)
  })

  /* A read that failed is not a clean tree — `dirtyCount` is `null` and never
     `0` for exactly this reason — so the visit is still owed its opening. */
  it('stays armed through a read that answered nothing', () => {
    expect(gitAnswered(enterGitTab(UNREAD), UNREAD).armed).toBe(true)
  })
})

describe('the answers after the first one', () => {
  /* Window focus and the panel's refresh button both re-read the tree under
     somebody already sitting on the tab. Neither may unfold what they folded a
     moment ago. */
  it('leave a folded section folded once the visit is spent', () => {
    const visit = gitAnswered(enterGitTab(DIRTY), DIRTY)
    const folded = toggleChanges(true, visit)
    expect(changesVisible(folded.changesOpen, gitAnswered(folded.visit, DIRTY))).toBe(false)
  })

  /* The same rule from the other side: a visit that arrived on a clean tree got
     nothing and is spent, so work appearing under it changes nothing on screen
     until somebody leaves the tab and comes back. */
  it('do not open a section when a clean tree goes dirty mid-visit', () => {
    const visit = gitAnswered(enterGitTab(CLEAN), DIRTY)
    expect(changesVisible(false, visit)).toBe(false)
  })

  /* And a refresh under a forced-open section does not take the override away
     either: the visit is the same visit. */
  it('leave a forced-open section open', () => {
    const visit = gitAnswered(enterGitTab(DIRTY), CLEAN)
    expect(changesVisible(false, visit)).toBe(true)
  })
})

describe('a press on the Changes caption', () => {
  /* The case the override created: what is stored is `false`, what is on
     screen is open, and folding it must happen on the first press rather than
     writing `true` and folding nothing. */
  it('folds a forced-open section on the first press and stores that', () => {
    const visit = enterGitTab(DIRTY)
    const pressed = toggleChanges(false, visit)
    expect(pressed.changesOpen).toBe(false)
    expect(changesVisible(pressed.changesOpen, pressed.visit)).toBe(false)
  })

  it('is an ordinary inversion with no visit forcing anything', () => {
    expect(toggleChanges(true, NO_VISIT).changesOpen).toBe(false)
    expect(toggleChanges(false, NO_VISIT).changesOpen).toBe(true)
  })

  /* A `vcs_status` still in flight must not reopen the section behind the
     press: the visit is spent by the press, not only stripped of its override. */
  it('drops a pending arm as well as the override', () => {
    const pressed = toggleChanges(true, enterGitTab(UNREAD))
    expect(pressed.changesOpen).toBe(false)
    expect(changesVisible(pressed.changesOpen, gitAnswered(pressed.visit, DIRTY))).toBe(false)
  })
})
