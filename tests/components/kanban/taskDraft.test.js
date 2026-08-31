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
  const world = { project: '/work/app', columns: ['ready', 'done'], column: 'ready' }

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
})
