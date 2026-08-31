import { describe, expect, it } from 'vitest'
import { canRestore, draftIsEmpty } from '../../../src/components/kanban/taskDraft.js'

const draft = (over = {}) => ({
  project: '/work/app',
  text: 'The tree loses its scroll position',
  issue_type: null,
  priority: null,
  brainstorm: 'auto',
  spec: 'auto',
  plan: 'auto',
  images: [],
  parent: null,
  ...over
})

describe('draftIsEmpty', () => {
  it('a window nobody typed into is not worth keeping', () => {
    expect(draftIsEmpty(draft({ text: '   ' }))).toBe(true)
    expect(draftIsEmpty(null)).toBe(true)
  })

  it('a picture alone is worth keeping, since attaching one is work too', () => {
    expect(draftIsEmpty(draft({ text: '', images: ['/data/attachments/a/shot.png'] }))).toBe(false)
  })

  it('typed text is worth keeping', () => {
    expect(draftIsEmpty(draft())).toBe(false)
  })
})

describe('canRestore', () => {
  const world = {
    project: '/work/app',
    columns: ['ready', 'done'],
    column: 'ready',
    boardArrived: true
  }

  it('puts a draft back in the project it was written for', () => {
    expect(canRestore(draft(), world)).toBe(true)
  })

  it('keeps another project’s draft off this board', () => {
    expect(canRestore(draft({ project: '/work/other' }), world)).toBe(false)
  })

  /* A project switch does not deliver a board at once. Restoring before the
     columns land would open a window the staleness watcher closes on the spot,
     with a second notice about a window the person never saw. */
  it('waits until the board carries the column the card would land in', () => {
    expect(canRestore(draft(), { ...world, columns: [] })).toBe(false)
    expect(canRestore(draft(), { ...world, columns: ['done'] })).toBe(false)
  })

  it('has nothing to put back when the draft is empty or missing', () => {
    expect(canRestore(draft({ text: '' }), world)).toBe(false)
    expect(canRestore(null, world)).toBe(false)
  })

  /* Nowhere to put it back into. The app window asks this on every pass of the
     ground watcher, including the ones where no project is open at all. */
  it('has nowhere to put one back when no project is open', () => {
    expect(canRestore(draft(), { ...world, project: null })).toBe(false)
  })

  /* The board's columns reach this rule as a list here and as a `Set` in the
     watcher next door, and the two objects are near enough alike to be handed
     to the wrong one. Read as a list, a `Set` says the column is missing and
     the window never comes back — with nothing on screen and nothing in any
     gate to say why. */
  it('reads a set of columns as readily as a list of them', () => {
    expect(canRestore(draft(), { ...world, columns: new Set(['ready', 'done']) })).toBe(true)
    expect(canRestore(draft(), { ...world, columns: new Set(['done']) })).toBe(false)
  })

  /* The trap the columns check alone walks into. Between the click and the new
     board there are seconds in which the active project is the new one and the
     columns are still the old one's — and the old one's `ready` would answer
     yes on behalf of a board nobody is looking at any more. */
  it('will not read the previous project’s leftover columns as this one’s board', () => {
    expect(canRestore(draft(), { ...world, boardArrived: false })).toBe(false)
  })
})
