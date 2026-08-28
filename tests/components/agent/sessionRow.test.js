import { describe, expect, it } from 'vitest'
import {
  META_SEPARATOR,
  lastMessageLine,
  messageLabel,
  oneLine,
  relativeTime,
  sessionMeta,
  sessionTitle,
  subagentLabel
} from '../../../src/components/agent/sessionRow.js'

/* The whole of what a session row says, which is the whole of why this module
   exists: `SessionRow.vue` is a `.vue` file and no test in this repository can
   reach one. */

const NOW = Date.parse('2026-08-28T12:00:00Z')
const ago = (ms) => new Date(NOW - ms).toISOString()
const MINUTE = 60 * 1000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

describe('one line out of a transcript', () => {
  it('collapses the line breaks a message arrived with', () => {
    expect(oneLine('first\nsecond\n\n  third')).toBe('first second third')
  })

  it('a blank string is no text at all', () => {
    expect(oneLine('   \n ')).toBe(null)
    expect(oneLine('')).toBe(null)
  })

  it('anything that is not a string is no text either', () => {
    expect(oneLine(null)).toBe(null)
    expect(oneLine(undefined)).toBe(null)
    expect(oneLine(12)).toBe(null)
  })

  /* The worker clips both strings for the wire; this is the guard against a
     record that arrives unclipped, and what it protects is the DOM rather than
     the layout — the visible cut is the component's ellipsis. */
  it('a message nobody clipped does not reach the row whole', () => {
    const long = 'x'.repeat(5000)

    const line = oneLine(long)

    expect(line.length).toBeLessThan(long.length)
    expect(line.endsWith('…')).toBe(true)
  })
})

describe('the row\'s title', () => {
  it('is the first thing the person said', () => {
    expect(sessionTitle({ title: 'Talk to me in Russian' })).toBe('Talk to me in Russian')
  })

  /* A transcript with no human message in it is an ordinary file: opened and
     abandoned. The id is deliberately not offered instead — they are file stems
     of one length and shape, and a column of them says less than a column of
     the same three words. */
  it('says so in words when there is nothing to title it with', () => {
    expect(sessionTitle({ title: null })).toBe('Untitled session')
    expect(sessionTitle({})).toBe('Untitled session')
    expect(sessionTitle(undefined)).toBe('Untitled session')
  })
})

describe('the row\'s last message', () => {
  it('the person\'s own words are attributed to them', () => {
    expect(lastMessageLine({ lastRole: 'user', lastText: 'Leave it for now.' })).toBe(
      'You: Leave it for now.'
    )
  })

  it('and the agent\'s to the agent', () => {
    expect(lastMessageLine({ lastRole: 'assistant', lastText: 'Done.' })).toBe('Agent: Done.')
  })

  /* An unattributed line is the one thing this line must not be: "Agent:" over
     the person's own words is worse than no line. A role this front end has
     never heard of is an ordinary outcome, not an error. */
  it('a role nobody recognises leaves the line off entirely', () => {
    expect(lastMessageLine({ lastRole: 'system', lastText: 'Compacted.' })).toBe(null)
    expect(lastMessageLine({ lastRole: null, lastText: 'Compacted.' })).toBe(null)
  })

  it('nothing said means no line', () => {
    expect(lastMessageLine({ lastRole: 'user', lastText: null })).toBe(null)
    expect(lastMessageLine({})).toBe(null)
  })

  it('a message that came with line breaks still takes one line', () => {
    expect(lastMessageLine({ lastRole: 'user', lastText: 'first\nsecond' })).toBe(
      'You: first second'
    )
  })
})

describe('how much was said', () => {
  it('counts the messages, and the one is not a plural', () => {
    expect(messageLabel(48)).toBe('48 msgs')
    expect(messageLabel(1)).toBe('1 msg')
    expect(messageLabel(0)).toBe('0 msgs')
  })

  it('a count nobody sent is nothing rather than NaN', () => {
    expect(messageLabel(undefined)).toBe('0 msgs')
    expect(messageLabel(null)).toBe('0 msgs')
  })

  it('counts the subagents, and the one is not a plural either', () => {
    expect(subagentLabel(3)).toBe('3 subagents')
    expect(subagentLabel(1)).toBe('1 subagent')
  })

  /* The absence is the point: a session with no sidechain records is the
     ordinary case, and `0 subagents` on every second row would spend a third of
     the line saying that nothing happened. */
  it('a session with no subagents says nothing about subagents', () => {
    expect(subagentLabel(0)).toBe(null)
    expect(subagentLabel(undefined)).toBe(null)
  })
})

describe('when the session was last written to', () => {
  it('reads as a distance from now', () => {
    expect(relativeTime(ago(20 * 1000), NOW)).toBe('just now')
    expect(relativeTime(ago(4 * MINUTE), NOW)).toBe('4m ago')
    expect(relativeTime(ago(18 * HOUR), NOW)).toBe('18h ago')
    expect(relativeTime(ago(2 * DAY), NOW)).toBe('2d ago')
    expect(relativeTime(ago(21 * DAY), NOW)).toBe('3w ago')
    expect(relativeTime(ago(400 * DAY), NOW)).toBe('1y ago')
  })

  /* Each step is entered at its own boundary and not one unit early: 59 minutes
     is still minutes, and the hour begins at the hour. */
  it('a step begins where it says it does', () => {
    expect(relativeTime(ago(59 * MINUTE), NOW)).toBe('59m ago')
    expect(relativeTime(ago(HOUR), NOW)).toBe('1h ago')
    expect(relativeTime(ago(23 * HOUR), NOW)).toBe('23h ago')
    expect(relativeTime(ago(DAY), NOW)).toBe('1d ago')
    expect(relativeTime(ago(6 * DAY), NOW)).toBe('6d ago')
    expect(relativeTime(ago(7 * DAY), NOW)).toBe('1w ago')
  })

  /* The clock ticks once a minute and the mtime comes off another machine's
     idea of the same second, so a time in the future is a thing that happens.
     "In 30 seconds" for a session somebody is sitting in would be a strange
     thing to read. */
  it('a time in the future is clamped rather than counted backwards', () => {
    expect(relativeTime(new Date(NOW + HOUR).toISOString(), NOW)).toBe('just now')
  })

  it('a date nobody can read has no label at all', () => {
    expect(relativeTime('not a date', NOW)).toBe(null)
    expect(relativeTime(null, NOW)).toBe(null)
    expect(relativeTime(ago(HOUR), undefined)).toBe(null)
  })
})

describe('the row\'s meta line', () => {
  const full = {
    model: 'claude-opus-5',
    messages: 48,
    subagents: 3,
    branch: 'main',
    modifiedAt: ago(18 * HOUR)
  }

  it('reads left to right: what it is, how much of it, when, and where', () => {
    expect(sessionMeta(full, NOW).map((part) => part.text)).toEqual([
      'claude-opus-5',
      '48 msgs',
      '3 subagents',
      '18h ago',
      'main'
    ])
  })

  /* Identifiers in mono, the prose about them in sans — the project's rule, and
     the reason the pieces travel tagged rather than joined into one string. */
  it('the identifiers are the model and the branch, and nothing else', () => {
    expect(sessionMeta(full, NOW).filter((part) => part.mono).map((part) => part.text)).toEqual([
      'claude-opus-5',
      'main'
    ])
  })

  it('what the worker could not answer is left out rather than named', () => {
    expect(
      sessionMeta({ model: null, messages: 6, subagents: 0, branch: null, modifiedAt: ago(DAY) }, NOW)
        .map((part) => part.text)
    ).toEqual(['6 msgs', '1d ago'])
  })

  /* The message count is the one piece that is always there: a session with
     nothing else known about it is still a session, and a row with an empty
     third line would read as one that failed to draw. */
  it('a session nothing is known about still counts its messages', () => {
    expect(sessionMeta({}, NOW).map((part) => part.text)).toEqual(['0 msgs'])
  })

  /* The line wraps at 340px, and a separator that is a box of its own can be
     left at the end of a wrapped line pointing at nothing — which is what it
     did: `1y ago ·` with the branch on the row below. Every other list in this
     app joins the middot into the string it precedes; this one cannot, because
     it is set in two families, so the property is stated here instead. */
  it('a separator belongs to the piece that follows it', () => {
    const parts = sessionMeta(full, NOW)

    expect(parts[0].lead).toBe(null)
    expect(parts.slice(1).every((part) => part.lead === META_SEPARATOR)).toBe(true)
  })

  it('no piece is a separator on its own, so no line can end in one', () => {
    const parts = sessionMeta(full, NOW)

    expect(parts.every((part) => part.text && part.text !== META_SEPARATOR)).toBe(true)
  })

  /* Whichever pieces the worker could answer, the first of them is the one
     without a separator — the lead is a position in the line, not a property of
     the model id that usually opens it. */
  it('the piece that opens the line has no separator whatever it is', () => {
    const parts = sessionMeta({ model: null, messages: 6, branch: 'main', modifiedAt: ago(DAY) }, NOW)

    expect(parts[0].text).toBe('6 msgs')
    expect(parts[0].lead).toBe(null)
  })

  /* The separator travels with the piece; it must not travel with the piece's
     family. A mono piece keeps its mono and keeps its lead. */
  it('a separator does not disturb which family its piece is set in', () => {
    const branch = sessionMeta(full, NOW).at(-1)

    expect(branch).toEqual({ text: 'main', mono: true, lead: META_SEPARATOR })
  })
})
