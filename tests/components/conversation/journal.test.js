import { describe, expect, it } from 'vitest'
import { isBusy, journalRows } from '../../../src/components/conversation/journal.js'

/* One journal event as the worker writes it: `seq`, `at`, and the kind's own
   fields flattened in beside a kebab-case `kind` — the serde shape of
   `session::model::Event`, which is the contract rather than a convenience.
   Written the same way `tests/stores/conversation.test.js` writes it, and for
   the same reason: a fixture in the front end's own casing would agree with a
   translation that is wrong. */
const event = (seq, kind, over = {}) => ({ seq, at: '2026-09-10T12:00:00Z', kind, ...over })

describe('the journal as rows', () => {
  it('draws both halves of the conversation and the agent thinking', () => {
    const rows = journalRows([
      event(1, 'user-message', { text: 'hello', attachments: ['/tmp/a.png'] }),
      event(2, 'text', { text: 'an answer' }),
      event(3, 'reasoning', { text: 'working it out' })
    ])

    expect(rows).toEqual([
      { key: 1, kind: 'user', text: 'hello', attachments: ['/tmp/a.png'] },
      { key: 2, kind: 'agent', text: 'an answer' },
      { key: 3, kind: 'reasoning', text: 'working it out' }
    ])
  })

  /* A message sent with no files at all still has a list to draw: `attachments`
     is `Vec<String>` on the wire and never absent, but the prop it feeds takes
     an array and a missing one would be a render-time failure rather than an
     empty strip. */
  it('gives a message with no files an empty list rather than nothing', () => {
    expect(journalRows([event(1, 'user-message', { text: 'hello' })])[0].attachments).toEqual([])
  })

  /* **The four names the wire actually uses**, which is the one translation in
     the whole front end and the one the tracker singled out: every numeric prop
     of `TurnResult` has a default, so an event handed over raw draws
     "0 in · 0 out · 0 ms" with no warning anywhere. */
  it('translates a turn result out of the wire names', () => {
    const [row] = journalRows([
      event(9, 'result', { tokens_in: 12480, tokens_out: 416, cost_usd: 0.0312, ms: 4200 })
    ])

    expect(row).toEqual({
      key: 9,
      kind: 'result',
      tokensIn: 12480,
      tokensOut: 416,
      costUsd: 0.0312,
      ms: 4200
    })
  })

  /* `cost_usd` is `Option<f64>`, and the difference between a turn that was free
     and a harness that did not say is what `TurnResult` omits the cost for. */
  it('keeps a cost the harness did not report as null', () => {
    const [row] = journalRows([
      event(9, 'result', { tokens_in: 1, tokens_out: 2, cost_usd: null, ms: 3 })
    ])

    expect(row.costUsd).toBe(null)
  })

  it('folds a tool call together with its result, by the id they share', () => {
    const rows = journalRows([
      event(1, 'tool-use', { id: 't1', name: 'Read', detail: 'src/main.js' }),
      event(2, 'tool-use', { id: 't2', name: 'Bash', detail: 'cargo test' }),
      event(3, 'tool-result', { id: 't2', ok: false, summary: 'exit 101' }),
      event(4, 'tool-result', { id: 't1', ok: true, summary: '10 lines' })
    ])

    expect(rows).toHaveLength(2)
    expect(rows[0]).toMatchObject({ name: 'Read', result: { ok: true, summary: '10 lines' } })
    expect(rows[1]).toMatchObject({ name: 'Bash', result: { ok: false, summary: 'exit 101' } })
  })

  /* `null` and not a guess: `ToolCall` draws the live glyph for it, where an
     outcome would be the panel claiming something it does not know. */
  it('leaves a call with no result yet running', () => {
    const [row] = journalRows([event(1, 'tool-use', { id: 't1', name: 'Grep', detail: 'fn main' })])

    expect(row.result).toBe(null)
  })

  /* A trimmed journal: the call this answers is no longer held, so there is
     nothing to put in a row and the result is dropped rather than drawn on a
     line of its own with no name and no detail on it. */
  it('drops a result whose call is not in the journal', () => {
    expect(journalRows([event(4, 'tool-result', { id: 't1', ok: true, summary: 'done' })])).toEqual(
      []
    )
  })

  /* The closed chain. `turn-start` is said by the message under it, and the two
     permission kinds are drawn from the session's open question at the foot of
     the panel — once, where they are answered. */
  it('draws no row for a turn start or for either half of a permission', () => {
    expect(
      journalRows([
        event(1, 'turn-start', { by: 'person' }),
        event(2, 'permission', { id: 'q1', tool: 'Bash', detail: 'ls', options: ['allow'] }),
        event(3, 'permission-answered', { id: 'q1', decision: 'allow' })
      ])
    ).toEqual([])
  })

  /* An error is the worker saying what happened where an answer would have
     gone — a message that never reached the agent, a line off stderr — and it is
     the one kind here a person can act on. */
  it('draws an error the worker reported', () => {
    expect(journalRows([event(1, 'error', { text: 'the agent could not be reached' })])).toEqual([
      { key: 1, kind: 'error', text: 'the agent could not be reached' }
    ])
  })

  /* **No fallback branch, and that is the rule rather than an omission**: a kind
     this front end has never heard of produces no row, exactly as a line a
     driver does not recognise produces no event on the Rust side. A wall of raw
     protocol costs somebody the panel; a missing row costs them nothing the
     harness's own logs do not still hold. */
  it('draws nothing at all for a kind it has never heard of', () => {
    expect(journalRows([event(1, 'thermostat-changed', { to: 21 })])).toEqual([])
  })

  it('takes no events at all', () => {
    expect(journalRows()).toEqual([])
  })
})

/* **The closed list of `SessionState`'s wire words**, and the reason it is out
   here: rename `Running` in `session::model` and a copy of this list inside a
   `.vue` file would stop turning the composer's one button into Stop for the
   whole of a turn — somebody could fire messages into a working agent with no
   way to stop it, and every gate in this repository would stay green. */
describe('whether the agent is working', () => {
  it('counts one in a turn, and one holding a question', () => {
    expect(['running', 'needs-you'].map(isBusy)).toEqual([true, true])
  })

  /* **A session that has yet to say anything is waiting, not working**, and it
     is waiting for the person. The harness this app drives is started with
     `--input-format stream-json` and says nothing at all until it is sent a
     message, so a new session's journal is empty and `state_of` calls that
     `starting`. Counted as a turn in flight, it left the composer's one button
     on Stop — and the only thing that ends the state is the message that button
     refuses to send, so nobody could say the first word to a new agent. */
  it('does not count a session that has yet to say anything', () => {
    expect(isBusy('starting')).toBe(false)
  })

  it('does not count a session waiting on a person, or one that has ended', () => {
    expect(['ready', 'exited', 'failed'].map(isBusy)).toEqual([false, false, false])
  })

  /* The safe direction: a word this front end has never heard of is not a turn
     in flight, so the field stays sendable rather than locking somebody out of
     their own composer over a state Rust added and this list has not learned. */
  it('does not count a word it has never heard of', () => {
    expect(isBusy('hibernating')).toBe(false)
    expect(isBusy(undefined)).toBe(false)
  })
})
