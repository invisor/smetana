import { describe, expect, it } from 'vitest'
import {
  COLUMN_OPTIONS,
  FONT_OPTIONS,
  INTERVAL_OPTIONS,
  PHONE_DEFAULTS,
  THEME_OPTIONS,
  loadPhoneSettings,
  readPhoneSettings,
  resolveKanban,
  resolveLook,
  savePhoneSettings
} from '../src/phoneSettings.js'

const memory = () => {
  const map = new Map()
  return { getItem: (k) => map.get(k) ?? null, setItem: (k, v) => map.set(k, v), map }
}

describe('reading the phone\'s settings', () => {
  it('falls to the defaults for nothing at all', () => {
    for (const raw of [undefined, null, 'text', 7]) expect(readPhoneSettings(raw)).toEqual(PHONE_DEFAULTS)
  })

  it('defaults every choice to the desktop\'s and the font to 13', () => {
    expect(PHONE_DEFAULTS).toEqual({ theme: 'desktop', density: 'desktop', uiFontSize: 13, columns: 'desktop', interval: 'desktop' })
  })

  it('replaces each unknown or out-of-range value with its default', () => {
    expect(readPhoneSettings({ theme: 'purple', uiFontSize: 2, interval: 'year' })).toEqual(PHONE_DEFAULTS)
  })

  it('offers "Same as desktop" first, except for the font size', () => {
    for (const options of [THEME_OPTIONS, COLUMN_OPTIONS, INTERVAL_OPTIONS]) {
      expect(options[0]).toEqual({ value: 'desktop', label: 'Same as desktop' })
    }
    expect(FONT_OPTIONS[0]).toEqual({ value: 10, label: '10 px' })
    expect(FONT_OPTIONS.at(-1)).toEqual({ value: 24, label: '24 px' })
  })
})

describe('keeping the phone\'s settings', () => {
  it('round-trips a valid object through the storage', () => {
    const storage = memory()
    const value = { theme: 'dark', density: 'compact', uiFontSize: 18, columns: 'non-empty', interval: 'week' }
    savePhoneSettings(storage, value)
    expect([...storage.map.keys()]).toEqual(['smetana.companion.settings'])
    expect(loadPhoneSettings(storage)).toEqual(value)
  })

  it('reads the defaults when the storage throws', () => {
    const broken = { getItem: () => { throw new Error('denied') }, setItem: () => { throw new Error('full') } }
    expect(loadPhoneSettings(broken)).toEqual(PHONE_DEFAULTS)
    expect(() => savePhoneSettings(broken, PHONE_DEFAULTS)).not.toThrow()
  })

  it('reads the defaults from damaged text', () => {
    const storage = { getItem: () => '{not json' }
    expect(loadPhoneSettings(storage)).toEqual(PHONE_DEFAULTS)
  })
})

describe('what wins, the phone or the desktop', () => {
  it('takes the desktop\'s look where the phone has no choice', () => {
    expect(resolveLook(PHONE_DEFAULTS, 'light', 'compact')).toEqual({ theme: 'light', density: 'compact' })
    expect(resolveLook({ ...PHONE_DEFAULTS, theme: 'dark' }, 'light', 'compact')).toEqual({ theme: 'dark', density: 'compact' })
  })

  it('overrides only the column mode and the period, and leaves the lists', () => {
    const kanban = { columns: 'all', alwaysShow: ['a'], interval: 'week', unlimited: ['b'] }
    expect(resolveKanban(PHONE_DEFAULTS, kanban)).toEqual(kanban)
    expect(resolveKanban({ ...PHONE_DEFAULTS, columns: 'non-empty', interval: 'day' }, kanban)).toEqual({
      columns: 'non-empty', alwaysShow: ['a'], interval: 'day', unlimited: ['b']
    })
  })
})
