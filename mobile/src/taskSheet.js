/* What the phone's task sheet offers for a card, as a rule rather than as a template.

   A tap on a board card opens a sheet that repeats the desktop's card menu with one row more on
   top, "View task". The menu's own rule is not copied: `taskMenuItems` is called with the same
   five inputs `DesktopApp.vue` feeds it, so a card in a given column offers on the phone exactly
   what it offers there. What this file adds is the inputs the desktop works out inside a
   component (`runnable`, `cardScope`, now shared from `kanban/runScope.js`), and the one thing
   that is the phone's own: a desktop too old to carry tracker writes.

   Pure, no Vue and no Tauri, like the rest of this family. */
import { taskMenuItems } from '@src/components/kanban/taskMenu.js'
import { cardScope, runOffered, runnable } from '@src/components/kanban/runScope.js'
import { parentBlocked } from '@src/components/kanban/lockCascade.js'
import { scopeBusyReason } from '@src/components/run/runScopes.js'
import { desktopMinor } from './bridge/states.js'

/* The first protocol minor whose desktop answers `tracker_update` and `tracker_delete` to a
   phone (smetana-jdw5). */
export const TRACKER_WRITES_MINOR = 13

/* Whether the desktop on the other end can carry a tracker write. */
export const canWriteTracker = (status) => desktopMinor(status) >= TRACKER_WRITES_MINOR

const list = (issues) => (issues instanceof Map ? [...issues.values()] : [...(issues ?? [])])

/* The caption under the rows when the desktop cannot carry tracker writes: three grey rows with
   one reason are one sentence, not three. */
export const OLD_SERVER_NOTE =
  "Block, Move to... and Delete need the desktop's server restarted: it is older than the desktop app."

/* '' when there is nothing to explain. */
export const sheetNote = (status) => (canWriteTracker(status) ? '' : OLD_SERVER_NOTE)

/* The rows of the sheet for one card: View task, then what the desktop's menu returns. On a
   desktop that cannot carry tracker writes the rows that are writes (lock, move, delete) stay,
   grey, and the separator that sets Delete apart stays with them: absent rows made a person
   conclude the feature did not exist, and the reason is told once by `sheetNote` instead. A grey
   Move to... keeps its children as they are, and a grey row does not open. The paperclip is the
   other trade (`canAttach`): there the control is absent. */
export function sheetRows({ card, issues, runs, configState, status, busy }) {
  const all = list(issues)
  const live = runnable(card, runOffered(configState))
  const menu = taskMenuItems({
    bdStatus: card.bdStatus,
    runnable: live,
    runBlockedReason: live ? scopeBusyReason(cardScope(all, card.id), runs) : '',
    busy,
    parentBlocked: parentBlocked(all, card.id)
  })
  const writable = canWriteTracker(status)
  const rows = writable
    ? menu
    : menu.map((row) => (['lock', 'move', 'delete'].includes(row.kind) ? { ...row, disabled: true } : row))
  return [{ kind: 'view', label: 'View task', icon: 'eye' }, ...rows]
}
