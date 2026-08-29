import { describe, expect, it } from 'vitest'
import { branchMenuItems } from '../../../src/components/git/branchMenu.js'

const verbs = (items) => items.filter((it) => !it.type)
const caption = (items) => items.find((it) => it.type === 'label')?.label ?? null
const disabledKinds = (items) => verbs(items).filter((it) => it.disabled).map((it) => it.kind)

describe('branchMenuItems', () => {
  it('offers what a branch row can do, in the order the row learnt it', () => {
    expect(verbs(branchMenuItems()).map((it) => it.kind)).toEqual([
      'checkout',
      'compare',
      'favorite',
      'merge',
      'rebase',
      'new-branch',
      'delete'
    ])
  })

  it('says nothing about a refusal when there is none', () => {
    const items = branchMenuItems()
    expect(caption(items)).toBe(null)
    expect(verbs(items).some((it) => it.disabled)).toBe(false)
  })

  it('keeps the two writes apart from the switch', () => {
    const items = branchMenuItems()
    const at = items.findIndex((it) => it.type === 'separator')
    expect(items.slice(0, at).map((it) => it.kind)).toEqual(['checkout', 'compare', 'favorite'])
    expect(items[at + 1].kind).toBe('merge')
  })

  /* Every item carries an icon, because a menu row without one leaves a hole in
     the gutter every other row fills. */
  it('names a glyph for every verb', () => {
    expect(verbs(branchMenuItems()).map((it) => it.icon)).toEqual([
      'git-branch',
      'git-compare',
      'star',
      'git-merge',
      'git-graph',
      'git-branch-plus',
      'trash-2'
    ])
  })

  it('offers the comparison beside the switch', () => {
    const kinds = verbs(branchMenuItems()).map((it) => it.kind)
    expect(kinds.slice(0, 2)).toEqual(['checkout', 'compare'])
  })

  /* The one item in this menu that asks git for nothing: it writes a line in
     `settings.json` and moves a row up the list. Its label is the act and not
     the state, so a row already marked offers the way back out — which is the
     whole of what tells somebody the mark is theirs to remove. */
  it('names the favourite by what a press does rather than by what is true', () => {
    const of = (favorite) =>
      verbs(branchMenuItems({ favorite })).find((it) => it.kind === 'favorite').label
    expect(of(false)).toBe('Add to favourites')
    expect(of(true)).toBe('Remove from favourites')
  })

  /* The fourth reach of refusal in this file, and the narrowest: nothing at all
     refuses it. The comparison beside it still reads the repository, so the
     current row refuses that one; this writes a preference and reads nothing. */
  it('leaves the favourite live in every state there is', () => {
    for (const at of [
      {},
      { current: true },
      { allowed: false },
      { busy: true },
      { current: true, allowed: false, busy: true }
    ]) {
      expect(disabledKinds(branchMenuItems(at))).not.toContain('favorite')
    }
  })

  /* Last, behind a separator of its own, and the only item here that loses
     work. Its own group rather than beside `New branch from this`: the two are
     refused differently — cutting a branch from where you are standing is the
     ordinary case and deleting where you are standing is impossible — so one
     group holding both would grey half of itself. */
  it('puts the delete last, in a group of its own', () => {
    const items = branchMenuItems()
    expect(items[items.length - 1].kind).toBe('delete')
    expect(items[items.length - 2].type).toBe('separator')
    expect(items[items.length - 3].kind).toBe('new-branch')
  })

  it('refuses the delete on the branch already checked out', () => {
    expect(disabledKinds(branchMenuItems({ current: true }))).toContain('delete')
  })

  it('refuses the delete while a run is going and while git is working', () => {
    expect(disabledKinds(branchMenuItems({ allowed: false }))).toContain('delete')
    expect(disabledKinds(branchMenuItems({ busy: true }))).toContain('delete')
  })

  it('leaves the delete live on any branch the repository is not on', () => {
    expect(disabledKinds(branchMenuItems())).not.toContain('delete')
  })

  /* It reads and writes nothing, so neither a run nor an operation in flight
     has anything to refuse. */
  it('offers the comparison while a run is going and while git is working', () => {
    expect(disabledKinds(branchMenuItems({ allowed: false }))).not.toContain('compare')
    expect(disabledKinds(branchMenuItems({ busy: true }))).not.toContain('compare')
  })

  /* There is nothing to compare a branch with itself. */
  it('refuses the comparison on the branch already checked out', () => {
    expect(disabledKinds(branchMenuItems({ current: true }))).toContain('compare')
  })

  /* The one refusal that is about the row rather than about the moment: it
     reaches the three verbs about moving between branches, and the comparison
     beside them since a branch has no difference from itself to draw, and stops
     there. A branch cut from where you are standing is the ordinary case. */
  it('greys only the moving verbs on the branch already checked out', () => {
    const items = branchMenuItems({ current: true })
    expect(caption(items)).toBe('Already on this branch')
    expect(disabledKinds(items)).toEqual([
      'checkout',
      'compare',
      'merge',
      'rebase',
      'delete'
    ])
  })

  /* Every verb that writes, which since the comparison arrived is every verb
     but that one — the caption above it says "not now" about the rest. */
  it('greys the whole menu while a run holds the repository', () => {
    const items = branchMenuItems({ allowed: false })
    expect(caption(items)).toBe('A run is going in this project')
    expect(disabledKinds(items)).toEqual([
      'checkout',
      'merge',
      'rebase',
      'new-branch',
      'delete'
    ])
  })

  it('keeps the new branch live on the row nothing else can be done from', () => {
    const items = branchMenuItems({ current: true })
    expect(verbs(items).find((it) => it.kind === 'new-branch').disabled).toBe(false)
  })

  /* Behind a separator of its own: it is the only item that leaves the list
     longer than it found it, and the separator is also what says how far the
     caption above the greyed rows reaches. Second from the bottom now that the
     delete has the last group. */
  it('puts the new branch in a group of its own near the end', () => {
    const items = branchMenuItems()
    const at = items.findIndex((it) => it.kind === 'new-branch')
    expect(items[at - 1].type).toBe('separator')
    expect(items[at + 1].type).toBe('separator')
  })

  it('greys the whole menu while git is already working', () => {
    const items = branchMenuItems({ busy: true })
    expect(caption(items)).toBe('Git is working in this repository')
    expect(disabledKinds(items)).toEqual([
      'checkout',
      'merge',
      'rebase',
      'new-branch',
      'delete'
    ])
  })

  /* A caption reaches exactly as far as the greying under it, so on the current
     branch under a run it has to be the run's sentence: that is the fact
     refusing the last row too, and "already on this branch" would leave the one
     item it does not explain greyed with nothing above it that fits. */
  it('says the run on the current branch, since the run is what refuses the lot', () => {
    const items = branchMenuItems({ current: true, allowed: false, busy: true })
    expect(caption(items)).toBe('A run is going in this project')
    /* Every verb but the favourite, which is the one thing here a run has no
       claim on: it writes a preference and asks git for nothing. */
    expect(verbs(items).filter((it) => !it.disabled).map((it) => it.kind)).toEqual(['favorite'])
  })

  it('says a run before it says git is busy, since the run is the longer wait', () => {
    expect(caption(branchMenuItems({ allowed: false, busy: true }))).toBe(
      'A run is going in this project'
    )
  })
})
