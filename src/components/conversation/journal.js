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
   of raw protocol costs them the panel. Three known kinds deliberately produce
   none either: `turn-start` is said by the message under it, and the two
   permission kinds are drawn from the session's open question, at the foot of
   the panel where it is answered, rather than twice.

   A tool call and its result are **one row**, folded by the id they share:
   `tool-result` carries no name and no detail of its own, so a row of its own
   would split one thing the agent did across two. `result: null` is the running
   state and is what `ToolCall` is written against — it draws the live glyph
   rather than guessing at an outcome. A result whose call is not in this
   journal is dropped: that is a journal the worker has trimmed, and there is
   nothing to put in the row.

   `key` is the event's own `seq`, which the worker mints from one counter per
   session and never reuses. */
export function journalRows(events = []) {
  const rows = []
  const calls = new Map()
  for (const event of events) {
    if (event.kind === 'user-message') {
      rows.push({
        key: event.seq,
        kind: 'user',
        text: event.text,
        attachments: event.attachments ?? []
      })
    } else if (event.kind === 'text') {
      rows.push({ key: event.seq, kind: 'agent', text: event.text })
    } else if (event.kind === 'reasoning') {
      rows.push({ key: event.seq, kind: 'reasoning', text: event.text })
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
        kind: 'result',
        tokensIn: event.tokens_in,
        tokensOut: event.tokens_out,
        costUsd: event.cost_usd ?? null,
        ms: event.ms
      })
    } else if (event.kind === 'error') {
      rows.push({ key: event.seq, kind: 'error', text: event.text })
    }
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
