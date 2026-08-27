import { describe, expect, it } from 'vitest'
import {
  CHROME_ATTRIBUTE,
  CHROME_BUTTONS,
  CHROME_NONE,
  CHROME_STATES,
  CHROME_TRAFFIC_LIGHTS,
  WINDOW_CONTROLS,
  chromeFromPlatform,
  chromeInFullscreen,
  controlIcon,
  controlLabel
} from '../../../src/components/shell/windowChrome.js'

describe('chromeFromPlatform', () => {
  it('gives macOS the real traffic lights', () => {
    expect(chromeFromPlatform(CHROME_TRAFFIC_LIGHTS)).toBe(CHROME_TRAFFIC_LIGHTS)
  })

  it('gives the other desktops buttons of our own', () => {
    expect(chromeFromPlatform(CHROME_BUTTONS)).toBe(CHROME_BUTTONS)
  })

  /* A browser is the ordinary case, not a failure: it is where the gallery and
     the dev server run, and there is no window there to have chrome at all. */
  it('falls back to no chrome at all for anything it has not heard of', () => {
    expect(chromeFromPlatform(null)).toBe(CHROME_NONE)
    expect(chromeFromPlatform(undefined)).toBe(CHROME_NONE)
    expect(chromeFromPlatform('')).toBe(CHROME_NONE)
    expect(chromeFromPlatform('haiku')).toBe(CHROME_NONE)
  })
})

describe('chromeInFullscreen', () => {
  /* macOS moves the traffic lights into an auto-hiding bar in fullscreen, so
     the inset the bar keeps for them would be an empty gap. */
  it('takes the traffic lights away in fullscreen', () => {
    expect(chromeInFullscreen(CHROME_TRAFFIC_LIGHTS, true)).toBe(CHROME_NONE)
  })

  it('leaves the traffic lights alone out of fullscreen', () => {
    expect(chromeInFullscreen(CHROME_TRAFFIC_LIGHTS, false)).toBe(CHROME_TRAFFIC_LIGHTS)
  })

  /* Our own buttons are the only way to leave fullscreen on a window with no
     decorations, so they stay. */
  it('keeps our own buttons in fullscreen', () => {
    expect(chromeInFullscreen(CHROME_BUTTONS, true)).toBe(CHROME_BUTTONS)
    expect(chromeInFullscreen(CHROME_NONE, true)).toBe(CHROME_NONE)
  })
})

describe('the window controls', () => {
  it('runs minimize, maximize, close in that order', () => {
    expect(WINDOW_CONTROLS.map((control) => control.action)).toEqual([
      'minimize',
      'toggle-maximize',
      'close'
    ])
  })

  it('names every icon it asks for', () => {
    for (const control of WINDOW_CONTROLS) {
      expect(controlIcon(control, false)).toBeTruthy()
      expect(controlIcon(control, true)).toBeTruthy()
    }
  })

  /* The middle button is two buttons wearing one seat, and a label saying
     "Maximize" over a maximized window is the kind of thing nobody notices
     until they are looking for the way back. */
  it('says restore over a maximized window, and maximize otherwise', () => {
    const middle = WINDOW_CONTROLS[1]
    expect(controlLabel(middle, false)).toBe('Maximize')
    expect(controlLabel(middle, true)).toBe('Restore')
    expect(controlIcon(middle, false)).not.toBe(controlIcon(middle, true))
  })

  it('leaves the other two saying the same thing either way', () => {
    for (const control of [WINDOW_CONTROLS[0], WINDOW_CONTROLS[2]]) {
      expect(controlLabel(control, true)).toBe(controlLabel(control, false))
      expect(controlIcon(control, true)).toBe(controlIcon(control, false))
    }
  })

  it('writes every label in sentence case', () => {
    for (const control of WINDOW_CONTROLS) {
      for (const label of [controlLabel(control, false), controlLabel(control, true)]) {
        expect(label).toBe(label[0].toUpperCase() + label.slice(1).toLowerCase())
      }
    }
  })
})

describe('the vocabulary itself', () => {
  it('holds three states and names the attribute they are written to', () => {
    expect(CHROME_STATES).toEqual([CHROME_NONE, CHROME_TRAFFIC_LIGHTS, CHROME_BUTTONS])
    expect(CHROME_ATTRIBUTE).toBe('data-window-chrome')
  })
})
