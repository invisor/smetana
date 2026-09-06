/* What a row in the agents panel offers on a secondary click.

   The `branchMenu.js` / `taskMenu.js` / `sessionMenu.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — a `.vue` file
   is the one thing no test in this repository can reach, so a rule left inside
   the component is a rule nothing checks. It is also one half of a pair nothing
   mechanical joins: these `kind` strings are matched by hand in
   `AgentList.vue`, which turns each into an event of its own, the same seam
   `branchMenu.js` has with `BranchList.vue`. The test pins this side.

   **Three verbs and no more.** The panel is 236px wide by default and a row is
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

   The order is pinning, clearing, closing. Pinning first because it is wanted
   most often, closing last because every destructive row in this app is last,
   and clearing between them because it is neither: it deletes nothing — the
   transcript stays a file and the Sessions tab goes on listing it — which is
   also why it asks for no confirmation where `DeleteSessionModal` must.

   `pin` is the glyph — the same one the board's `pinned` status draws — `x` is
   the close, which is the mark the row already carries for that verb, and
   `eraser` is the clear, deliberately not the bin: the bin is deletion and
   these two verbs must not look alike. */

/* How wide the menu may get. A ceiling and not a width — `ContextMenu` draws
   itself as wide as its widest row and clips there with an ellipsis, and a menu
   row has neither a tooltip nor a `title`, so whatever does not fit is gone
   with no way back.

   The longest label this file can produce is `Clear session — this agent cannot
   do it` at 39 characters, against `Pin to top — nothing to remember it by` and
   `Clear session — it has not started yet` at 38 apiece, `Close agent — it has
   not started yet` at 36 and `Close agent` at 11. `sessionMenu.js` measured a
   menu row at 6.4px a character of `--text-sm` in `--font-sans` and
   `ContextMenu` at 70px of chrome around the label (`MENU_W` in
   `kanban/taskMenu.js` itemises where those pixels go), which puts the binding
   row at about 320px. 340 leaves the same seventh
   of headroom for Segoe UI and Noto Sans that the other three left, and the
   trade is the one every `MENU_W` in this app carries: px does not follow the
   app-wide font size, so a person running the interface large loses the tail of
   that one row.

   **The ceiling is what the refusals are worded against, not the other way
   round**, which is `sessionMenu.js`'s rule and it bites harder here: this menu
   opens over the narrowest panel in the app. So each fragment is as short as it
   can be and still be a reason — the clear row's four are what the ceiling cost
   most, since its label is two characters longer than the close's and its
   refusals had to fit under the same 42. */
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
     one. */
  cannotClear: 'this agent cannot do it'
}

export const PIN_LABEL = 'Pin to top'
export const UNPIN_LABEL = 'Unpin'
export const CLEAR_LABEL = 'Clear session'
export const CLOSE_LABEL = 'Close agent'


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

/* The three rows.

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

   Two of the three ask nothing about what kind of row this is: the panel is one
   flat list on purpose, and a live session, a start and an offline record can
   be pinned, dragged and closed alike — closing an offline row takes its record
   away rather than ending a process, which is `AgentList`'s caller's business
   and not this file's. Clearing is the one that has to know, because it is the
   one that writes into a process, and a row with none is refused here rather
   than at the wire.

   The pin's label is the act and not the state: a row already pinned offers the
   way back out, which is the whole of what tells somebody the mark is theirs to
   remove. `branchMenu.js`'s favourite row is the same rule, written down there
   first. */
export function agentMenuItems({
  pinned = false,
  conversation = null,
  starting = false,
  state = null,
  clearable = false
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
    }
  ]
}
