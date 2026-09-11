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
   keeps the old bare `error` row a turn's fold has no share in. A turn still
   open when the batch ends draws `waiting`, carrying the `turn-start`'s own
   `at` rather than a duration — nothing pure can compute how long "now" is,
   which is `TurnResult.vue`'s own clock to tick.

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
   session and never reuses — except `waiting`, keyed by the `turn-start` that
   opened the turn it is still describing, since no closing event exists yet to
   mint one under. */
export function journalRows(events = []) {
  const rows = []
  const calls = new Map()
  /* The turn open right now, if any — never more than one, since a person
     cannot send a second message while the composer's one button reads Stop.
     Cleared the moment `result` or `error` closes it, and read once more after
     the loop if nothing did. */
  let openAt = null
  let openSeq = null
  const elapsedSince = (at) => (openAt == null ? null : Date.parse(at) - Date.parse(openAt))

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
    rows.push({ key: openSeq, kind: 'activity', state: 'waiting', startedAt: openAt })
  }

  return rows
}

/* Whether the agent is working, which is what turns the composer's one button
   into Stop.

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
