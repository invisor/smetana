/* What the phone takes from the desktop's `settings_load`, and what it does with it.

   The stores read their settings from one reactive object (`settings` in stores/settings.js),
   which on the desktop is filled by `loadSettings` - and that function also starts the watch
   that writes every change back with `settings_save`. The phone must neither write the desktop's
   settings nor call a command off its list, so it does not use `loadSettings`: it reads the
   answer itself and copies into the same object the few fields the screens and the stores they
   use actually read. The object is only ever written to here, and nothing watches it.

   The desktop's look is copied too, the theme and the density, but only from a desktop that
   keeps them on the server (`LOOK_MINOR`): an older one answers `dark` and `comfortable` out of
   its defaults, which the phone could not tell from a choice, so below it the phone keeps its own.
   The font size is never copied, by the person's decision: it is the phone's own
   (`phoneSettings.js`), as is any theme, density, column mode or period the person chose here,
   and that choice wins over the desktop's. This is the one place the priority is decided.

   Pure over its three arguments, so that what is taken and what is left can be tested. */
import { readKanban } from '../../src/components/kanban/boardView.js'
import { PHONE_DEFAULTS, resolveKanban, resolveLook } from './phoneSettings.js'

/* The sound a phone makes when the stores ring: none. A phone is told by a notification, and
   the stores ring `chime` on a run ending and on an agent starting to wait - the same two
   moments the push covers. */
/* The protocol minor from which the desktop's theme and density are the server's to answer. */
export const LOOK_MINOR = 14

const SILENT = { runFinished: 'off', needsAttention: 'off' }

export function applyDesktopSettings(target, stored, { minor = 0, phone = PHONE_DEFAULTS } = {}) {
  const from = stored ?? {}
  /* The desktop's pair: from its answer when it keeps them on the server, else what the target holds. */
  let desktopTheme = target.appearance.theme
  let desktopDensity = target.appearance.density
  if (minor >= LOOK_MINOR && from.appearance && typeof from.appearance === 'object') {
    if (typeof from.appearance.theme === 'string') desktopTheme = from.appearance.theme
    if (typeof from.appearance.density === 'string') desktopDensity = from.appearance.density
  }
  const look = resolveLook(phone, desktopTheme, desktopDensity)
  target.appearance.theme = look.theme
  target.appearance.density = look.density
  target.appearance.uiFontSize = phone.uiFontSize
  target.openProjects = Array.isArray(from.openProjects) ? from.openProjects : []
  target.activeProject = from.activeProject ?? null
  /* The board's view: which columns, and over what period. */
  Object.assign(target.kanban, resolveKanban(phone, readKanban(from.kanban)))
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
