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
      'merge',
      'rebase',
      'new-branch'
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
    expect(items.slice(0, at).map((it) => it.kind)).toEqual(['checkout', 'compare'])
    expect(items[at + 1].kind).toBe('merge')
  })

  /* Every item carries an icon, because a menu row without one leaves a hole in
     the gutter every other row fills. */
  it('names a glyph for every verb', () => {
    expect(verbs(branchMenuItems()).map((it) => it.icon)).toEqual([
      'git-branch',
      'git-compare',
      'git-merge',
      'git-graph',
      'git-branch-plus'
    ])
  })

  it('offers the comparison beside the switch', () => {
    expect(verbs(branchMenuItems()).map((it) => it.kind)).toEqual([
      'checkout',
      'compare',
      'merge',
      'rebase',
      'new-branch'
    ])
    expect(verbs(branchMenuItems()).map((it) => it.icon)).toEqual([
      'git-branch',
      'git-compare',
      'git-merge',
      'git-graph',
      'git-branch-plus'
    ])
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
    expect(disabledKinds(items)).toEqual(['checkout', 'compare', 'merge', 'rebase'])
  })

  /* Every verb that writes, which since the comparison arrived is every verb
     but that one — the caption above it says "not now" about the rest. */
  it('greys the whole menu while a run holds the repository', () => {
    const items = branchMenuItems({ allowed: false })
    expect(caption(items)).toBe('A run is going in this project')
    expect(disabledKinds(items)).toEqual(['checkout', 'merge', 'rebase', 'new-branch'])
  })

  it('keeps the new branch live on the row nothing else can be done from', () => {
    const items = branchMenuItems({ current: true })
    expect(verbs(items).find((it) => it.kind === 'new-branch').disabled).toBe(false)
  })

  /* Last, and behind a separator of its own: it is the only item that leaves
     the list longer than it found it, and the separator is also what says how
     far the caption above the greyed rows reaches. */
  it('puts the new branch in a group of its own at the end', () => {
    const items = branchMenuItems()
    expect(items[items.length - 1].kind).toBe('new-branch')
    expect(items[items.length - 2].type).toBe('separator')
  })

  it('greys the whole menu while git is already working', () => {
    const items = branchMenuItems({ busy: true })
    expect(caption(items)).toBe('Git is working in this repository')
    expect(disabledKinds(items)).toEqual(['checkout', 'merge', 'rebase', 'new-branch'])
  })

  /* A caption reaches exactly as far as the greying under it, so on the current
     branch under a run it has to be the run's sentence: that is the fact
     refusing the last row too, and "already on this branch" would leave the one
     item it does not explain greyed with nothing above it that fits. */
  it('says the run on the current branch, since the run is what refuses the lot', () => {
    const items = branchMenuItems({ current: true, allowed: false, busy: true })
    expect(caption(items)).toBe('A run is going in this project')
    expect(verbs(items).every((it) => it.disabled)).toBe(true)
  })

  it('says a run before it says git is busy, since the run is the longer wait', () => {
    expect(caption(branchMenuItems({ allowed: false, busy: true }))).toBe(
      'A run is going in this project'
    )
  })
})
