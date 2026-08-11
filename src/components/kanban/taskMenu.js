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

/* The card's play used to interpolate its reason into a tooltip, which grows to
   whatever it holds. A menu row grows too — `ContextMenu` sizes itself by its
   widest row — but only up to the caller's ceiling, past which the label is
   clipped with an ellipsis and a row has no tooltip and no `title` to recover
   the rest from. So the ceiling is measured rather than guessed against the
   longest sentence `runScopes.js` composes, and `TaskCard`'s `MENU_W` is where
   that measurement is written down. The fragment is lowercase, which is why it
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
       menu is four verbs, and a fifth that is dead on all but a handful of
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
