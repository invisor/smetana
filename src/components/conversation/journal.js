/* What a driven session's journal is drawn as, and whether a turn is in flight.
   Both are whole rules and neither is in `ConversationView.vue`, for the reason
   this whole family of modules is outside the component that draws it: a `.vue`
   file is the one thing no runner in this repository can reach, so a rule left
   inside one is a rule nothing checks.

   Pure: no Vue, no DOM, no store. The events are the worker's own
   (`src-tauri/src/session/model.rs`), unchanged, and what comes back is a list
   of rows a template can `v-for` over. */

/* The wire's names, written out once for the whole front end.

   `session::model::Event` flattens its kind's fields in beside a kebab-case
   `kind` and renames none of them — `tokens_in`, `tokens_out`, `cost_usd` —
   which is deliberate on that side and carries a comment saying so. Every
   numeric prop of `TurnResult` has a default, so an event handed over raw
   (`v-bind="event"`, or `:tokens-in="event.tokens_in"` read off a store with no
   translation) draws `0 in · 0 out · 0 ms` silently: no Vue warning, nothing on
   screen to say the numbers are not the session's, and no component test in
   this project to catch it. That is why the translation is here, where a test
   can read it, and why there is exactly one copy of it.

   **An event kind this does not know produces no row.** The chain is closed and
   has no fallback, which is `EventKind`'s own rule one layer up — a missing row
   costs a person nothing the harness's own logs do not still hold, while a wall
   of raw protocol costs them the panel. The two permission kinds are drawn from
   the session's open question, at the foot of the panel where it is answered,
   rather than twice.

   A tool call and its result are **one row**, folded by the id they share:
   `tool-result` carries no name and no detail of its own, so a row of its own
   would split one thing the agent did across two. `result: null` is the running
   state and is what `ToolCall` is written against — it draws the live glyph
   rather than guessing at an outcome. A result whose call is not in this
   journal is dropped: that is a journal the worker has trimmed, and there is
   nothing to put in the row.

   **`turn-start` no longer produces nothing.** It used to, on the reading that
   the message under it already says a turn began; what it opens now is the
   strip `markup-contract.md` section 6 calls the agent's activity — one
   element, three moments, folded here exactly as a tool call and its result
   are. `result` closes it `done`, with the wire's own `tokens_in`, `tokens_out`,
   `cost_usd` and `ms`, the same four this file has always translated. `error`
   closes it `failed` **only while a turn is actually open** — `session::model`
   also uses `EventKind::Error` for a message that never reached a session with
   nothing open at all (`Request::Send` against a dead child), and that one is
   not a turn ending, it is the worker saying where the words went instead; it
   keeps the old bare `error` row a turn's fold has no share in.

   **A turn still open when the batch ends is not always `waiting`.** The
   commoner of the two is a live one — `waiting`, carrying the `turn-start`'s
   own `at` rather than a duration, since nothing pure can compute how long
   "now" is, which is `TurnResult.vue`'s own clock to tick. But `Chunk::Eof`
   (`session::service`, an agent process ending on its own) sets the session's
   own state `failed` while appending **no** `Error` at all — closing a shell
   window, and `Stop` itself, both end there, since `ClaudeDriver::interrupt`
   answers `None` and `Request::Stop` reaches for `start_kill()` outright. A
   turn read against the events alone would stay `waiting` forever in that
   case, its clock still climbing next to a header that already reads
   `failed` and a composer already reading Send again — which is why `state`
   is the second argument here.

   **The check is narrow on purpose: `state === 'failed' || state === 'exited'`,
   never the wider `!isBusy(state)`.** `state_of` (`session::model`) is the
   authority on what those words can mean: with the child dead it answers
   `Failed` for an open turn and `Exited` for a closed one, so `exited` with a
   turn still open is a state that cannot truthfully arise and is named here
   only so a value this file has misread can never reach it either. `ready` and
   `starting` are the ordinary skew of two events delivered as two IPC
   messages — `append()` ships `session:events` and only then calls
   `refresh_state`, which ships `session:state`
   (`service.rs`) — so a fresh `turn-start`/`user-message` can sit for a
   reactive flush against a `state` that has not moved off `ready` yet. Read
   against `!isBusy`, that skew closed the strip `failed`, wordless and
   saturated, under the very message that just opened the turn, on every
   single send. Read against this narrower pair it ticks `waiting` instead —
   the honest answer for a state this fold cannot yet see has ended, which is
   the same conservative reading `BUSY`'s own header gives a word it has never
   heard of.

   **The closed turn still gets words of its own.** `text` is not the empty
   string here any more: nobody said anything, but the panel knows one true
   sentence — the session ended while this turn was still open — and prints
   that rather than a blank strip a person would read as broken rather than
   stopped. This is the strip's only reader of `state` — every other row is
   the events alone.

   **The elapsed figure is the gap to the *last event this batch still
   holds*, not to the moment the child actually died** — `journal.js` has no
   later timestamp to read, and a batch holding only the `turn-start` reads
   `0s` rather than nothing. That under-reports whenever the agent's last
   tool call landed well before the process actually ended, sometimes by
   minutes; it never over-reports, and unlike anything measured off
   `Date.now()` it does not change if the panel is closed and reopened, which
   matters for a number sitting in a journal rather than ticking on screen.

   `Error` carries no duration of its own — `session::model::EventKind::Error`
   is `{ text }`, full stop — so a `failed` turn's elapsed time is the one this
   file *can* compute, the gap between the `turn-start` that opened it and the
   `error` that closed it. `elapsedSince` is that same gap asked of anything
   produced while a turn is open, which is also what gives `Reasoning`'s own
   `<summary><time>` a number to show: `EventKind::Reasoning` carries no
   duration either, and "how long the agent had been going when it said this"
   is the honest reading available from two timestamps the wire already sends.
   `null` where no turn is open, which the component reads as nothing to print
   rather than a guess at zero.

   `key` is the event's own `seq`, which the worker mints from one counter per
   session and never reuses — except the trailing `waiting` or crash-closed
   `failed`, both keyed by the `turn-start` that opened the turn they are
   still describing, since neither has a closing event of its own to mint a
   key under. */

/* Whether the agent is working, which is what turns the composer's one button
   into Stop — moved above `journalRows` because its trailing branch is now a
   second reader of it.

   **A closed list of `session::model::SessionState`'s wire words, and the
   sharpest reason anything here is outside the component.** Rename `Running` on
   the Rust side and a `.vue` file holding this list would stop showing the Stop
   button for the whole of a turn — a person could fire messages into a working
   agent with no way to stop it, and every gate in this repository would stay
   green. Here, the list is one grep and one test away from whoever renames it.

   `needs-you` counts as busy: the turn is open and what to do about it is the
   permission card above the field, not another message. `starting`, `ready`,
   `exited` and `failed` are the four that are not, and a word this front end
   has never heard of is not either — the honest reading of "we do not know that
   the agent is working" is to leave the person able to type.

   **`starting` is on that side, and the reason is worth keeping written down.**
   A driven session is started with `--input-format stream-json`, and a harness
   parked on its own stdin says nothing whatever: the journal stays empty, which
   is what `state_of` in `session::model` calls `starting`. Counted as a turn in
   flight, that state closed the only road out of itself — it ends at the first
   event, the first event is the `TurnStart` the worker appends when a message
   is sent, and the button that sends one had become Stop. A new agent sat at an
   empty panel with a composer that refused every key, for good. */
const BUSY = ['running', 'needs-you']

export const isBusy = (state) => BUSY.includes(state)

/* Closes an open turn even though nothing in its own events did — the two
   states `state_of` can only reach with the child already dead. Not
   `!isBusy(state)`: `ready` and `starting` are the ordinary skew between
   `session:events` and the `session:state` that follows it, and reading that
   skew as a death closed the strip `failed` under the very message that had
   just opened the turn. See the fold's own header for the fuller reasoning. */
const CHILD_GONE = ['failed', 'exited']

/* The one sentence the panel can truthfully say about a turn the events never
   closed: not what happened, since the worker gave no words for that, but
   that the session ended while this turn was still open. A blank strip reads
   as broken rather than stopped, which is the wordless version this
   replaces. */
const TURN_ENDED = 'The session ended while this turn was still open.'

export function journalRows(events = [], state) {
  const rows = []
  const calls = new Map()
  /* The turn open right now, if any — never more than one, since a person
     cannot send a second message while the composer's one button reads Stop.
     Cleared the moment `result` or `error` closes it, and read once more after
     the loop if nothing did. */
  let openAt = null
  let openSeq = null
  /* `null` rather than `NaN` where either stamp is missing — the wire always
     sends one, but a `NaN` would pass `TurnResult`'s `type: Number` check and
     draw `0s`, indistinguishable from a turn that genuinely took none. */
  const elapsedSince = (at) => (openAt == null || at == null ? null : Date.parse(at) - Date.parse(openAt))

  for (const event of events) {
    if (event.kind === 'turn-start') {
      openAt = event.at
      openSeq = event.seq
    } else if (event.kind === 'user-message') {
      rows.push({
        key: event.seq,
        kind: 'user',
        text: event.text,
        attachments: event.attachments ?? []
      })
    } else if (event.kind === 'text') {
      rows.push({ key: event.seq, kind: 'agent', text: event.text })
    } else if (event.kind === 'reasoning') {
      rows.push({ key: event.seq, kind: 'reasoning', text: event.text, ms: elapsedSince(event.at) })
    } else if (event.kind === 'tool-use') {
      const row = {
        key: event.seq,
        kind: 'tool',
        name: event.name,
        detail: event.detail,
        result: null
      }
      calls.set(event.id, row)
      rows.push(row)
    } else if (event.kind === 'tool-result') {
      const row = calls.get(event.id)
      if (row) row.result = { ok: event.ok, summary: event.summary }
    } else if (event.kind === 'result') {
      rows.push({
        key: event.seq,
        kind: 'activity',
        state: 'done',
        tokensIn: event.tokens_in,
        tokensOut: event.tokens_out,
        costUsd: event.cost_usd ?? null,
        ms: event.ms
      })
      openAt = null
      openSeq = null
    } else if (event.kind === 'error') {
      if (openAt != null) {
        rows.push({ key: event.seq, kind: 'activity', state: 'failed', text: event.text, ms: elapsedSince(event.at) })
        openAt = null
        openSeq = null
      } else {
        rows.push({ key: event.seq, kind: 'error', text: event.text })
      }
    }
  }

  if (openAt != null) {
    if (CHILD_GONE.includes(state)) {
      /* The events said nothing closed this turn, but the session's own state
         already has — `Chunk::Eof` with nothing journalled, which is `Stop`,
         a shell window closing, or any other end that rang no `Error`. `ms` is
         the gap to the last event this batch actually holds, in place of the
         one a real `error` would have closed on. */
      rows.push({
        key: openSeq,
        kind: 'activity',
        state: 'failed',
        text: TURN_ENDED,
        ms: elapsedSince(events[events.length - 1].at)
      })
    } else {
      rows.push({ key: openSeq, kind: 'activity', state: 'waiting', startedAt: openAt })
    }
  }

  return rows
}
