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
   element, four moments (`streaming` joined `waiting`, `done` and `failed`
   at smetana-6we6), folded here exactly as a tool call and its result are.
   `result` closes it `done`, with the wire's own `tokens_in`, `tokens_out`,
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

   **`streaming` is the fourth moment (smetana-6we6), and it is derived from
   the same events rather than carried on the wire as a state of its own.**
   `text-delta` is `session::model::EventKind::TextDelta` — one incremental
   piece of the reply, never accumulated on the Rust side — and this fold is
   what stitches a run of them into one growing `agent` row, `streamingRow`
   below. The stitched row's `key` is the *first* delta's own `seq` and never
   changes again: Vue's `:key` is what decides whether a row is the same DOM
   node or a new one, and a key that moved when the reply finished would be
   the reflow the acceptance criteria explicitly rule out. Closing a stream is
   the existing whole `text` event doing exactly what it always did — this
   fold does not wait for a second "done streaming" signal, because there
   isn't one and none is needed: the wire's `Text` already arrives once a
   content block completes (`claude_driver.rs`'s own header), so it is both
   the reply's authoritative content and its closing bell. On close the
   stitched text is **replaced wholesale** by the whole event's own copy
   rather than trusted to equal the concatenation — cheap, and immune to the
   two ever silently drifting.

   A row still being streamed carries `streaming: true`, which is the one
   thing that tells `AgentMessage`/`Markdown` to draw the live edge
   (`span[data-edge]`, contract section 6) after its last character — see
   `Markdown.vue`'s own header for where. Five places close a row still
   streaming, setting `streaming: false` without touching its `text` again:
   the ordinary close (`text` above), `result`, `error`, the trailing
   `CHILD_GONE` branch, and the trailing `else if (streamingRow)` beside it —
   because a caret pinned to a reply nothing is still writing is the exact
   scrap the acceptance criteria refuse to leave behind, and every one of the
   five is unconditional on whether this fold can still see the turn that
   opened the stream. That last qualification is load-bearing rather than
   defensive phrasing: `openAt` tracks the *turn* (set at `turn-start`,
   cleared at `result` or at a turn-closing `error`), and `streamingRow`
   tracks the *stream*, and the two can come apart once a journal is long
   enough to trim the `turn-start` off its front while later `text-delta`s
   survive — a trim drops from the front regardless of which kind of event it
   meets, and a reply's own deltas no longer accumulate past its own close
   (`journal.rs`'s own `Journal::append`), so this is reachable rather than
   ordinary: it takes one reply's still-open stream alone reaching `BUDGET`
   deltas to evict its own `turn-start`. `error`'s clear is therefore
   outside its own `openAt != null` branch, and the trailing `else if` exists
   for the same reason applied to the tail of the fold: a `streamingRow` still
   open when the loop ends with no `openAt` at all has nothing above it left
   to close it otherwise. In the protocol this wire actually speaks a content
   block is always closed by its own whole event before the next one opens,
   so in the ordinary case `streamingRow` is already `null` by the time any of
   the five is reached — the explicit closes are what stop a truly abandoned
   stream (the child dying mid-chunk, or its turn-start trimmed out from under
   it) from leaving a pulsing mark on screen forever, not a shape this file
   expects to hit on the every-day path.

   **A re-entry never finds a stitched row waiting to be resumed, and that
   rests on two separate guarantees rather than one.** Re-attaching to a
   session whose worker is still alive asks for a fresh snapshot
   (`session::journal::Journal::snapshot`), and what that snapshot holds
   depends on whether the stream it would be replaying is still open. A reply
   genuinely still arriving is handed over exactly as its own deltas stand
   right now, so it draws here exactly as it would have live — correct, since
   it is live. **A reply that has already closed is not handed over as the
   run of deltas a live listener actually watched arrive, one at a time** —
   `Journal::append` drops a stream's deltas the instant its closing `text`
   lands beside them, so a re-attacher's snapshot already holds only the
   consolidated event, the same way a fresh session opens on one. That
   collapse is safe *because* this fold treats the two shapes identically:
   a `text` with no `streamingRow` open behind it and a `text` closing one
   both end in the same finished row, so a re-attacher seeing fewer raw events
   than a live listener once did draws the same thing regardless. A session
   read back from Claude Code's own transcript (`session::history`, after a
   restart) never carries a `text-delta` at all in the first place: that
   harness's `.jsonl` holds only the consolidated `assistant` records this
   fold already turns into `text`, never the raw `stream_event` lines partial
   messages ride on — checked against the installed CLI rather than assumed,
   and `history::is_past` refuses the event kind a second time regardless. So
   there is no event sequence this fold could ever see that ends on an
   unclosed `streamingRow`, other than the live, still-open turn the trailing
   branch below already handles on purpose.

   **The mirror of that Rust-side collapse does not belong on this side, and
   that is worth writing down before somebody reaches for it.**
   `stores/conversation.js`'s `held.events` is push-only for the life of an
   attachment — it never had a budget and this task did not give it one, so a
   window open long enough keeps every delta it was ever sent. That is not
   this fold's problem to solve, but the obvious fix looks like one:
   dropping a held `text-delta` the moment its `text` arrives would make this
   very fold take the *plain* `text` branch instead of the closing one, which
   keys the new row on the `text` event's own `seq` rather than the first
   delta's. That is the reflow the acceptance criteria protect against,
   reintroduced by the fix meant to save memory — a row remounting at exactly
   the moment it finishes. Any future trim of `held.events` has to preserve
   the *key* a stream opened under, not merely the words.

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
  /* The `agent` row currently taking `text-delta`s, or `null` between
     streams — see this file's own header. Cleared (with `streaming` set
     `false` first) by the same four places that close a turn, and read once
     more after the loop for the same reason `openAt` is. */
  let streamingRow = null
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
    } else if (event.kind === 'text-delta') {
      if (streamingRow) {
        streamingRow.text += event.text
      } else {
        streamingRow = { key: event.seq, kind: 'agent', text: event.text, streaming: true }
        rows.push(streamingRow)
      }
    } else if (event.kind === 'text') {
      /* The stream's own close, not a second row: the stitched text is
         replaced wholesale by the wire's own authoritative copy rather than
         trusted to equal the concatenation, and the row keeps the key its
         first delta minted so nothing here reflows when a reply finishes. A
         harness that never streams (today: any history replay, and Codex's
         own PTY road never reaches this fold at all) still takes the plain
         branch exactly as before this task. */
      if (streamingRow) {
        streamingRow.text = event.text
        streamingRow.streaming = false
        streamingRow = null
      } else {
        rows.push({ key: event.seq, kind: 'agent', text: event.text })
      }
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
      // A defensive close, not the ordinary road: this wire's protocol closes
      // a text block with its own `text` before a turn's `result` can follow,
      // so `streamingRow` is already `null` here in the every-day case. What
      // this guards is the shape that protocol does not promise — a result
      // arriving with a stream never properly closed — and it takes the same
      // "keep the words, drop the caret" reading as every other close.
      if (streamingRow) {
        streamingRow.streaming = false
        streamingRow = null
      }
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
      // The caret close is deliberately outside the `openAt != null` branch
      // below: `openAt` tracks the *turn*, not the stream, and the two can
      // come apart once a journal is long enough to trim a `turn-start` away
      // while a `text-delta` appended after it survives. A `streamingRow`
      // orphaned that way must still lose its caret on any event that says
      // the words stopped, whether or not this fold can still see the turn
      // that opened them.
      if (streamingRow) {
        streamingRow.streaming = false
        streamingRow = null
      }
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
         one a real `error` would have closed on. A stream still open at this
         point is the same abandoned-reply shape: the words stay, the caret
         goes, for the reason the two branches above already give. */
      if (streamingRow) streamingRow.streaming = false
      rows.push({
        key: openSeq,
        kind: 'activity',
        state: 'failed',
        text: TURN_ENDED,
        ms: elapsedSince(events[events.length - 1].at)
      })
    } else if (streamingRow) {
      /* The fourth moment: the same strip `waiting` stands in, at the same
         `startedAt` — a stream is still the one open turn, and switching the
         clock to a second one at the first delta would tick from zero right
         next to a reply already several seconds in. */
      rows.push({ key: openSeq, kind: 'activity', state: 'streaming', startedAt: openAt })
    } else {
      rows.push({ key: openSeq, kind: 'activity', state: 'waiting', startedAt: openAt })
    }
  } else if (streamingRow) {
    // `openAt` tracks the turn, not the stream, and a long enough reply can
    // trim its own `turn-start` off the front of the journal while its own
    // `text-delta`s, appended later, survive — `openAt` reads `null` here
    // with `streamingRow` still open. There is no turn left to draw an
    // activity strip for, but the row already in `rows` still carries a
    // caret nothing above this point would otherwise ever clear.
    streamingRow.streaming = false
  }

  return rows
}
