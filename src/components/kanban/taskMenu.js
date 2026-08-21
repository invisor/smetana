/* What a card's overflow menu offers, as a rule rather than as a template.

   The `branchChoice.js` / `columnOrder.js` / `taskStages.js` family: pure, no
   Vue and no DOM, which is the whole reason it is a file of its own — no test
   in this repository can reach a `.vue`, so a rule left inside the component
   is a rule nothing checks.

   It also owns `STATUSES`, which `TaskInspector` used to keep. The three a
   person is offered and the rule for appending a fourth now exist in one copy;
   two copies would have drifted the first time bd grew a status. */
import { statusLabel } from '../status/status.js'
import { isParked, READY } from './parked.js'

/* The three a person is given, and no more. bd has eleven statuses in this
   build and most of them are an agent's business: `in_progress` is claimed by
   whoever starts work, `hooked` says an agent owns the molecule, `deferred`
   and `blocked` are answers to questions this menu is not asking. */
export const STATUSES = [
  { value: READY, label: 'Ready' },
  { value: 'pinned', label: 'Pinned' },
  { value: 'closed', label: 'Done' }
]

/* An issue may well hold a status outside those three — an agent moves it to
   in_progress, bd's own tooling to hooked. Leaving it out would draw a list
   with nothing checked in it, which reads as an issue holding no status at
   all. So it is appended when it is not already there: the value is bd's own
   string, since that is what would go back to bd, and the label is written the
   way the three above are, so a `parked` cannot sit in lower case under them.

   No status at all is the one thing that is not a status to append. `TaskCard`
   defaults `bdStatus` to '', so a caller that forgets it would otherwise draw a
   fourth row labelled by `statusLabel('')` — the empty string — checked and
   refused: a blank line in a menu, with a check beside it. Nothing on screen
   would say what it was. The three are the honest answer there, none of them
   checked, which is exactly what "we were not told" looks like. */
export const statusOptions = (bdStatus) => {
  if (!bdStatus) return STATUSES
  return STATUSES.some((s) => s.value === bdStatus)
    ? STATUSES
    : [...STATUSES, { value: bdStatus, label: statusLabel(bdStatus) }]
}

/* How wide the menu may get, and it is a measurement rather than a taste.

   It lives with the rule rather than with either caller, because the number is
   a property of the menu: `TaskCard` opens it from a card's header and
   `DesktopApp` from the header of the Task & details panel, over the very same
   items. A second copy of the measurement would have drifted from the first the
   day somebody re-measured one of them.

   `ContextMenu` draws itself as wide as its widest row and clips at this
   ceiling with an ellipsis; a menu row has no tooltip and no `title`, so
   whatever does not fit is gone with no way back. The ordinary menu is four
   short verbs and is nowhere near this number — what buys it is the one row
   that can carry a sentence. The longest label the menu can produce is the
   greyed Run row, which carries `scopeBusyReason`'s whole sentence — the reason
   moved out of the card's play tooltip, where it used to grow to fit, and into
   the row itself.

   Measured through CoreText at `--text-sm` (12px) in the system sans, which is
   what `--font-sans` resolves to in the webview: "Run this — a run over task
   smetana-hth is already going" is 315px, and 337px for a 14-character issue
   id. `ContextMenu` spends 70px of its width on chrome before the label —
   2×`--border-w`, 2×`--space-2` of panel padding, 2×`--space-4` of row padding,
   the 14px icon column, the 14px gutter mirroring it and the two `--space-4`
   gaps around the label — so 424 leaves the label 354px. That covers every id
   up to about 14 characters with room to spare for the other two webviews'
   fonts, where Segoe UI and Noto Sans have their own metrics and none of this
   could be measured from here.

   Compact needs no number of its own: density shrinks the space scale and
   leaves `--text-sm` alone, so the chrome costs 60px there instead of 70 and
   the label is 10px wider than it is here. Comfortable is the binding case.

   The app-wide font size is the one thing this does not follow — it is a number
   in px and the type grows past it. The long reason fits to a `uiFontSize` of
   14 and is ellipsised above that, which is the same failure the old tooltip
   never had. Now that the ceiling only binds the row that reaches it, raising
   it would cost the ordinary four-verb menu nothing at all; what it would cost
   is the busy card at the top of the font range, whose one long row would hang
   further over the board than a 212px card has any business doing. So the
   number stays where the measurement put it.

   Costing nothing on a narrow board: the panel is fixed-position, right-aligned
   to the trigger and clamped to the window by `EDGE`, so it opens leftwards
   over the card and only a window under ~440px could not hold it — and only
   with the long row on it, since anything shorter never reaches the ceiling.
   That clamp is also what lets the copy in the Task & details header open at
   all: the right column's minimum is 240px (`RIGHT_MIN` in
   `views/panelWidths.js`), so the menu is wider than the panel it hangs in and
   simply opens leftwards over the board, the same as a card in the last
   column. */
export const MENU_W = 424

/* The card's play used to interpolate its reason into a tooltip, which grows to
   whatever it holds. A menu row grows too — `ContextMenu` sizes itself by its
   widest row — but only up to the caller's ceiling, past which the label is
   clipped with an ellipsis and a row has no tooltip and no `title` to recover
   the rest from. So the ceiling is measured rather than guessed against the
   longest sentence `runScopes.js` composes, and `MENU_W` above is where that
   measurement is written down. The fragment is lowercase, which is why it
   joins with a dash rather than as a second sentence. */
const runLabel = (reason) => (reason ? `Run this — ${reason}` : 'Run this')

export function taskMenuItems({ bdStatus, runnable, runBlockedReason, busy }) {
  /* A write in flight greys everything: a bd call takes about two seconds, and
     a live menu for those two seconds invites a second choice racing the
     first. */
  const frozen = Boolean(busy)

  return [
    /* First, above the play, and only on a parked card. A parked task is one an
       agent could not settle on its own, so answering is the thing to do with
       it and running it is the thing not to — which is why this row is here and
       the play below is dead. Absent rather than greyed everywhere else: the
       menu is five verbs, and a sixth that is dead on all but a handful of
       cards is a row a person learns to read past. */
    ...(isParked(bdStatus)
      ? [{
          kind: 'resolve',
          label: 'Answer questions',
          icon: 'message-circle-question-mark',
          disabled: frozen
        }]
      : []),
    {
      kind: 'run',
      label: runLabel(runBlockedReason),
      icon: 'play',
      /* Two different refusals, deliberately drawn the same. There is nothing
         to run on a done or blocked card, and nothing to run *now* while the
         scope is busy — but only the second has words, so the first is a bare
         "Run this" rather than a sentence ending in a dangling dash. */
      disabled: frozen || !runnable || Boolean(runBlockedReason)
    },
    {
      /* The verb, and only the verb. The label used to spell out the mechanism
         — "Ask agent to edit", since nothing in this app edits an issue's text
         in place and an agent session opens on it — and it was the longest row
         in a menu of one-word verbs, which is a sentence's worth of panel spent
         on something the person finds out the moment the terminal tab opens.
         `kind` still says `ask-agent`, because that is what the caller does.

         The glyph carries what the words dropped: a pen, not the robot, so the
         row reads as the action rather than as who performs it — `AgentList`
         and the scope bar are where `bot` means an agent. */
      kind: 'ask-agent',
      label: 'Edit',
      icon: 'square-pen',
      disabled: frozen
    },
    {
      /* A new task that comes off this one: the case is a task already done
         over which clarifications have since arrived. The dialog it opens is
         the ordinary New task dialog carrying this issue as a parent, and the
         agent files the new issue as depending on it — so it waits in Blocked
         until this one closes and lands in Ready the moment it does, with
         nothing stored anywhere.

         Live on every card and not only on a done one. A follow-up to work
         still in progress is an ordinary thing to want, and it simply waits;
         a row dead on all but a handful of cards is the thing this menu's
         resolve row is careful not to be.

         `git-branch-plus` rather than a plain plus: the row makes a new thing
         that comes off an existing one, and the plus alone is already the tab
         bar's "new agent, terminal or task". */
      kind: 'follow-up',
      label: 'Follow-up task',
      icon: 'git-branch-plus',
      disabled: frozen
    },
    {
      kind: 'move',
      label: 'Move to…',
      icon: 'corner-down-right',
      disabled: frozen,
      children: statusOptions(bdStatus).map((option) => ({
        kind: 'status',
        value: option.value,
        label: option.label,
        /* The one it already holds is checked and refused: choosing it is a
           write that changes nothing, and two seconds of a greyed board for
           nothing is worse than an option that cannot be pressed. */
        icon: option.value === bdStatus ? 'check' : undefined,
        disabled: frozen || option.value === bdStatus
      }))
    },
    { type: 'separator' },
    {
      kind: 'delete',
      label: 'Delete',
      icon: 'trash-2',
      tone: 'danger',
      disabled: frozen
    }
  ]
}
