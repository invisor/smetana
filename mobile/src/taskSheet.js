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

/* The rows of the sheet for one card: View task, then what the desktop's menu returns. On a
   desktop that cannot carry tracker writes the rows that are writes (lock, move, delete) and the
   separator that sets Delete apart are absent rather than grey: a row nothing can ever turn on
   is the one a person learns to read past, the same trade the paperclip makes (`canAttach`). */
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
  const rows = canWriteTracker(status)
    ? menu
    : menu.filter((row) => row.type !== 'separator' && !['lock', 'move', 'delete'].includes(row.kind))
  return [{ kind: 'view', label: 'View task', icon: 'eye' }, ...rows]
}
