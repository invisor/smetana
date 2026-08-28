import { describe, expect, it } from 'vitest'
import { COPIED_MS, copyLabel } from '../../../src/components/kanban/copyId.js'

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

  /* "Anything" includes the names every object inherits, which a plain object
     literal would answer with a function or with its prototype — the fallback
     is nullish-only, so an inherited key is never reached. Unreachable from the
     three producers of this state, and pinned here because the sentence above
     is stated about anything at all. */
  it('holds for the names an object inherits, which it does not have', () => {
    expect(copyLabel('constructor')).toBe('Copy id')
    expect(copyLabel('__proto__')).toBe('Copy id')
    expect(copyLabel('toString')).toBe('Copy id')
    expect(copyLabel('valueOf')).toBe('Copy id')
    expect(copyLabel('hasOwnProperty')).toBe('Copy id')
  })
})

describe('how long a copy says it worked for', () => {
  /* One number for every copy in the app, and it was three before this module
     took it: a `COPIED_ID_MS` in `views/DesktopApp.vue`, a bare `1200` in
     `views/Gallery.vue`, and a third in the session menu's rule. The gallery is
     this project's only verification of anything under `src/components/`, so a
     duration that moved in the app alone would leave the harness confirming a
     copy at a speed the product no longer uses. */
  it('is one number, long enough to read and short enough not to queue', () => {
    expect(COPIED_MS).toBe(1200)
  })
})
