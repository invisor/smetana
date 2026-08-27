import { describe, expect, it } from 'vitest'
import { copyLabel } from '../../../src/components/kanban/copyId.js'

describe('what the id says about being copied', () => {
  /* Three strings, and they are three of this feature's acceptance criteria.
     They live in one place because the card and the inspector both draw an id
     and neither can be reached by a test — two copies of these words could
     drift apart with everything green. */
  it('invites the click before anything has been asked', () => {
    expect(copyLabel('')).toBe('Copy id')
  })

  it('confirms a copy that worked', () => {
    expect(copyLabel('copied')).toBe('Copied')
  })

  it('says so when the clipboard refused, in the same panel rather than a toast', () => {
    expect(copyLabel('failed')).toBe('Could not copy')
  })

  /* The state arrives as a prop with a default of `''`, so undefined is the
     ordinary case rather than an error, and a state nobody has heard of is
     better answered with the invitation than with a blank panel. */
  it('falls back to the invitation for anything it has never heard of', () => {
    expect(copyLabel(undefined)).toBe('Copy id')
    expect(copyLabel(null)).toBe('Copy id')
    expect(copyLabel('COPIED')).toBe('Copy id')
    expect(copyLabel('pending')).toBe('Copy id')
  })
})
