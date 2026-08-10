import { describe, expect, it } from 'vitest'
import {
  EDITOR_FONT_DEFAULT,
  FONT_MAX,
  FONT_MIN,
  FONT_SIZES,
  THEME_CHOICES,
  UI_FONT_DEFAULT,
  clampFont,
  effectiveTheme,
  fontVars
} from '../src/appearance.js'

describe('effectiveTheme', () => {
  it('a picked theme is the theme, whatever the machine says', () => {
    expect(effectiveTheme('dark', false)).toBe('dark')
    expect(effectiveTheme('light', true)).toBe('light')
  })

  it('system follows the machine', () => {
    expect(effectiveTheme('system', true)).toBe('dark')
    expect(effectiveTheme('system', false)).toBe('light')
  })

  it('a value this front end has never heard of follows the machine too', () => {
    /* Rust has already turned junk into dark before it reaches here; what can
       still arrive is a theme a newer build knows and this one does not, and the
       machine's own answer is the closest thing to honouring it. */
    expect(effectiveTheme('solarized', true)).toBe('dark')
    expect(effectiveTheme(undefined, false)).toBe('light')
  })
})

describe('clampFont', () => {
  it('keeps a size the dropdown offers', () => {
    for (const size of FONT_SIZES) expect(clampFont(size, UI_FONT_DEFAULT)).toBe(size)
  })

  it('a size outside the range takes the shipped one rather than the nearest', () => {
    // The same refusal to guess as Rust's font_in_range: a 2 is a mis-edit, not
    // a request for the smallest size.
    expect(clampFont(2, UI_FONT_DEFAULT)).toBe(UI_FONT_DEFAULT)
    expect(clampFont(80, UI_FONT_DEFAULT)).toBe(UI_FONT_DEFAULT)
    expect(clampFont(FONT_MIN - 1, EDITOR_FONT_DEFAULT)).toBe(EDITOR_FONT_DEFAULT)
    expect(clampFont(FONT_MAX + 1, EDITOR_FONT_DEFAULT)).toBe(EDITOR_FONT_DEFAULT)
  })

  it('anything that is not a whole number of pixels takes the shipped one', () => {
    expect(clampFont(13.5, UI_FONT_DEFAULT)).toBe(UI_FONT_DEFAULT)
    expect(clampFont('16', UI_FONT_DEFAULT)).toBe(UI_FONT_DEFAULT)
    expect(clampFont(null, UI_FONT_DEFAULT)).toBe(UI_FONT_DEFAULT)
  })
})

describe('fontVars', () => {
  /* What this can and cannot check is worth being plain about. It owns the
     *factor*; the eight step values live in `tokens/typography.css` and the row
     and control heights in `tokens/space.css`, and that those files keep the
     factor plumbed through is `tests/styles/tokens.test.js`'s job. An earlier
     version of this file asserted a copy of the scale against literals in the
     same file and claimed to catch the two drifting — it could not, since both
     sides of the comparison were the copy. */
  it('at the shipped size the factor is exactly 1, so nothing moves', () => {
    expect(fontVars(UI_FONT_DEFAULT, EDITOR_FONT_DEFAULT)).toEqual({
      '--ui-scale': '1',
      '--text-code-size': '12px'
    })
  })

  it('the factor is the chosen size over the shipped one', () => {
    for (const size of FONT_SIZES) {
      const scale = Number(fontVars(size, EDITOR_FONT_DEFAULT)['--ui-scale'])
      expect(scale).toBeCloseTo(size / UI_FONT_DEFAULT, 12)
      // Which is what makes `--text-md` — the step the dropdown names, and the
      // one whose shipped value is UI_FONT_DEFAULT — land on the size a person
      // picked.
      expect(UI_FONT_DEFAULT * scale).toBeCloseTo(size, 12)
    }
  })

  it('the factor is monotonic, so a bigger choice is never smaller on screen', () => {
    const scales = FONT_SIZES.map((size) => Number(fontVars(size, EDITOR_FONT_DEFAULT)['--ui-scale']))
    for (let i = 1; i < scales.length; i += 1) {
      expect(scales[i]).toBeGreaterThan(scales[i - 1])
    }
  })

  it('the editor size is pinned, not scaled with the app', () => {
    // The one token the app-wide factor deliberately does not reach: chrome and
    // code are two answers, and this is what keeps them two.
    expect(fontVars(24, 12)['--text-code-size']).toBe('12px')
    expect(fontVars(10, 20)['--text-code-size']).toBe('20px')
  })

  it('an out-of-range size falls back rather than shrinking everything to nothing', () => {
    expect(fontVars(0, 0)).toEqual(fontVars(UI_FONT_DEFAULT, EDITOR_FONT_DEFAULT))
  })
})

describe('the theme choices', () => {
  it('offers exactly what settings.json accepts', () => {
    // The twin of THEMES in src-tauri/src/settings/model.rs. A value offered
    // here and unknown there would be picked and come back as dark.
    expect(THEME_CHOICES.map((choice) => choice.value)).toEqual(['system', 'dark', 'light'])
  })
})
