/* What a row in the agents panel offers on a secondary click.

   The `branchMenu.js` / `taskMenu.js` / `sessionMenu.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — a `.vue` file
   is the one thing no test in this repository can reach, so a rule left inside
   the component is a rule nothing checks. It is also one half of a pair nothing
   mechanical joins: these `kind` strings are matched by hand in
   `AgentList.vue`, which turns each into an event of its own, the same seam
   `branchMenu.js` has with `BranchList.vue`. The test pins this side.

   **Four verbs and no more.** The panel is 236px wide by default and a row is
   one line; everything else an agent can be asked to do is a gesture somewhere
   else in the app — starting one is the project tile's menu, watching one is
   the row's own click, answering one is the terminal.

   Clearing was the third and arrived after the other two, from its own task
   (smetana-xyck), and the wait was the point: what clears a conversation is
   each harness's own vocabulary and belongs with the profiles, so
   `Profile::clear_command` had to exist before a row could offer it. What this
   file keeps of that fact is nothing at all: the answer arrives as `clearable`,
   a boolean the caller reads out of `stores/agents.js`, which read it out of
   `agents::catalogue`, which asked the profile. This file used to hold a list of
   the ids that clear, beside two more in `sessionMenu.js` for the ids that
   resume and fork.

   Closing every other row was the fourth, from its own task (smetana-t6x4): the
   ordinary case this panel is built for is one agent still worth watching among
   several that are done, closed one cross at a time today. `closableOthers`
   below is the whole of which rows that press reaches — every row but the one
   the menu is open on, minus the pinned and the still-starting, which is
   exactly the set that already carries a cross of its own.

   The order is pinning, clearing, closing, closing every other row. Pinning
   first because it is wanted most often, clearing between pinning and closing
   because it is neither — it deletes nothing, the transcript stays a file and
   the Sessions tab goes on listing it, which is also why it asks for no
   confirmation where `DeleteSessionModal` must — and the two closes last,
   because every destructive row in this app is last. `close-others` sits after
   `close` and not before it: it is the wider of the pair, and a menu opened by
   a roughly aimed pointer keeps its widest, most destructive row at the very
   foot.

   `pin` is the glyph — the same one the board's `pinned` status draws — `x` is
   the close, which is the mark the row already carries for that verb, and
   `eraser` is the clear, deliberately not the bin: the bin is deletion and
   these two verbs must not look alike. `close-others` takes `x` again rather
   than a glyph of its own: it is the same verb applied more widely, not a
   different one, and no glyph was added to `core/icons.js` for it. */

/* Whether a row is one of the pinned ones — `closableOthers` below reads the
   very rule `agentOrder.js` keeps rather than a second copy of it, since two
   readings of "is this pinned" is exactly the kind of pair that drifts. */
import { isPinned } from './agentOrder.js'

/* How wide the menu may get. A ceiling and not a width — `ContextMenu` draws
   itself as wide as its widest row and clips there with an ellipsis, and a menu
   row has neither a tooltip nor a `title`, so whatever does not fit is gone
   with no way back.

   The longest label this file can produce is `Close other agents — nothing
   else to close` at 42 characters, against `Clear session — this agent cannot
   do it` at 39, `Pin to top — nothing to remember it by` and `Clear session —
   it has not started yet` at 38 apiece, `Close agent — it has not started
   yet` at 36, `Close other agents` at 18 on its own and `Close agent` at 11.
   `sessionMenu.js` measured a menu row at 6.4px a character of `--text-sm` in
   `--font-sans` and `ContextMenu` at 70px of chrome around the label (`MENU_W`
   in `kanban/taskMenu.js` itemises where those pixels go), which puts the
   binding row at about 339px. 340 leaves almost nothing over that row and the
   same seventh of headroom for Segoe UI and Noto Sans the other three left
   over theirs, and the trade is the one every `MENU_W` in this app carries:
   px does not follow the app-wide font size, so a person running the
   interface large loses the tail of that one row first.

   **The ceiling is what the refusals are worded against, not the other way
   round**, which is `sessionMenu.js`'s rule and it bites harder here: this menu
   opens over the narrowest panel in the app. In characters that ceiling is
   `floor((340 − 70) / 6.4) = 42`, and it is now exactly met rather than merely
   approached: close-others' full label is 42 characters and comes to 338.8px,
   a fifth of one character under 340 rather than the wider margin the other
   three left. So each fragment is as short as it can be and still be a
   reason — the close-others row is what the ceiling cost most now, since its
   label is seven characters longer than the close's own and its one refusal
   still had to leave room under 42: `nothing else to close` is the shortest
   sentence that says a list of one row, or a list where everything else is
   pinned or starting, has nothing left for this verb to take. Whatever is
   worded here next has no room left under this ceiling at all. */
export const AGENT_MENU_W = 340

/* Why one of the three cannot be pressed, as lowercase fragments joined onto the
   label with a dash — `taskMenu.js`'s Run row is the precedent and this is the
   same trade it records: a menu row has nowhere else to put a reason.

   A refusal is drawn greyed and worded, never left out. A row that vanishes
   tells nobody why it was ever there. */
const AGENT_REASON = {
  /* A run's batch, a fork, a start that the worker has not answered for yet, a
     harness that cannot be told an id: none of them has a conversation id, and
     a pin kept under anything else would come back after a restart attached to
     a stranger. Pinning is the one thing here that has to outlive the session,
     so this is the one thing it cannot be done without. */
  noConversation: 'nothing to remember it by',
  /* "Cannot be closed" is drawn rather than enforced: a pinned row has no cross
     at all, and this is the same sentence in the menu. Two steps and not a
     confirmation — unpin, then close — because the pin is what says the person
     meant to keep this row. */
  pinned: 'unpin it first',
  /* The cross on a starting row is greyed for this already: the id such a row
     carries is this window's own rather than the worker's, so there is no
     session to end and nothing to take away. It lasts about a second. */
  starting: 'it has not started yet',
  /* Clearing needs a process to write into, and three of the panel's row kinds
     have none: a restored row from a previous launch, a session that exited,
     and one that fell over. All three are the same absence to this verb, so
     they get one sentence. */
  offline: 'it is not running',
  /* An agent waiting for an answer reads the next line written into it as that
     answer, so the clearing command would be a pick in somebody else's dialog
     rather than a command at all. Two steps and not a confirmation, which is
     the shape `pinned` above already has: answer it, then clear it. */
  waiting: 'answer it first',
  /* A harness nobody has confirmed a clearing line for. `sessionMenu.js`'s
     `this agent cannot resume by id` is the same sentence one verb over, and
     shorter here only because the ceiling above left no room for the longer
     one.

     Since the panel drew its first driven row it covers a second case: a
     session with no PTY to write that line into at all, which
     `components/agent/drivenRows.js` says by handing this file `clearable:
     false`. **One sentence for the two, and that is a decision rather than an
     oversight.** It reads as being about the agent on *this row* — which in
     both cases is one that cannot be cleared — rather than about the harness in
     the abstract: Claude Code clears perfectly well and a driven session of it
     still cannot. Split them the day somebody standing in front of it has to
     tell the two apart, and do it on a flag the row carries rather than by
     guessing from `clearable`, since a Codex PTY row says these same words
     truthfully. */
  cannotClear: 'this agent cannot do it',
  /* A list of one row, or a list where every other row is pinned or still
     starting, leaves this verb nothing to take. Drawn rather than left out,
     the same reasoning every other refusal here carries — a row that
     vanishes on a list of one tells nobody the verb was ever there. */
  nothingElse: 'nothing else to close'
}

export const PIN_LABEL = 'Pin to top'
export const UNPIN_LABEL = 'Unpin'
export const CLEAR_LABEL = 'Clear session'
export const CLOSE_LABEL = 'Close agent'
export const CLOSE_OTHERS_LABEL = 'Close other agents'


/* The states in which there is no process behind the row to write into.

   A closed list of the dead states rather than a list of the live ones, and the
   direction is the decision: `toUiState` in `stores/terminals.js` is free to
   grow a word this file has never heard of, and offering the verb is the softer
   way to be wrong — the write's own guard answers `noSession` and the toast
   says so, where greying would refuse a live agent with a reason that is not
   true. `sessionMenu.js` makes the same choice about `cwdExists`. */
const OFFLINE_STATES = ['done', 'failed']

/* The label with its reason joined on, or the plain label when there is none.
   Exported because the greying and the wording are one rule and a caller
   drawing either without the other would be drawing half of it. */
export function agentMenuLabel(label, reason) {
  return reason ? `${label} — ${reason}` : label
}

/* Which rows a `Close other agents` press would close: every row but the one
   the menu is open on, minus the pinned and the still-starting — exactly the
   set that already carries a cross of its own. Exported and pure, so
   `agentMenuItems` can grey the row from its length and the caller — the
   `remove-others` handler — can act on the very list this counted rather than
   asking the same three questions a second time.

   Matched by `id` and never by `agentKey`. What closes a row downstream is
   `removeAgentRow(row.id)`: `drivenSessionOf` reads the `conversation:`
   prefix off it and every other branch looks the id up in the session list or
   the restored records by that same field, so `id` is the one identity this
   panel's removal path actually understands. A row's `agentKey` — its
   conversation id where it has one — is a different string from its `id` the
   moment a conversation exists, and excluding the menu's own row by that key
   would leave the row the menu is open on inside the very list this verb is
   about to close.

   Pinned and starting are refused for the reason the cross beside each of
   them already is: a pinned row has no cross to press, and a starting row's
   id is this window's own with no session behind it yet to end. Offline and
   driven rows are not refused — the cross closes them exactly as it closes a
   live session, so this verb does too. */
export function closableOthers(rows, keptId, pinned) {
  const list = Array.isArray(rows) ? rows : []
  return list.filter((row) => row?.id !== keptId && !row?.starting && !isPinned(row, pinned))
}

/* The four rows.

   `pinned` and `starting` are facts about the row; `conversation` is the id the
   row carries, and its absence is what refuses the pin. `state` is the row's ui
   state, the same word `attentionLevel` reads, and `clearable` is whether the
   project's **configured** harness has a line that clears a conversation at all
   — neither is about this row alone, which is why only the clear row asks for
   them.

   That last one is handed in rather than looked up, so this module stays pure
   and reachable by a test: the caller has it from `stores/agents.js` before the
   row is drawn, and it is the configured agent's answer rather than whatever
   `agents::pick` actually started, for the reason `sessionMenu.js` records
   about the same substitution.

   Three of the four ask nothing about what kind of row this is: the panel is
   one flat list on purpose, and a live session, a start and an offline record
   can be pinned, dragged and closed alike — closing an offline row takes its
   record away rather than ending a process, which is `AgentList`'s caller's
   business and not this file's, and closing every other row is the same verb
   spent on every row but one. Clearing is the one that has to know, because it
   is the one that writes into a process, and a row with none is refused here
   rather than at the wire.

   The pin's label is the act and not the state: a row already pinned offers the
   way back out, which is the whole of what tells somebody the mark is theirs to
   remove. `branchMenu.js`'s favourite row is the same rule, written down there
   first.

   `others` is the one argument none of the row's own facts can answer: how
   many rows a `close-others` press would actually reach is a fact about the
   whole panel, not about the row the menu is open on, so it arrives counted
   rather than computed here — `closableOthers(...).length`, from a caller that
   has the whole list. This module stays pure and does not read it twice. */
export function agentMenuItems({
  pinned = false,
  conversation = null,
  starting = false,
  state = null,
  clearable = false,
  others = 0
} = {}) {
  /* Asked in this order because a pinned row cannot also be one with no
     conversation — pinning is what needed the id in the first place — so the
     two never compete, and the pin's own refusal is the only one it has. */
  const pinReason = pinned || conversation ? null : AGENT_REASON.noConversation
  const closeReason = pinned
    ? AGENT_REASON.pinned
    : starting
      ? AGENT_REASON.starting
      : null

  /* The harness first, because it is a fact about the whole project rather than
     about this row: if the configured agent cannot be told at all, every row
     says the same thing and there is nothing about any one of them left to
     explain. `terminal::service`'s own arm asks in this same order, so a row
     refused twice over is worded the same on both sides of the wire.

     Then the row: a start has no process yet, an offline row has none any more,
     and a live one that is waiting is the one case where the command would be
     read as something else entirely. */
  const clearReason = !clearable
    ? AGENT_REASON.cannotClear
    : starting
      ? AGENT_REASON.starting
      : OFFLINE_STATES.includes(state)
        ? AGENT_REASON.offline
        : state === 'needs-you'
          ? AGENT_REASON.waiting
          : null

  /* Nothing about this row refuses it — the row the menu is open on is never
     one of its own targets, so the only way it has nothing to do is a panel
     that gave it nothing: `others` counted to zero. */
  const closeOthersReason = others === 0 ? AGENT_REASON.nothingElse : null

  return [
    {
      kind: 'pin',
      label: agentMenuLabel(pinned ? UNPIN_LABEL : PIN_LABEL, pinReason),
      icon: 'pin',
      disabled: Boolean(pinReason)
    },
    {
      kind: 'clear',
      label: agentMenuLabel(CLEAR_LABEL, clearReason),
      icon: 'eraser',
      disabled: Boolean(clearReason)
    },
    {
      kind: 'close',
      label: agentMenuLabel(CLOSE_LABEL, closeReason),
      icon: 'x',
      disabled: Boolean(closeReason)
    },
    {
      kind: 'close-others',
      label: agentMenuLabel(CLOSE_OTHERS_LABEL, closeOthersReason),
      icon: 'x',
      disabled: Boolean(closeOthersReason)
    }
  ]
}
