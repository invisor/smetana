import { describe, expect, it } from 'vitest'
import { phoneLook } from '../src/look.js'

describe('what the phone is drawn in', () => {
  it('resolves the desktop\'s system theme by the phone\'s own', () => {
    expect(phoneLook({ theme: 'system', density: 'compact' }, true, null)).toEqual({ theme: 'dark', density: 'compact' })
    expect(phoneLook({ theme: 'system', density: 'compact' }, false, null)).toEqual({ theme: 'light', density: 'compact' })
  })

  it('takes dark and light as they are, whatever the system says', () => {
    expect(phoneLook({ theme: 'light', density: 'comfortable' }, true, null).theme).toBe('light')
    expect(phoneLook({ theme: 'dark', density: 'comfortable' }, false, null).theme).toBe('dark')
  })

  it('reads an unknown theme or density as system and comfortable', () => {
    expect(phoneLook({ theme: 'neon', density: 7 }, false, null)).toEqual({ theme: 'light', density: 'comfortable' })
    expect(phoneLook({ theme: 'neon', density: 'roomy' }, true, null)).toEqual({ theme: 'dark', density: 'comfortable' })
    expect(phoneLook(undefined, true, null)).toEqual({ theme: 'dark', density: 'comfortable' })
  })

  it('lets the page\'s query win, for the browser', () => {
    const params = new URLSearchParams('theme=light&density=compact')
    expect(phoneLook({ theme: 'dark', density: 'comfortable' }, true, params)).toEqual({ theme: 'light', density: 'compact' })
  })

  it('ignores a query it does not know', () => {
    const params = new URLSearchParams('theme=system&density=wide')
    expect(phoneLook({ theme: 'light', density: 'compact' }, true, params)).toEqual({ theme: 'light', density: 'compact' })
  })
})
