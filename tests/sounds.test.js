import { describe, expect, it } from 'vitest'
import {
  NOTIFICATION_DEFAULTS,
  SOUND_CHOICES,
  SOUND_IDS,
  SOUND_OFF,
  isSound,
  normalizeSound,
  shouldPlay
} from '../src/sounds.js'

describe('what a notification sound may be', () => {
  it('offers Off first, then every shipped sound', () => {
    expect(SOUND_CHOICES[0]).toEqual({ value: SOUND_OFF, label: 'Off' })
    expect(SOUND_CHOICES.slice(1).map((choice) => choice.value)).toEqual(SOUND_IDS)
    expect(SOUND_CHOICES.slice(1).map((choice) => choice.label)).toEqual([
      'Sound 1',
      'Sound 2',
      'Sound 3',
      'Sound 4'
    ])
  })

  it('both defaults are real sounds, and they are not the same one', () => {
    // Shipped on, for git.autoFetch's reason, and different because the two
    // events call for different reactions.
    expect(isSound(NOTIFICATION_DEFAULTS.runFinished)).toBe(true)
    expect(isSound(NOTIFICATION_DEFAULTS.needsAttention)).toBe(true)
    expect(NOTIFICATION_DEFAULTS.runFinished).not.toBe(NOTIFICATION_DEFAULTS.needsAttention)
    expect(NOTIFICATION_DEFAULTS.runFinished).not.toBe(SOUND_OFF)
  })

  it('off is a value, not the absence of one', () => {
    expect(isSound(SOUND_OFF)).toBe(true)
    expect(normalizeSound(SOUND_OFF, 'sound-1')).toBe(SOUND_OFF)
  })

  it('anything else is not a sound, and takes the fallback', () => {
    for (const junk of ['sound-5', 'Sound 1', '', null, undefined, 3, {}, ['sound-1']]) {
      expect(isSound(junk)).toBe(false)
      expect(normalizeSound(junk, 'sound-2')).toBe('sound-2')
    }
  })

  it('with no fallback given, junk becomes off rather than a noise nobody chose', () => {
    expect(normalizeSound('sound-9')).toBe(SOUND_OFF)
  })
})

describe('whether a chime asked for right now makes a noise', () => {
  /* Every combination of the three answers, because the option only ever takes
     a noise away and the way to see that is the whole table. Focus arrives as
     an argument: `document.hasFocus` is `chime.js`'s question, which is what
     leaves this rule reachable by a test at all. */
  it('with the option on, a sound waits until the window is in the background', () => {
    expect(shouldPlay('sound-1', true, false)).toBe(true)
    expect(shouldPlay('sound-1', true, true)).toBe(false)
  })

  it('with the option off, a sound plays whether or not somebody is looking', () => {
    // Exactly the behaviour that stood before the option existed.
    expect(shouldPlay('sound-1', false, false)).toBe(true)
    expect(shouldPlay('sound-1', false, true)).toBe(true)
  })

  it('off is silence in all four positions of the other two', () => {
    for (const onlyWhenUnfocused of [true, false]) {
      for (const focused of [true, false]) {
        expect(shouldPlay(SOUND_OFF, onlyWhenUnfocused, focused)).toBe(false)
      }
    }
  })

  it('a value that is not a sound at all is silence too', () => {
    // The same fallback `normalizeSound` makes, one question over: an id nobody
    // ships must not become a noise nobody chose.
    for (const junk of ['sound-9', '', null, undefined, 3, {}]) {
      expect(shouldPlay(junk, false, false)).toBe(false)
      expect(shouldPlay(junk, true, false)).toBe(false)
    }
  })

  it('every shipped sound answers the same way, so the rule is about the option', () => {
    for (const id of SOUND_IDS) {
      expect(shouldPlay(id, true, true)).toBe(false)
      expect(shouldPlay(id, true, false)).toBe(true)
    }
  })
})
