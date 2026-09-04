/* What a row in the agents panel offers on a secondary click.

   The `branchMenu.js` / `taskMenu.js` / `sessionMenu.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — a `.vue` file
   is the one thing no test in this repository can reach, so a rule left inside
   the component is a rule nothing checks. It is also one half of a pair nothing
   mechanical joins: these `kind` strings are matched by hand in
   `AgentList.vue`, which turns each into an event of its own, the same seam
   `branchMenu.js` has with `BranchList.vue`. The test pins this side.

   **Two verbs and no more.** The panel is 236px wide by default and a row is
   one line; everything else an agent can be asked to do is a gesture somewhere
   else in the app — starting one is the project tile's menu, watching one is
   the row's own click, answering one is the terminal. Clearing a session was
   asked for and put aside deliberately: that is a question about each harness's
   own command line and belongs with the profiles, not in the panel's menu.

   The order is pinning first and closing second, and it is the order of how
   often either is wanted rather than of how much either costs. Closing is last
   anyway for the reason every destructive row in this app is last.

   `pin` is the glyph — the same one the board's `pinned` status draws — and `x`
   is the close, which is the mark the row already carries for that verb. */

/* How wide the menu may get. A ceiling and not a width — `ContextMenu` draws
   itself as wide as its widest row and clips there with an ellipsis, and a menu
   row has neither a tooltip nor a `title`, so whatever does not fit is gone
   with no way back.

   The longest label this file can produce is `Pin to top — nothing to remember
   it by` at 38 characters, against `Close agent — it has not started yet` at 36
   and `Close agent` at 11. `sessionMenu.js` measured a menu row at 6.4px a
   character of `--text-sm` in `--font-sans` and `ContextMenu` at 70px of chrome
   around the label (`MENU_W` in `kanban/taskMenu.js` itemises where those pixels
   go), which puts the binding row at about 313px. 340 leaves the same seventh
   of headroom for Segoe UI and Noto Sans that the other three left, and the
   trade is the one every `MENU_W` in this app carries: px does not follow the
   app-wide font size, so a person running the interface large loses the tail of
   that one row.

   **The ceiling is what the refusals are worded against, not the other way
   round**, which is `sessionMenu.js`'s rule and it bites harder here: this menu
   opens over the narrowest panel in the app. So each fragment is as short as it
   can be and still be a reason. */
export const AGENT_MENU_W = 340

/* Why one of the two cannot be pressed, as lowercase fragments joined onto the
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
  starting: 'it has not started yet'
}

export const PIN_LABEL = 'Pin to top'
export const UNPIN_LABEL = 'Unpin'
export const CLOSE_LABEL = 'Close agent'

/* The label with its reason joined on, or the plain label when there is none.
   Exported because the greying and the wording are one rule and a caller
   drawing either without the other would be drawing half of it. */
export function agentMenuLabel(label, reason) {
  return reason ? `${label} — ${reason}` : label
}

/* The two rows.

   `pinned` and `starting` are facts about the row; `conversation` is the id the
   row carries, and its absence is what refuses the pin. Nothing here asks
   whether the row is a live session, a start or an offline record: the panel is
   one flat list on purpose, and every one of the three can be pinned, dragged
   and closed alike — closing an offline row takes its record away rather than
   ending a process, which is `AgentList`'s caller's business and not this
   file's.

   The pin's label is the act and not the state: a row already pinned offers the
   way back out, which is the whole of what tells somebody the mark is theirs to
   remove. `branchMenu.js`'s favourite row is the same rule, written down there
   first. */
export function agentMenuItems({ pinned = false, conversation = null, starting = false } = {}) {
  /* Asked in this order because a pinned row cannot also be one with no
     conversation — pinning is what needed the id in the first place — so the
     two never compete, and the pin's own refusal is the only one it has. */
  const pinReason = pinned || conversation ? null : AGENT_REASON.noConversation
  const closeReason = pinned
    ? AGENT_REASON.pinned
    : starting
      ? AGENT_REASON.starting
      : null

  return [
    {
      kind: 'pin',
      label: agentMenuLabel(pinned ? UNPIN_LABEL : PIN_LABEL, pinReason),
      icon: 'pin',
      disabled: Boolean(pinReason)
    },
    {
      kind: 'close',
      label: agentMenuLabel(CLOSE_LABEL, closeReason),
      icon: 'x',
      disabled: Boolean(closeReason)
    }
  ]
}
