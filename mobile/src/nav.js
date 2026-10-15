/* Where the working screens are and how a person moves between them: a pure rule, with no
   Vue in it, so that the one thing a notification does - open the right screen - can be
   tested without a screen.

   A route is `{ tab, project, stack }`. The four tabs are the roots; `stack` is what has been
   opened on top of the tab (a task, a conversation, the form for a new task) and its last
   entry is what is drawn. `project` is set only by a run notification: the Runs tab shows the
   project the desktop has open unless a notification named another one. Routes are never
   mutated: every function answers a new one, which is what lets a screen hold the route in
   one `ref` and be sure that a change is a change. */

export const TABS = [
  { id: 'board', label: 'Board', icon: 'columns-3' },
  { id: 'agents', label: 'Agents', icon: 'bot' },
  { id: 'runs', label: 'Runs', icon: 'play' },
  { id: 'projects', label: 'Projects', icon: 'folder' }
]

export const START = Object.freeze({ tab: 'board', project: null, stack: [] })

const isTab = (id) => TABS.some((tab) => tab.id === id)

/* The tab's own screen again, with whatever was on top of it taken off. A tab pressed while it
   is already in front goes back to its root, which is what a bottom bar does everywhere. */
export function selectTab(route, tab) {
  if (!isTab(tab)) return route
  return { tab, project: tab === 'runs' ? route.project : null, stack: [] }
}

/* One screen on top of the current one. */
export function open(route, entry) {
  return { ...route, stack: [...route.stack, entry] }
}

export function back(route) {
  return route.stack.length ? { ...route, stack: route.stack.slice(0, -1) } : route
}

/* The screen to draw: the top of the stack, or the tab's own. */
export function current(route) {
  return route.stack.length ? route.stack[route.stack.length - 1] : { screen: route.tab }
}

export const canGoBack = (route) => route.stack.length > 0

/* What a push notification's `target` names. The desktop writes three shapes:
   `run:<project path>`, `session:<id>` (a driven, conversation-panel session) and
   `terminal:<id>` (a PTY session, which the phone shows read-only). The two id spaces are
   separate counters on the desktop and both start at 1, so `session:3` and `terminal:3` are
   different sessions: the kind travels with the id and is never guessed from it. Anything
   else - an empty string, a shape from a newer desktop, an id that is not a number - is
   `null`, and the caller leaves the screen where it is: opening the wrong thing is worse than
   opening nothing. */
export function parseTarget(target) {
  if (typeof target !== 'string') return null
  const run = /^run:(.+)$/s.exec(target)
  if (run) return { kind: 'run', project: run[1] }
  const session = /^session:(\d+)$/.exec(target)
  if (session) return { kind: 'session', id: Number(session[1]) }
  const terminal = /^terminal:(\d+)$/.exec(target)
  if (terminal) return { kind: 'terminal', id: Number(terminal[1]) }
  return null
}

/* The route a notification opens. A run opens the Runs tab on that project; an agent waiting
   opens its conversation (`driven: true`) or its read-only terminal (`driven: false`) over the
   Agents tab, so that going back lands on the list. */
export function routeForTarget(target) {
  const parsed = parseTarget(target)
  if (!parsed) return null
  if (parsed.kind === 'run') return { tab: 'runs', project: parsed.project, stack: [] }
  return {
    tab: 'agents',
    project: null,
    stack: [{ screen: 'conversation', id: parsed.id, driven: parsed.kind === 'session' }]
  }
}
