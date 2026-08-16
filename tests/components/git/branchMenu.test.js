import { describe, expect, it } from 'vitest'
import { branchMenuItems } from '../../../src/components/git/branchMenu.js'

const verbs = (items) => items.filter((it) => !it.type)
const caption = (items) => items.find((it) => it.type === 'label')?.label ?? null

describe('branchMenuItems', () => {
  it('offers the three things a branch row can do, in the order the row learnt them', () => {
    expect(verbs(branchMenuItems()).map((it) => it.kind)).toEqual(['checkout', 'merge', 'rebase'])
  })

  it('says nothing about a refusal when there is none', () => {
    const items = branchMenuItems()
    expect(caption(items)).toBe(null)
    expect(verbs(items).some((it) => it.disabled)).toBe(false)
  })

  it('keeps the two writes apart from the switch', () => {
    const items = branchMenuItems()
    const at = items.findIndex((it) => it.type === 'separator')
    expect(items[at - 1].kind).toBe('checkout')
    expect(items[at + 1].kind).toBe('merge')
  })

  /* Every item carries an icon, because a menu row without one leaves a hole in
     the gutter every other row fills. */
  it('names a glyph for every verb', () => {
    expect(verbs(branchMenuItems()).map((it) => it.icon)).toEqual([
      'git-branch',
      'git-merge',
      'git-graph'
    ])
  })

  it('greys the whole menu on the branch already checked out', () => {
    const items = branchMenuItems({ current: true })
    expect(caption(items)).toBe('Already on this branch')
    expect(verbs(items).every((it) => it.disabled)).toBe(true)
  })

  it('greys the whole menu while a run holds the repository', () => {
    const items = branchMenuItems({ allowed: false })
    expect(caption(items)).toBe('A run is going in this project')
    expect(verbs(items).every((it) => it.disabled)).toBe(true)
  })

  it('greys the whole menu while git is already working', () => {
    const items = branchMenuItems({ busy: true })
    expect(caption(items)).toBe('Git is working in this repository')
    expect(verbs(items).every((it) => it.disabled)).toBe(true)
  })

  /* The row with the tick answers about itself. A run blocks the other rows
     too, and those are the ones that say so. */
  it('answers about the row rather than about the run', () => {
    expect(caption(branchMenuItems({ current: true, allowed: false, busy: true }))).toBe(
      'Already on this branch'
    )
  })

  it('says a run before it says git is busy, since the run is the longer wait', () => {
    expect(caption(branchMenuItems({ allowed: false, busy: true }))).toBe(
      'A run is going in this project'
    )
  })
})
