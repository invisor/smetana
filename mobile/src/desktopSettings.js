/* What the phone takes from the desktop's `settings_load`, and what it does with it.

   The stores read their settings from one reactive object (`settings` in stores/settings.js),
   which on the desktop is filled by `loadSettings` - and that function also starts the watch
   that writes every change back with `settings_save`. The phone must neither write the desktop's
   settings nor call a command off its list, so it does not use `loadSettings`: it reads the
   answer itself and copies into the same object the few fields the screens and the stores they
   use actually read. The object is only ever written to here, and nothing watches it.

   Pure over its two arguments, so that what is taken and what is left can be tested. */
import { readKanban } from '../../src/components/kanban/boardView.js'

/* The sound a phone makes when the stores ring: none. A phone is told by a notification, and
   the stores ring `chime` on a run ending and on an agent starting to wait - the same two
   moments the push covers. */
const SILENT = { runFinished: 'off', needsAttention: 'off' }

export function applyDesktopSettings(target, stored) {
  const from = stored ?? {}
  target.openProjects = Array.isArray(from.openProjects) ? from.openProjects : []
  target.activeProject = from.activeProject ?? null
  /* The board's view: which columns, and over what period. */
  Object.assign(target.kanban, readKanban(from.kanban))
  /* The active project's own state: the column order, and how its last run was started. */
  if (from.project && typeof from.project === 'object') Object.assign(target.project, from.project)
  /* What decides whether an agent can be started as a conversation, and which. */
  target.conversationPanel = from.conversationPanel ?? target.conversationPanel
  target.agent = from.agent ?? target.agent
  target.model = from.model ?? target.model
  if (from.agentRoles && typeof from.agentRoles === 'object') {
    for (const [role, pair] of Object.entries(from.agentRoles)) {
      if (target.agentRoles[role] && pair && typeof pair === 'object') Object.assign(target.agentRoles[role], pair)
    }
  }
  Object.assign(target.notifications, SILENT)
  return target
}

/* The projects to offer, as rows: the folder's name, the path it is told apart by, and whether
   it is the one the desktop has open. Two projects can share a folder name, so the path is
   always kept. */
export function projectRows(openProjects, activeProject, name) {
  return (openProjects ?? []).map((path) => ({ path, name: name(path), active: path === activeProject }))
}
