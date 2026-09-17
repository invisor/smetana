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

  /* The turn the app opened the session with. What went to the harness is the
     whole brief; what the row carries is the person's own share of it — and
     `null` for a start nobody typed a word into, which the panel draws as the
     session row's caption rather than as an empty bubble. */
  it('draws the opening turn as the person, with or without words', () => {
    const withWords = journalRows([
      event(1, 'turn-start', { by: 'person' }),
      event(2, 'opening', { text: 'Ring once', attachments: ['/a.png'] })
    ])
    expect(withWords[0]).toMatchObject({ key: 2, kind: 'user', text: 'Ring once', attachments: ['/a.png'], opening: true })

    const wordless = journalRows([
      event(1, 'turn-start', { by: 'person' }),
      event(2, 'opening', { text: null, attachments: [] })
    ])
    expect(wordless[0]).toMatchObject({ kind: 'user', text: null, attachments: [], opening: true })
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

  /* `AskUserQuestion`'s reply is the one `permission-answered` that becomes a
     row (smetana-58j7): the person's own words, drawn as an ordinary message
     rather than left to vanish with the card that asked for them. One
     question draws the answer as-is, with no question text above it — it
     reads as a sentence a person actually typed or chose. */
  it('draws a user row for a single AskUserQuestion answer, with no question repeated', () => {
    const rows = journalRows([
      event(1, 'permission', {
        id: 'q1',
        tool: 'AskUserQuestion',
        // `agents::claude::tool_detail` answers the first question's own
        // text for this tool, never `'AskUserQuestion'` itself.
        detail: 'Which branch should worktrees use?',
        options: ['allow', 'deny'],
        input: {
          questions: [
            {
              question: 'Which branch should worktrees use?',
              header: 'Branch naming',
              multiSelect: false,
              options: [{ label: 'Keep the slash', description: '' }]
            }
          ]
        }
      }),
      event(2, 'permission-answered', { id: 'q1', decision: 'allow', answers: { 'Which branch should worktrees use?': 'Keep the slash' } })
    ])

    expect(rows).toEqual([{ key: 2, kind: 'user', text: 'Keep the slash', attachments: [] }])
  })

  /* The mirror of the guard pinned below for the decline branch, on the
     answer branch this time: `session_answer` takes `answers` straight from
     the front end with no tool check on the Rust side, so nothing on the
     wire stops some other tool's `permission-answered` from carrying one.
     A `Bash` permission answered `allow` with an `answers` map hung off it
     must draw no row, the same as an ordinary `Bash` allow — every other
     test reaching this branch either uses `AskUserQuestion` or has no
     `permission` at all to check the tool against, so none of them would
     notice `isAskUserQuestion(permission.tool)` going missing from this
     condition. */
  it('draws no row for a Bash permission answered allow with an answers map hung off it', () => {
    const rows = journalRows([
      event(1, 'permission', { id: 'q1', tool: 'Bash', detail: 'ls', options: ['allow', 'deny'] }),
      event(2, 'permission-answered', { id: 'q1', decision: 'allow', answers: { 'Not a real question': 'Not a real answer' } })
    ])

    expect(rows).toEqual([])
  })

  /* Several questions in one call become a markdown list, one item per
     question, in the order the agent actually asked them
     (`input.questions`) — never in `answers`' own key order, which travels
     as a `BTreeMap` and so arrives sorted by question text. The two orders
     disagree here on purpose: alphabetically "Should" sorts before "Which",
     the reverse of how the call actually asked them. */
  it('orders several answers by input.questions, not by the answers map’s own key order', () => {
    const rows = journalRows([
      event(1, 'permission', {
        id: 'q1',
        tool: 'AskUserQuestion',
        detail: 'Which environment?',
        options: ['allow', 'deny'],
        input: {
          questions: [
            { question: 'Which environment?', header: '', multiSelect: false, options: [] },
            { question: 'Should we deploy?', header: '', multiSelect: false, options: [] }
          ]
        }
      }),
      event(2, 'permission-answered', {
        id: 'q1',
        decision: 'allow',
        answers: { 'Should we deploy?': 'Yes', 'Which environment?': 'Staging' }
      })
    ])

    expect(rows).toEqual([
      {
        key: 2,
        kind: 'user',
        text: '- **Which environment?**: Staging\n- **Should we deploy?**: Yes',
        attachments: []
      }
    ])
  })

  /* An `answers` key `input.questions` never named — asked about by some
     other means, or a call the agent's own model answered outside the
     questions it listed — is not dropped: it is appended after every matched
     question, in the design's own words, rather than lost for naming
     something `parseQuestions` never turned up. This is the one case that
     actually exercises the second loop in `answerText` with a `permission`
     present; the trimmed-permission test above never runs the first loop at
     all. The stray key is written *first* in the `answers` literal, matching
     a `BTreeMap` sorted by question text ("A stray…" before "Which…", the
     same discipline the sibling ordering test above applies deliberately) —
     an implementation that walked `Object.keys(answers)` alone, ignoring
     `input.questions` entirely, would then produce this exact reversed
     order, so writing the matched key first would have let that
     implementation pass by accident. */
  it('appends an answers key that input.questions never named to the end of the list', () => {
    const rows = journalRows([
      event(1, 'permission', {
        id: 'q1',
        tool: 'AskUserQuestion',
        detail: 'Which environment?',
        options: ['allow', 'deny'],
        input: { questions: [{ question: 'Which environment?', header: '', multiSelect: false, options: [] }] }
      }),
      event(2, 'permission-answered', {
        id: 'q1',
        decision: 'allow',
        answers: { 'A stray question nobody parsed': 'Answered anyway', 'Which environment?': 'Staging' }
      })
    ])

    expect(rows).toEqual([
      {
        key: 2,
        kind: 'user',
        text: '- **Which environment?**: Staging\n- **A stray question nobody parsed**: Answered anyway',
        attachments: []
      }
    ])
  })

  /* Decline to answer is `decision: 'deny'` with no `answers`, exactly like
     refusing any other tool — the fixed sentence is what tells the two
     apart, and it only appears once the looked-up `permission` confirms the
     declined tool actually was `AskUserQuestion`. */
  it('draws the fixed decline sentence for a declined AskUserQuestion', () => {
    const rows = journalRows([
      event(1, 'permission', {
        id: 'q1',
        tool: 'AskUserQuestion',
        detail: 'Which branch?',
        options: ['allow', 'deny'],
        input: { questions: [{ question: 'Which branch?', header: '', multiSelect: false, options: [] }] }
      }),
      event(2, 'permission-answered', { id: 'q1', decision: 'deny', answers: null })
    ])

    expect(rows).toEqual([{ key: 2, kind: 'user', text: 'Declined to answer.', attachments: [] }])
  })

  /* The guard the decline branch actually needs pinned: a `Bash` permission
     declined the ordinary way must stay silent even though its own
     `permission` is right there to look up — nothing about `decision: 'deny'`
     with no `answers` tells this apart from a declined `AskUserQuestion` on
     its own, and `isAskUserQuestion(permission.tool)` is the one thing that
     does. Every other case in this file either has no `permission` to check
     (the trimmed cases) or fails the `decision === 'deny'` half first (the
     preserved Bash test above, which answers `allow`), so none of them would
     notice this guard going missing. */
  it('draws no row for a declined Bash permission even though its own permission survived', () => {
    const rows = journalRows([
      event(1, 'permission', { id: 'q1', tool: 'Bash', detail: 'rm -rf /', options: ['allow', 'deny'] }),
      event(2, 'permission-answered', { id: 'q1', decision: 'deny', answers: null })
    ])

    expect(rows).toEqual([])
  })

  /* `journal::trim` can evict an answered `permission` ahead of its own
     `permission-answered`. With nothing left to check the tool against, a
     non-empty `answers` draws its row on trust — the one case a trimmed
     journal must not lose is the words themselves — ordered by the map's own
     key order since there is no `input.questions` left to order it by. */
  it('draws a row from answers alone when its own permission has been trimmed out of the journal', () => {
    const rows = journalRows([
      event(5, 'permission-answered', { id: 'q1', decision: 'allow', answers: { 'Which branch?': 'Keep the slash' } })
    ])

    expect(rows).toEqual([{ key: 5, kind: 'user', text: 'Keep the slash', attachments: [] }])
  })

  /* The mirror case: no `answers` and no `permission` either. An ordinary
     Deny looks exactly the same on the wire, and there is nothing here to
     tell the two apart — inventing a decline row would be guessing which
     tool was declined rather than reading it off the journal. */
  it('draws no row for a trimmed permission answered with no answers at all', () => {
    expect(journalRows([event(5, 'permission-answered', { id: 'q1', decision: 'deny', answers: null })])).toEqual([])
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

/* **Streaming (smetana-6we6)**: `text-delta` is `EventKind::TextDelta`, one
   incremental piece of a reply, never accumulated on the Rust side — this
   fold is what stitches a run of them into one growing `agent` row, and what
   closes it, on the wire decision this task made and `journal.js`'s own
   header carries in full. */
describe('a streamed reply', () => {
  it('stitches a run of deltas into one growing row, marked streaming', () => {
    // A real `turn-start` in front, the way the wire actually pairs the two —
    // without one, `openAt` never opens and the trailing branch below reads
    // an orphaned stream rather than a live one.
    const rows = journalRows([
      event(1, 'turn-start', { by: 'agent' }),
      event(2, 'text-delta', { text: 'The identity is read once' }),
      event(3, 'text-delta', { text: ' and passed down' })
    ])

    // The stitched row, and the trailing activity strip a genuinely still-open
    // turn draws beside it — `streaming`, covered on its own further down.
    expect(rows).toEqual([
      { key: 2, kind: 'agent', text: 'The identity is read once and passed down', streaming: true },
      { key: 1, kind: 'activity', state: 'streaming', startedAt: '2026-09-10T12:00:00Z' }
    ])
  })

  /* The closing `text` is not a second row: the key stays the first delta's
     own, so nothing here reflows when the reply finishes, and the text is the
     wire's own authoritative copy rather than the concatenation — cheap
     insurance against the two ever drifting even though they agree today.
     `streaming` stays on the row at `false` rather than being deleted: a
     truthiness check reads it exactly like a row that never streamed at all,
     which is the property the markup contract asks for; the row itself is
     free to remember that it once did. */
  it('closes a streamed reply on the whole text event, replacing the text and dropping streaming', () => {
    const rows = journalRows([
      event(1, 'text-delta', { text: 'The identity is read once' }),
      event(2, 'text-delta', { text: ' and passed down' }),
      event(3, 'text', { text: 'The identity is read once and passed down as `root`.' })
    ])

    expect(rows).toEqual([
      {
        key: 1,
        kind: 'agent',
        text: 'The identity is read once and passed down as `root`.',
        streaming: false
      }
    ])
  })

  /* A harness that never streams — every history replay, and Codex's own PTY
     road, which never reaches this fold at all — still takes the plain
     branch precisely as it did before this task: no `streaming` field at
     all, not `streaming: false`. */
  it('draws a whole text with no streaming field at all when nothing streamed it first', () => {
    const [row] = journalRows([event(1, 'text', { text: 'a whole reply' })])

    expect(row).toEqual({ key: 1, kind: 'agent', text: 'a whole reply' })
    expect(row).not.toHaveProperty('streaming')
  })

  /* Two separate replies in one turn — text, a tool call, more text — stitch
     into two rows and never one one running on. */
  it('starts a fresh stitched row after a tool call interrupts the stream', () => {
    const rows = journalRows([
      event(1, 'turn-start', { by: 'agent' }),
      event(2, 'text-delta', { text: 'Reading the file' }),
      event(3, 'text', { text: 'Reading the file first.' }),
      event(4, 'tool-use', { id: 't1', name: 'Read', detail: 'src/main.js' }),
      event(5, 'tool-result', { id: 't1', ok: true, summary: '10 lines' }),
      event(6, 'text-delta', { text: 'Found it' })
    ])

    expect(rows).toEqual([
      { key: 2, kind: 'agent', text: 'Reading the file first.', streaming: false },
      { key: 4, kind: 'tool', name: 'Read', detail: 'src/main.js', result: { ok: true, summary: '10 lines' } },
      { key: 6, kind: 'agent', text: 'Found it', streaming: true },
      { key: 1, kind: 'activity', state: 'streaming', startedAt: '2026-09-10T12:00:00Z' }
    ])
  })

  /* The fourth moment of the activity strip: the same trailing spot `waiting`
     takes, at the same `startedAt` as the turn that opened — a stream in
     progress is still the one open turn, and a second clock ticking from
     zero right beside a reply already seconds in would be a second, competing
     answer to "how long has this been going". */
  it('draws the strip streaming, not waiting, while a reply is still arriving', () => {
    const rows = journalRows(
      [
        event(1, 'turn-start', { by: 'agent', at: '2026-09-10T12:00:04Z' }),
        event(2, 'text-delta', { text: 'Working on it' })
      ],
      'running'
    )

    expect(rows.at(-1)).toEqual({
      key: 1,
      kind: 'activity',
      state: 'streaming',
      startedAt: '2026-09-10T12:00:04Z'
    })
  })

  /* A closed stream leaves the strip back at `waiting` for whatever the turn
     does next — reasoning, a tool call, or nothing at all yet. */
  it('returns the strip to waiting once a streamed reply has closed', () => {
    const rows = journalRows(
      [
        event(1, 'turn-start', { by: 'agent', at: '2026-09-10T12:00:00Z' }),
        event(2, 'text-delta', { text: 'Working on it' }),
        event(3, 'text', { text: 'Working on it.' })
      ],
      'running'
    )

    expect(rows.at(-1)).toMatchObject({ kind: 'activity', state: 'waiting' })
  })

  /* **The acceptance criterion this whole task turns on**: a stream the
     events never closed does not leave a caret pinned to a reply nothing is
     still writing, once the session's own state says the child is gone. The
     words stay — they are the honest record of what had arrived — only the
     live mark goes, the same "keep the words, drop the caret" reading
     `error` and `result` take on the same shape. */
  it('drops the caret rather than the words when the child dies mid-stream', () => {
    const rows = journalRows(
      [
        event(1, 'turn-start', { by: 'agent', at: '2026-09-10T12:00:00Z' }),
        event(2, 'text-delta', { text: 'Half a sentence', at: '2026-09-10T12:00:02Z' })
      ],
      'failed'
    )

    const agentRow = rows.find((row) => row.kind === 'agent')
    expect(agentRow).toEqual({ key: 2, kind: 'agent', text: 'Half a sentence', streaming: false })
    expect(rows.at(-1)).toMatchObject({ kind: 'activity', state: 'failed' })
  })

  /* Defensive rather than load-bearing on this wire's own protocol — a text
     block always closes with its own whole `text` before a `result` can
     follow — but a `result` arriving with a stream still nominally open must
     not leave that row pulsing forever either. */
  it('drops the caret if a result somehow arrives before the stream closed', () => {
    const rows = journalRows([
      event(1, 'turn-start', { by: 'agent', at: '2026-09-10T12:00:00Z' }),
      event(2, 'text-delta', { text: 'Half a sentence' }),
      event(3, 'result', { tokens_in: 1, tokens_out: 1, cost_usd: null, ms: 10 })
    ])

    const agentRow = rows.find((row) => row.kind === 'agent')
    expect(agentRow).toEqual({ key: 2, kind: 'agent', text: 'Half a sentence', streaming: false })
  })

  /* Same defence, for the other event that can close a turn. */
  it('drops the caret if an error somehow arrives before the stream closed', () => {
    const rows = journalRows([
      event(1, 'turn-start', { by: 'agent', at: '2026-09-10T12:00:00Z' }),
      event(2, 'text-delta', { text: 'Half a sentence' }),
      event(3, 'error', { text: 'exit 101', at: '2026-09-10T12:00:05Z' })
    ])

    const agentRow = rows.find((row) => row.kind === 'agent')
    expect(agentRow).toEqual({ key: 2, kind: 'agent', text: 'Half a sentence', streaming: false })
  })

  /* The review's own reproduction for the finding that two of the four
     caret-clearing paths were gated on `openAt != null` and two were not: a
     `turn-start` old enough to have been trimmed off the front of the journal
     (`journal.rs`'s own `BUDGET`) leaves `openAt` reading `null` while a
     `text-delta` appended after it, and therefore newer, survives — an
     orphaned stream this fold never opened a turn for. Before the fix, the
     `error` arm's clear lived inside `if (openAt != null)` and never ran here,
     leaving `streaming: true` beside a bare `error` row for good. */
  it('drops the caret on an error even with no turn-start in view to open one', () => {
    const rows = journalRows([
      event(1, 'text-delta', { text: 'orphan' }),
      event(2, 'error', { text: 'This session has ended.' })
    ])

    expect(rows).toEqual([
      { key: 1, kind: 'agent', text: 'orphan', streaming: false },
      { key: 2, kind: 'error', text: 'This session has ended.' }
    ])
  })

  /* The same orphaning, with nothing at all closing the stream afterwards —
     the trailing `else if (streamingRow)` this finding added. Before it, a
     `streamingRow` left open with `openAt` already `null` at the end of the
     loop had no path left above it to ever clear the caret, and it pulsed
     over "orphan" for the life of the panel. */
  it('drops the caret at the tail when the stream outlives its own trimmed-away turn-start', () => {
    const rows = journalRows([event(1, 'text-delta', { text: 'orphan' })])

    expect(rows).toEqual([{ key: 1, kind: 'agent', text: 'orphan', streaming: false }])
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
