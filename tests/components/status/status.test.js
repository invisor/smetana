import { describe, expect, it } from 'vitest'
import {
  RESERVED,
  STATUS_GLYPH,
  attentionLevel,
  hashStatus,
  normalizeStatus,
  statusCode,
  statusColors,
  statusGlyph,
  statusLabel,
  statusSlot
} from '../../../src/components/status/status.js'

describe('normalizeStatus', () => {
  it('lowercases and trims the edges', () => {
    expect(normalizeStatus('  Needs You  ')).toBe('needs-you')
  })

  it('collapses any run of non-alphanumerics into one dash', () => {
    expect(normalizeStatus('in___progress // now')).toBe('in-progress-now')
  })

  it('strips leading and trailing dashes', () => {
    expect(normalizeStatus('--ready--')).toBe('ready')
  })

  it('empty and missing give an empty string', () => {
    expect(normalizeStatus('')).toBe('')
    expect(normalizeStatus(null)).toBe('')
    expect(normalizeStatus(undefined)).toBe('')
  })
})

describe('hashStatus', () => {
  /* This test protects users rather than the hash: an "innocuous" edit to
     FNV-1a would recolour every user-defined status in every project at once.
     The values are taken from the current implementation and must not change. */
  it('is stable on a fixed sample', () => {
    expect(hashStatus('awaiting-review')).toBe(2045313954)
    expect(statusSlot('awaiting-review')).toBe(6)
    expect(hashStatus('triage')).toBe(166983937)
    expect(statusSlot('triage')).toBe(1)
    expect(hashStatus('deploy')).toBe(1557350270)
    expect(statusSlot('deploy')).toBe(2)
    expect(hashStatus('needs-review')).toBe(1866091121)
    expect(statusSlot('needs-review')).toBe(5)
  })

  it('normalization does not affect the hash', () => {
    expect(hashStatus('awaiting-review')).toBe(hashStatus('Awaiting Review'))
    expect(statusSlot('awaiting-review')).toBe(statusSlot('  awaiting__review '))
  })

  it('the slot is always within twelve', () => {
    const names = ['triage', 'awaiting-review', 'under-review', 'x', 'deploy', 'qa', 'sprint-3']
    for (const name of names) {
      const slot = statusSlot(name)
      expect(slot).toBeGreaterThanOrEqual(0)
      expect(slot).toBeLessThan(12)
      expect(Number.isInteger(slot)).toBe(true)
    }
  })
})

describe('statusColors', () => {
  it('a reserved status gets its own tokens and the reserved flag', () => {
    expect(statusColors('needs-you')).toEqual({
      reserved: true,
      key: 'needs-you',
      fg: 'var(--status-needs-you-fg)',
      bg: 'var(--status-needs-you-bg)',
      border: 'var(--status-needs-you-border)'
    })
  })

  it('all six reserved statuses are recognised', () => {
    for (const name of RESERVED) {
      expect(statusColors(name).reserved).toBe(true)
      expect(STATUS_GLYPH[name]).toBeTruthy()
    }
  })

  it('a user-defined status gets a generated slot', () => {
    const colors = statusColors('Awaiting Review')
    expect(colors.reserved).toBe(false)
    expect(colors.key).toBe('awaiting-review')
    /* Without this, fg/bg/border would be compared with themselves through
       colors.slot and would be true for any slot — the value is already pinned
       by the neighbouring hash stability test (hashStatus:
       statusSlot('awaiting-review') === 6). */
    expect(colors.slot).toBe(6)
    expect(colors.fg).toBe(`var(--status-gen-${colors.slot}-fg)`)
    expect(colors.bg).toBe(`var(--status-gen-${colors.slot}-bg)`)
    expect(colors.border).toBe(`var(--status-gen-${colors.slot}-border)`)
  })
})

describe('statusCode', () => {
  it('from two words it takes the first letter of each', () => {
    expect(statusCode('awaiting-review')).toBe('AR')
  })

  it('from one word it takes the first two letters', () => {
    expect(statusCode('triage')).toBe('TR')
  })

  it('from three words it takes the first two', () => {
    expect(statusCode('waiting-for-review')).toBe('WF')
  })
})

describe('statusLabel', () => {
  it('capitalises a bd status the picker has to append', () => {
    expect(statusLabel('parked')).toBe('Parked')
  })

  it('writes a multi-word status as sentence case prose', () => {
    expect(statusLabel('in_progress')).toBe('In progress')
    expect(statusLabel('ready_to_merge')).toBe('Ready to merge')
  })

  it('leaves nothing behind for an absent status', () => {
    expect(statusLabel('')).toBe('')
    expect(statusLabel(null)).toBe('')
  })
})

describe('attentionLevel', () => {
  it('needs-you and failed shout', () => {
    expect(attentionLevel('needs-you')).toBe('loud')
    expect(attentionLevel('failed')).toBe('loud')
  })

  it('running is live, done is quiet', () => {
    expect(attentionLevel('running')).toBe('live')
    expect(attentionLevel('done')).toBe('quiet')
  })

  it('an unknown status is live rather than quiet: hiding the unknown is worse than showing it', () => {
    expect(attentionLevel('awaiting-review')).toBe('live')
    expect(attentionLevel('')).toBe('live')
  })
})

describe('statusGlyph', () => {
  it('a reserved status draws its own glyph', () => {
    expect(statusGlyph('needs-you')).toBe('triangle-alert')
    expect(statusGlyph('done')).toBe('check')
  })

  it('a named custom status draws the glyph written down for it', () => {
    expect(statusGlyph('parked')).toBe('triangle-alert')
    expect(statusGlyph('ready_to_merge')).toBe('git-merge')
  })

  it('a status nobody has heard of falls back to the generic tag', () => {
    expect(statusGlyph('awaiting-review')).toBe('tag')
  })
})

/* The whole of what `human_check` is, pinned in one place. Every one of these
   follows from a rule elsewhere in this file rather than from a branch written
   for this status, and the point of the block is that the rules keep agreeing:
   a column of work that is merged and waiting on a person's own eye must not
   shout the way `needs-you` does, and it must say what it is in something other
   than a colour. */
describe('the human_check status', () => {
  it('is not reserved, so it takes a generated hue rather than a semantic one', () => {
    expect(RESERVED).not.toContain('human-check')
    expect(statusColors('human_check').reserved).toBe(false)
  })

  it('carries the two-letter code HC, since colour is never the only signal', () => {
    expect(statusCode('human_check')).toBe('HC')
  })

  it('is ordinary loudness: a column of these must never spend the loud budget', () => {
    expect(attentionLevel('human_check')).toBe('live')
  })

  it('draws a person with a tick rather than the generic tag', () => {
    expect(statusGlyph('human_check')).toBe('user-check')
  })

  it('is written as prose the way the status picker appends it', () => {
    expect(statusLabel('human_check')).toBe('Human check')
  })
})
