import { describe, expect, it } from 'vitest'
import {
  NOTIFICATION_DEFAULTS,
  SOUND_CHOICES,
  SOUND_IDS,
  SOUND_OFF,
  isSound,
  normalizeSound
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
