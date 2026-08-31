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
      'review',
      'favorite',
      'copy-name',
      'merge',
      'rebase',
      'new-branch',
      'rename',
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
    expect(items.slice(0, at).map((it) => it.kind)).toEqual([
      'checkout',
      'compare',
      'review',
      'favorite',
      'copy-name'
    ])
    expect(items[at + 1].kind).toBe('merge')
  })

  /* Every item carries an icon, because a menu row without one leaves a hole in
     the gutter every other row fills. */
  it('names a glyph for every verb', () => {
    expect(verbs(branchMenuItems()).map((it) => it.icon)).toEqual([
      'git-branch',
      'git-compare',
      'search-check',
      'star',
      'copy',
      'git-merge',
      'git-graph',
      'git-branch-plus',
      'pencil',
      'trash-2'
    ])
  })

  it('offers the comparison beside the switch', () => {
    const kinds = verbs(branchMenuItems()).map((it) => it.kind)
    expect(kinds.slice(0, 2)).toEqual(['checkout', 'compare'])
  })

  /* Compare shows and Review judges: two rows in one group, both readers, and
     the review directly under the comparison because that is where somebody
     looking for "what did this branch do" will already be looking. */
  it('puts the review directly under the comparison', () => {
    const kinds = verbs(branchMenuItems()).map((it) => it.kind)
    expect(kinds.slice(1, 3)).toEqual(['compare', 'review'])
  })

  /* The one ellipsis in this app, and it is a deliberate exception rather than
     a slip: every other row of this menu is over in a second, and this one
     opens a form and then starts an agent. `branchMenu.js`'s own note says so,
     which is what stops it being levelled away by somebody tidying. */
  it('carries the one ellipsis this app spends', () => {
    const labels = verbs(branchMenuItems()).map((it) => it.label)
    expect(labels.filter((label) => label.endsWith('…'))).toEqual(['Review this branch…'])
  })

  /* The fifth reach of refusal, and it shares the fourth's: nothing refuses it.
     It reads, it writes only inside `.smetana/` and it takes no git lock — so
     not a run, not an operation in flight, and not the row being the branch
     already checked out. That last is where it parts company with the
     comparison directly above it. */
  it('leaves the review live in every state there is', () => {
    for (const at of [
      {},
      { current: true },
      { allowed: false },
      { busy: true },
      { allowed: false, busy: true },
      { current: true, allowed: false, busy: true }
    ]) {
      const item = verbs(branchMenuItems(at)).find((it) => it.kind === 'review')
      expect(item.disabled).toBe(false)
    }
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
    /* The group above it is the two items that make a branch or change what it
       answers to, and the rename is the nearer of them. */
    expect(items[items.length - 3].kind).toBe('rename')
    expect(items[items.length - 4].kind).toBe('new-branch')
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
      'rename',
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
  it('puts the new branch and the rename in a group of their own near the end', () => {
    const items = branchMenuItems()
    const at = items.findIndex((it) => it.kind === 'new-branch')
    expect(items[at - 1].type).toBe('separator')
    expect(items[at + 1].kind).toBe('rename')
    expect(items[at + 2].type).toBe('separator')
  })

  it('greys the whole menu while git is already working', () => {
    const items = branchMenuItems({ busy: true })
    expect(caption(items)).toBe('Git is working in this repository')
    expect(disabledKinds(items)).toEqual([
      'checkout',
      'merge',
      'rebase',
      'new-branch',
      'rename',
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
    /* Every verb but the two a run has no claim on: the review, which reads and
       writes only inside `.smetana/`, and the favourite, which writes a
       preference and asks git for nothing. */
    expect(verbs(items).filter((it) => !it.disabled).map((it) => it.kind)).toEqual([
      'review',
      'favorite',
      'copy-name'
    ])
  })

  it('says a run before it says git is busy, since the run is the longer wait', () => {
    expect(caption(branchMenuItems({ allowed: false, busy: true }))).toBe(
      'A run is going in this project'
    )
  })

  /* The one item here that reaches neither git nor `settings.json`. It sits
     beside the favourite because that is the other item nothing refuses, and
     because both are about the row rather than about the repository. */
  it('offers the copy beside the favourite', () => {
    const kinds = verbs(branchMenuItems()).map((it) => it.kind)
    expect(kinds.slice(3, 5)).toEqual(['favorite', 'copy-name'])
    expect(verbs(branchMenuItems()).find((it) => it.kind === 'copy-name').label).toBe(
      'Copy branch name'
    )
  })

  /* Nothing refuses it: it writes nowhere at all, so not a run, not an
     operation in flight, and not the row being the branch already checked out —
     the name of the branch you are standing on is as copyable as any other. */
  it('leaves the copy live in every state there is', () => {
    for (const at of [
      {},
      { current: true },
      { allowed: false },
      { busy: true },
      { current: true, allowed: false, busy: true }
    ]) {
      const item = verbs(branchMenuItems(at)).find((it) => it.kind === 'copy-name')
      expect(item.disabled).toBe(false)
    }
  })

  it('names the rename directly under the new branch', () => {
    const kinds = verbs(branchMenuItems()).map((it) => it.kind)
    expect(kinds.slice(-3, -1)).toEqual(['new-branch', 'rename'])
    expect(verbs(branchMenuItems()).find((it) => it.kind === 'rename').label).toBe(
      'Rename this branch'
    )
  })

  /* **The row with the tick can be renamed**, which is where it parts company
     with the delete two rows down: `git branch -m` renames the branch HEAD is
     on and HEAD travels with the ref, so a typo in the name of the branch
     somebody is working in is the ordinary case. It is refused by `held` and by
     nothing else, exactly like the new branch beside it. */
  it('leaves the rename live on the branch already checked out', () => {
    expect(disabledKinds(branchMenuItems({ current: true }))).not.toContain('rename')
  })

  it('refuses the rename while a run is going and while git is working', () => {
    expect(disabledKinds(branchMenuItems({ allowed: false }))).toContain('rename')
    expect(disabledKinds(branchMenuItems({ busy: true }))).toContain('rename')
  })
})
