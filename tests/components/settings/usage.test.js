import { describe, expect, it } from 'vitest'
import {
  agentOf,
  offersRefresh,
  usageLines,
  usageNote
} from '../../../src/components/settings/usage.js'

/* `agent_usage`'s three answers, in Rust's own shape — `runs::usage::AgentUsage`
   serialized, which is what `the_wire_shape_is_the_one_the_settings_window_reads`
   pins on the other side of the boundary. */
const read = ({ usage, ...over } = {}) => ({
  state: 'read',
  agent: 'claude',
  band: 'normal',
  ...over,
  usage: {
    sessionPct: 10,
    sessionReset: 'Aug 7 at 8pm (Europe/Moscow)',
    weekPct: 20,
    weekReset: 'Aug 11 at 5:59pm (Europe/Moscow)',
    ...usage
  }
})
const unreadable = { state: 'unreadable', agent: 'claude' }
const unsupported = { state: 'unsupported', agent: 'codex' }
const nothingInstalled = { state: 'unsupported', agent: null }

describe('the two lines of a reading', () => {
  it('reads a percentage and the harness own words about the reset', () => {
    expect(usageLines(read())).toEqual([
      { name: 'Session', value: '10% used · resets Aug 7 at 8pm (Europe/Moscow)' },
      { name: 'This week', value: '20% used · resets Aug 11 at 5:59pm (Europe/Moscow)' }
    ])
  })

  it('shows the percentage alone when there is no reset to show', () => {
    // A fresh allowance prints no reset at all, which is a reading and not a
    // gap: inventing a time would be worse than the missing half of a sentence.
    const lines = usageLines(read({ usage: { sessionReset: null, weekReset: '   ' } }))
    expect(lines[0].value).toBe('10% used')
    expect(lines[1].value).toBe('20% used')
  })

  it('draws no rows at all for the two states that carry no numbers', () => {
    expect(usageLines(unreadable)).toEqual([])
    expect(usageLines(unsupported)).toEqual([])
    expect(usageLines(nothingInstalled)).toEqual([])
    expect(usageLines(null)).toEqual([])
  })

  it('refuses half a reading rather than drawing the half that arrived', () => {
    // A percentage this build cannot read is not a zero, and a row beside a
    // dash would claim the other one is a complete answer.
    expect(usageLines(read({ usage: { weekPct: null } }))).toEqual([])
    expect(usageLines(read({ usage: { sessionPct: 'lots' } }))).toEqual([])
  })

  it('never turns the absence of a reading into a percentage', () => {
    const drawn = [unreadable, unsupported, nothingInstalled, null]
      .flatMap((answer) => usageLines(answer).map((line) => line.value))
      .concat([unreadable, unsupported, nothingInstalled, null].map((a) => usageNote(a)))
      .join(' ')
    expect(drawn).not.toContain('%')
  })
})

describe('the sentence under the rows', () => {
  it('says what a run would do at this level, in the run own terms', () => {
    expect(usageNote(read({ band: 'normal' }))).toBe('A run would take a full batch at this level.')
    expect(usageNote(read({ band: 'reduced' }))).toContain('fewer tasks')
    expect(usageNote(read({ band: 'pause' }))).toContain('no new work')
  })

  it('says nothing about a run for a band this build has never heard of', () => {
    // The percentages are still drawn: they do not depend on knowing which of
    // the three bands Rust meant, and guessing would be the one thing that
    // could put the wrong promise under a real reading.
    expect(usageNote(read({ band: 'throttled' }))).toBe('')
    expect(usageLines(read({ band: 'throttled' }))).toHaveLength(2)
  })

  it('tells an agent that cannot be asked apart from one that would not answer', () => {
    expect(usageNote(unsupported)).toContain('does not report')
    expect(usageNote(unreadable)).toContain('could not be read')
    expect(usageNote(unsupported)).not.toBe(usageNote(unreadable))
  })

  it('names the empty machine rather than blaming an agent that is not there', () => {
    expect(usageNote(nothingInstalled)).toBe(
      'No agent is installed on this machine, so there is nothing to ask.'
    )
  })

  it('says it is reading while a probe is out, whatever is on screen', () => {
    expect(usageNote(null, true)).toBe('Reading what is left of the allowance…')
    expect(usageNote(read(), true)).toBe('Reading what is left of the allowance…')
  })

  it('says nothing has been read rather than claiming a state before the first answer', () => {
    expect(usageNote(null)).toBe('The allowance has not been read yet.')
  })

  it('says nothing when the channel itself refused, since that line is drawn instead', () => {
    // The refusal is `invoke` failing, not an answer: the command cannot fail
    // in Rust. The block draws it as a line of its own, and the sentence that
    // would sit above it — the reading is cleared before every read, so it
    // would be "not read yet" — is the one thing the refusal contradicts.
    expect(usageNote(null, false, 'the settings window could not reach the app')).toBe('')
    expect(usageNote(unreadable, false, 'nobody answered')).toBe('')
    expect(usageNote(read(), false, 'nobody answered')).toBe('')
  })

  it('does not describe one attempt twice when a refusal and a probe are both set', () => {
    // The window makes these two exclusive — it clears the error before it sets
    // busy — but which wins is worth pinning rather than leaving to whichever
    // condition a later edit happens to write first.
    expect(usageNote(null, true, 'nobody answered')).toBe('')
    expect(usageNote(read(), true, 'nobody answered')).toBe('')
  })

  it('is the ordinary sentence again once there is no refusal to draw', () => {
    // The absence of an error is `null` from the window and `''` from a prop
    // default, and neither may swallow the sentence.
    expect(usageNote(read(), false, null)).toBe('A run would take a full batch at this level.')
    expect(usageNote(read(), false, '')).toBe('A run would take a full batch at this level.')
    expect(usageNote(null, true, null)).toBe('Reading what is left of the allowance…')
  })

  it('reads an answer from a newer build as unreadable, which promises nothing', () => {
    expect(usageNote({ state: 'throttled', agent: 'claude' })).toContain('could not be read')
    expect(usageNote(read({ usage: { sessionPct: null } }))).toContain('could not be read')
  })
})

describe('which agent the block is about', () => {
  it('is whoever answered, in every state that has an answer', () => {
    expect(agentOf(read())).toBe('claude')
    expect(agentOf(unreadable)).toBe('claude')
    expect(agentOf(unsupported)).toBe('codex')
  })

  it('is nobody when there is nobody to name', () => {
    // Borrowing the selected agent here is what the heading must never do:
    // `agents::pick` substitutes an installed profile for a configured one, so
    // the picker and the answer genuinely disagree.
    expect(agentOf(nothingInstalled)).toBeNull()
    expect(agentOf(null)).toBeNull()
    expect(agentOf({ state: 'read', agent: '' })).toBeNull()
  })
})

describe('whether there is anything to press', () => {
  it('offers no refresh for an agent that does not answer the question', () => {
    expect(offersRefresh(unsupported)).toBe(false)
  })

  it('keeps it everywhere a second press could tell somebody something new', () => {
    expect(offersRefresh(read())).toBe(true)
    expect(offersRefresh(unreadable)).toBe(true)
    expect(offersRefresh(null)).toBe(true)
    // Installing an agent is the fix for this one, and then there is something
    // new to ask.
    expect(offersRefresh(nothingInstalled)).toBe(true)
  })
})
