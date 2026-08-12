import { describe, expect, it } from 'vitest'
import {
  MIB,
  THRESHOLDS_MIB,
  crossedThreshold,
  reachedThreshold,
  rememberAfter,
  stillOver,
  storageNotification
} from '../../../src/components/notifications/notifications.js'

const mib = (n) => n * MIB

describe('the ladder', () => {
  it('is the three thresholds the design names, in order', () => {
    expect(THRESHOLDS_MIB).toEqual([10, 50, 100])
  })

  it('reaches nothing under the first step', () => {
    expect(reachedThreshold(0)).toBe(null)
    expect(reachedThreshold(mib(3))).toBe(null)
    expect(reachedThreshold(mib(10) - 1)).toBe(null)
  })

  it('reaches a step exactly on it', () => {
    expect(reachedThreshold(mib(10))).toBe(10)
    expect(reachedThreshold(mib(50))).toBe(50)
    expect(reachedThreshold(mib(100))).toBe(100)
  })

  it('reaches the highest step below the size, not the nearest one', () => {
    expect(reachedThreshold(mib(49))).toBe(10)
    expect(reachedThreshold(mib(99))).toBe(50)
    expect(reachedThreshold(mib(400))).toBe(100)
  })

  it('has no answer for a size nobody measured', () => {
    expect(reachedThreshold(null)).toBe(null)
    expect(reachedThreshold(undefined)).toBe(null)
    expect(reachedThreshold(-1)).toBe(null)
    expect(reachedThreshold('12')).toBe(null)
  })
})

describe('what is worth announcing', () => {
  it('says nothing about a folder under every threshold', () => {
    expect(crossedThreshold(mib(3), null)).toBe(null)
    expect(crossedThreshold(mib(9), null)).toBe(null)
  })

  it('announces the first crossing of each step once', () => {
    expect(crossedThreshold(mib(12), null)).toBe(10)
    expect(crossedThreshold(mib(60), 10)).toBe(50)
    expect(crossedThreshold(mib(120), 50)).toBe(100)
  })

  it('stays quiet for the whole of the gap above an announced step', () => {
    expect(crossedThreshold(mib(12), 10)).toBe(null)
    expect(crossedThreshold(mib(49), 10)).toBe(null)
    expect(crossedThreshold(mib(99), 50)).toBe(null)
    expect(crossedThreshold(mib(4000), 100)).toBe(null)
  })

  it('skips the step nobody was around to hear', () => {
    // Straight from nothing to 120 MiB — one card, naming where the size is
    // now, rather than three cards for three steps crossed while the app was
    // closed.
    expect(crossedThreshold(mib(120), null)).toBe(100)
  })

  it('says nothing when there was no measurement', () => {
    expect(crossedThreshold(null, 10)).toBe(null)
    expect(crossedThreshold(undefined, null)).toBe(null)
  })
})

describe('what the project remembers afterwards', () => {
  it('remembers the step just announced', () => {
    expect(rememberAfter(mib(12), null)).toBe(10)
    expect(rememberAfter(mib(60), 10)).toBe(50)
  })

  it('re-arms the ladder when the folder falls back below a step', () => {
    expect(rememberAfter(mib(3), 10)).toBe(null)
    expect(rememberAfter(mib(12), 100)).toBe(10)
    // And the re-armed step speaks again.
    expect(crossedThreshold(mib(12), rememberAfter(mib(3), 10))).toBe(10)
  })

  it('keeps what it knew when the measurement did not happen', () => {
    expect(rememberAfter(null, 50)).toBe(50)
    expect(rememberAfter(undefined, null)).toBe(null)
  })

  it('forgets a number that is not one of the steps as soon as it is measured', () => {
    // A hand-edited file is Rust's to refuse, but nothing here leans on the
    // stored number being on the ladder: it is only ever compared.
    expect(rememberAfter(mib(12), 37)).toBe(10)
    expect(crossedThreshold(mib(12), 37)).toBe(null)
  })
})

describe('whether a card is still true', () => {
  it('stands while the folder still reaches the step it was announced at', () => {
    expect(stillOver(mib(12), 10)).toBe(true)
    expect(stillOver(mib(10), 10)).toBe(true)
    expect(stillOver(mib(400), 10)).toBe(true)
  })

  it('goes the moment the folder falls under it', () => {
    expect(stillOver(mib(9), 10)).toBe(false)
    expect(stillOver(0, 10)).toBe(false)
  })

  it('is not true of a measurement that never happened', () => {
    expect(stillOver(null, 10)).toBe(false)
  })
})

describe('the card', () => {
  const card = storageNotification('/Users/you/Projects/smetana', mib(12) + 512 * 1024, 10)

  it('is one card per project and step, so a repeat replaces rather than piles up', () => {
    expect(card.id).toBe('storage:/Users/you/Projects/smetana:10')
    expect(storageNotification('/Users/you/Projects/other', mib(12), 10).id).not.toBe(card.id)
  })

  it('names the folder, the size and the step', () => {
    expect(card.body).toContain('smetana')
    expect(card.body).toContain('12.5 MiB')
    expect(card.body).toContain('10 MiB')
  })

  it('says where the button leads rather than promising a deletion', () => {
    expect(card.actionLabel).toBe('Clean up')
    expect(card.body).toContain('Storage in settings')
  })

  it('carries the source it came from, which is what a second one would plug into', () => {
    expect(card.source).toBe('storage')
  })
})
