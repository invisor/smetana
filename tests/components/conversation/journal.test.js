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
      { key: 3, kind: 'reasoning', text: 'working it out', ms: null }
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
     "0 in · 0 out · 0 ms" with no warning anywhere. A `result` closes the
     `activity` strip `turn-start` opened, `done`, at the `result`'s own seq —
     the same position the old bare `result` row held. */
  it('translates a turn result out of the wire names, closing the strip done', () => {
    const [row] = journalRows([
      event(7, 'turn-start', { by: 'person' }),
      event(9, 'result', { tokens_in: 12480, tokens_out: 416, cost_usd: 0.0312, ms: 4200 })
    ])

    expect(row).toEqual({
      key: 9,
      kind: 'activity',
      state: 'done',
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

  /* A `result` with no `turn-start` behind it still draws `done` — the wire
     always pairs them, but the fold does not lean on that: `ms` is the wire's
     own and needs no open turn to read. */
  it('still closes done with no turn-start at all', () => {
    const [row] = journalRows([
      event(9, 'result', { tokens_in: 1, tokens_out: 2, cost_usd: null, ms: 3 })
    ])

    expect(row).toMatchObject({ kind: 'activity', state: 'done' })
  })

  /* **The strip's `waiting` moment**: a `turn-start` with nothing closing it
     yet draws one row, at the very end, keyed by the `turn-start` itself since
     there is no closing event to mint a key under. `startedAt` is the
     `turn-start`'s own `at` — the ticking clock is `TurnResult.vue`'s to build
     from it, since nothing pure can know "now". Only reachable while the
     session's own `state` still counts as a turn in flight — `running` here,
     `needs-you` in the fixture below — which is the second argument. */
  it('draws the strip waiting while a turn is still open and the session is busy', () => {
    const rows = journalRows(
      [
        event(1, 'turn-start', { by: 'person', at: '2026-09-10T12:00:04Z' }),
        event(2, 'user-message', { text: 'hello' })
      ],
      'running'
    )

    expect(rows.at(-1)).toEqual({
      key: 1,
      kind: 'activity',
      state: 'waiting',
      startedAt: '2026-09-10T12:00:04Z'
    })
  })

  /* **A turn the events never closed is not always `waiting`.** `Chunk::Eof`
     — a shell window closing, or `Stop` itself, since `ClaudeDriver::interrupt`
     always answers `None` and the worker reaches for `start_kill()` — sets the
     session `failed` while appending no `Error` at all. Read against the
     events alone this would stay `waiting` forever, ticking beside a header
     that already reads `failed`; `state` says otherwise, so the strip closes
     too, with the panel's own sentence — the worker gave no words of its
     own — and an elapsed figure off the last event this batch still holds. */
  it('closes the strip failed when the session already has, and the events never did', () => {
    const rows = journalRows(
      [
        event(1, 'turn-start', { by: 'person', at: '2026-09-10T12:00:00Z' }),
        event(2, 'user-message', { text: 'rename the worktree', at: '2026-09-10T12:00:00Z' }),
        event(3, 'text', { text: 'working on it', at: '2026-09-10T12:03:41Z' })
      ],
      'failed'
    )

    expect(rows.at(-1)).toEqual({
      key: 1,
      kind: 'activity',
      state: 'failed',
      text: 'The session ended while this turn was still open.',
      ms: 221000
    })
  })

  /* **Only `failed` and `exited` close a turn the events never did**, never
     the wider `!isBusy(state)`. `state_of` can only reach those two with the
     child already dead — everything else, including `ready` and `starting`,
     is read as "we cannot see that it ended" and keeps the strip `waiting`.
     `ready`/`starting` matter especially: they are the ordinary skew between
     `session:events` and the `session:state` that follows it, one reactive
     flush behind a fresh send, and reading that skew as a death used to
     close the strip `failed` under the very message that had just opened the
     turn. */
  it('closes the strip failed only for the two states that mean the child is gone', () => {
    const at = (state) =>
      journalRows(
        [event(1, 'turn-start', { by: 'person', at: '2026-09-10T12:00:00Z' })],
        state
      ).at(-1).state

    expect(['failed', 'exited'].map(at)).toEqual(['failed', 'failed'])
    expect(['ready', 'starting', 'running', 'needs-you', undefined].map(at)).toEqual([
      'waiting',
      'waiting',
      'waiting',
      'waiting',
      'waiting'
    ])
  })

  /* **The strip's `failed` moment**: an `error` closes an *open* turn `failed`
     rather than standing on its own, with the worker's own words and the gap
     between the `turn-start` and the `error` — `EventKind::Error` carries no
     duration of its own, so this is the one the fold can compute. */
  it('closes an open turn failed on an error, with the elapsed gap', () => {
    const rows = journalRows([
      event(1, 'turn-start', { by: 'person', at: '2026-09-10T12:00:00Z' }),
      event(2, 'error', { text: 'exit 101 in wt/bd-3c9d', at: '2026-09-10T12:02:14Z' })
    ])

    expect(rows).toEqual([
      { key: 2, kind: 'activity', state: 'failed', text: 'exit 101 in wt/bd-3c9d', ms: 134000 }
    ])
  })

  /* **Not every `error` is a turn failing.** `Request::Send` against a session
     whose child has already died appends `Error` with no `turn-start` in front
     of it — the message never opened a turn at all — and that is the bare
     `error` row this journal has always drawn, not the strip. */
  it('leaves a standalone error alone, with no turn open to fold it into', () => {
    expect(
      journalRows([event(1, 'error', { text: 'This session has ended.' })])
    ).toEqual([{ key: 1, kind: 'error', text: 'This session has ended.' }])
  })

  /* `Reasoning`'s own `<time>` — how long the turn had been going when this
     was said, the gap between its `turn-start` and this event. */
  it('measures a reasoning block against the turn it was said in', () => {
    const rows = journalRows([
      event(1, 'turn-start', { by: 'person', at: '2026-09-10T12:00:00Z' }),
      event(2, 'reasoning', { text: 'working it out', at: '2026-09-10T12:00:18Z' }),
      event(3, 'result', { tokens_in: 1, tokens_out: 1, cost_usd: null, ms: 18000 })
    ])

    expect(rows[0]).toEqual({ key: 2, kind: 'reasoning', text: 'working it out', ms: 18000 })
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

  /* The two permission kinds are drawn from the session's open question at the
     foot of the panel — once, where they are answered — and produce no row of
     their own here, open turn or not. */
  it('draws no row for either half of a permission, leaving the open turn waiting', () => {
    const rows = journalRows(
      [
        event(1, 'turn-start', { by: 'person', at: '2026-09-10T12:00:00Z' }),
        event(2, 'permission', { id: 'q1', tool: 'Bash', detail: 'ls', options: ['allow'] }),
        event(3, 'permission-answered', { id: 'q1', decision: 'allow' })
      ],
      'needs-you'
    )

    expect(rows).toEqual([
      { key: 1, kind: 'activity', state: 'waiting', startedAt: '2026-09-10T12:00:00Z' }
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
