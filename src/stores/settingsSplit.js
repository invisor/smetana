/* The settings split, as the front end sees it: which of the settings the server owns, how a change
   to them becomes a patch, and how what other clients changed is folded in without losing what is
   still waiting here. Pure: no Vue, no Tauri, no DOM. `stores/settings.js` is the one caller.

   The rule is the server's (`crates/core/src/settings/split.rs`): a field is the server's when a
   worker on the server reads it, and the device's otherwise. The lists below are that file's, written
   out a second time because the two ends are different languages; `tests/stores/settingsSplit.test.js`
   holds this end to the shape the store has, and the Rust test pins the other.

   Everything here works on a *view*: the server's fields alone, in the shape of the settings file,
   with the one active project's slice under `projects`. A patch is a JSON merge patch (RFC 7386)
   between two views - an object merges key by key, `null` removes, anything else replaces. */

const clone = (value) => (value === undefined ? undefined : JSON.parse(JSON.stringify(value)))
const nil = (value) => value === null || value === undefined
const plain = (value) => value !== null && typeof value === 'object' && !Array.isArray(value)

/* The server's per-project fields the view carries. `usedAt` is the server's too, but it stamps it
   itself, so it never travels in a patch. */
const PROJECT_FIELDS = ['runSettings', 'agents']

/* Root fields that are the server's whole. */
const ROOT = [
  'agent',
  'agentLanguage',
  'taskLanguage',
  'commitLanguage',
  'reportLanguage',
  'agentPrompt',
  'model',
  'agentRoles',
  'conversationPanel',
  'subscription',
  'runFailover',
  'remote'
]

/* The server's half of a settings object, in the file's shape. Takes the store's `settings`, a
   snapshot of it, or the resolved view `settings_load` answers: they share one shape. */
export function serverView(s) {
  const active = s.activeProject ?? null
  const view = {
    git: { removeWorktrees: s.git?.removeWorktrees },
    openProjects: [...(s.openProjects ?? [])],
    lastProject: active,
    projects: {}
  }
  for (const key of ROOT) view[key] = clone(s[key])
  if (active) {
    const entry = {}
    for (const field of PROJECT_FIELDS) entry[field] = clone(s.project?.[field]) ?? null
    view.projects[active] = entry
  }
  return view
}

/* The view a `settings:changed` payload (the server half of the file, every project in it) comes to
   for the project open here. A section the payload lacks - an older server - keeps what `fallback`
   holds. */
export function viewFromHalf(half, active, fallback) {
  const view = clone(fallback)
  if (!plain(half)) return view
  for (const key of ROOT) {
    if (half[key] === undefined) continue
    view[key] = plain(half[key]) && plain(view[key]) ? { ...view[key], ...clone(half[key]) } : clone(half[key])
  }
  if (plain(half.git) && half.git.removeWorktrees !== undefined) view.git = { removeWorktrees: half.git.removeWorktrees }
  if (Array.isArray(half.openProjects)) view.openProjects = [...half.openProjects]
  view.lastProject = half.lastProject ?? null
  view.projects = {}
  if (active) {
    const entry = {}
    for (const field of PROJECT_FIELDS) entry[field] = clone(half.projects?.[active]?.[field]) ?? null
    view.projects[active] = entry
  }
  return view
}

/* What changed between two values, as a merge patch, or `undefined` for nothing. */
function diff(a, b) {
  if (nil(a) && nil(b)) return undefined
  if (nil(b)) return null
  if (plain(a) && plain(b)) {
    const patch = {}
    for (const key of new Set([...Object.keys(a), ...Object.keys(b)])) {
      const inner = diff(a[key], b[key])
      if (inner !== undefined) patch[key] = inner
    }
    return Object.keys(patch).length ? patch : undefined
  }
  return JSON.stringify(a) === JSON.stringify(b) ? undefined : clone(b)
}

/* The patch that takes `prev` to `next`, or `undefined` when they agree. `projects` is compared only
   for projects both views hold: a project that is in one view and not the other is a switch of the
   active project, not a removal, and a removal is never sent from here. */
export function diffViews(prev, next) {
  const { projects: before = {}, ...restPrev } = prev
  const { projects: after = {}, ...restNext } = next
  const patch = diff(restPrev, restNext) ?? {}
  const projects = {}
  for (const path of Object.keys(after)) {
    if (!(path in before)) continue
    const inner = diff(before[path], after[path])
    if (inner !== undefined) projects[path] = inner
  }
  if (Object.keys(projects).length) patch.projects = projects
  /* `writeView` keeps the project open here in the list whatever another client did to it. When that
     re-append is the only difference, it is this window's own bookkeeping and not an edit: sending it
     would put the project back in the list the other client took it out of, and be echoed back. */
  const open = next.lastProject
  if (patch.openProjects && open && !(prev.openProjects ?? []).includes(open)) {
    const without = (list) => (list ?? []).filter((path) => path !== open)
    if (JSON.stringify(without(prev.openProjects)) === JSON.stringify(without(next.openProjects))) {
      delete patch.openProjects
    }
  }
  return Object.keys(patch).length ? patch : undefined
}

/* `pending` without the keys of `refused`: the fields of a patch the server turned down. */
export function subtractPatch(pending, refused) {
  if (!plain(pending) || !plain(refused)) return undefined
  const out = {}
  for (const [key, value] of Object.entries(pending)) {
    if (!(key in refused)) out[key] = value
    else if (plain(value) && plain(refused[key])) {
      const inner = subtractPatch(value, refused[key])
      if (inner !== undefined) out[key] = inner
    }
  }
  return Object.keys(out).length ? out : undefined
}

/* Whether a failed call never reached the server's handler (the shell's own words for a connection
   that was down or lost), as against the server answering with a refusal. The first leaves a change
   waiting for the reconnect; the second drops it. */
export function transportFailure(error) {
  return typeof error === 'string' && /connection to the server|Not connected to the server/i.test(error)
}

/* RFC 7386, returning a new value. */
export function mergePatch(target, patch) {
  if (!plain(patch)) return clone(patch)
  const out = plain(target) ? clone(target) : {}
  for (const [key, change] of Object.entries(patch)) {
    if (change === null) delete out[key]
    else out[key] = mergePatch(out[key], change)
  }
  return out
}

/* Writes a view into the store's settings in place - the view captures `settings.layout` and the
   rest by reference, so nothing here replaces an object that is already held - and touches a field
   only when its value differs, so a watcher is woken only by a real change. `lastProject` is not
   written: which project is open is this window's own, followed through the project switch. */
export function writeView(target, view) {
  const set = (object, key, value) => {
    if (value === undefined) return
    if (JSON.stringify(object[key]) !== JSON.stringify(value)) object[key] = clone(value)
  }
  /* Every field of ROOT, however deep, from the one list: a field added there is applied from a
     broadcast and from the load without a second place to remember it. */
  const into = (object, source) => {
    for (const [key, value] of Object.entries(source ?? {})) {
      if (plain(value) && plain(object[key])) into(object[key], value)
      else set(object, key, value)
    }
  }
  for (const key of ROOT) {
    if (view[key] === undefined) continue
    if (plain(view[key]) && plain(target[key])) into(target[key], view[key])
    else set(target, key, view[key])
  }
  if (view.git) set(target.git, 'removeWorktrees', view.git.removeWorktrees)
  if (Array.isArray(view.openProjects)) {
    /* The project open here stays in the list, whatever another client did to it. */
    const list = [...view.openProjects]
    if (target.activeProject && !list.includes(target.activeProject)) list.push(target.activeProject)
    if (JSON.stringify(target.openProjects) !== JSON.stringify(list)) target.openProjects.splice(0, target.openProjects.length, ...list)
  }
  const entry = target.activeProject ? view.projects?.[target.activeProject] : null
  if (entry) {
    for (const field of PROJECT_FIELDS) set(target.project, field, entry[field] ?? null)
  }
}

/* Folds what the server now holds into what this window holds. `base` is the server's state as
   this window believes it to be, with its own patches still on the way laid over it; `pending` is
   what this window has changed beyond that. It is laid over the incoming state, so another client's
   change to one field never undoes this window's change to another. `drop` is a patch the server
   refused: its fields alone are taken from the server and the rest of what is pending is kept.

   Answers `{ next, baseline, pending }`: `next` to write into the store, `baseline` to hold from now
   on, and `pending` (a patch or `undefined`) so the caller knows a send is still owed. */
export function fold(base, current, incoming, { drop } = {}) {
  let pending = base ? diffViews(base, current) : undefined
  if (pending && drop) pending = subtractPatch(pending, drop)
  const next = pending ? mergePatch(incoming, pending) : clone(incoming)
  const kept = clone(incoming)
  /* Which project is open is this window's own; its baseline moves only when this window sends. */
  if (base) kept.lastProject = base.lastProject
  return { next, baseline: kept, pending }
}

/* One stored shape out of the two halves: the device's file for everything about this screen, the
   server's answer for everything a worker reads. Both are `settings_load`-shaped. A field the
   server's answer lacks is left to the device's copy and then to the defaults. */
export function combine(device, server) {
  const defined = (object) => Object.fromEntries(Object.entries(object ?? {}).filter(([, value]) => value !== undefined))
  const out = { ...device }
  out.git = { ...device.git, ...defined({ removeWorktrees: server.git?.removeWorktrees }) }
  for (const key of ROOT) {
    if (server[key] === undefined) continue
    out[key] = plain(server[key]) && plain(device[key]) ? { ...device[key], ...server[key] } : server[key]
  }
  for (const key of ['openProjects', 'activeProject']) {
    if (server[key] !== undefined) out[key] = server[key]
  }
  out.project = {
    ...device.project,
    ...defined({
      runSettings: server.project?.runSettings,
      agents: server.project?.agents,
      usedAt: server.project?.usedAt
    })
  }
  return out
}
