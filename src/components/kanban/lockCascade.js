/* Locking a task by hand, and the cascade an epic carries across its children.

   The `taskMenu.js` / `parked.js` / `readyPromote.js` family: pure, no Vue and
   no DOM, which is what makes it the part of this a test can reach — a `.vue`
   file is the one thing no test in this repository can open.

   A lock is bd's own stored `blocked` status, not a label: `bd ready` already
   refuses an issue held there, so a run cannot claim a locked task even while
   it sits in Ready, with nothing new to teach the queue or the skills that
   read it. Writing the status is `DesktopApp.vue`'s; this module only says
   which ids to write it to, and in which order.

   Locking an epic locks every open descendant with it, deepest first, epic
   last — so there is never a moment where the epic is `blocked` while one of
   its children is still `open` and a run could still be reasoning about it as
   the epic's own unfinished work. Unlocking reverses both halves: the epic
   writes `open` first and its locked descendants follow, so there is never a
   moment where a descendant has already come back before the epic that was
   holding it has. */

/* Coordination between two leads, not work — the same exclusion `boardColumns`
   makes in `stores/tracker.js`. A small copy of the label rather than an
   import: this family carries no Vue and no Tauri, and importing the store to
   read one string would pull both in for a boolean. The duplication is the
   same trade `stores/tracker.js`'s own header already accepts for this label
   — the cost of drift is a lock issue treated as an ordinary one, not lost
   data. */
const LOCK_LABEL = 'smetana-lock'
const isLockIssue = (issue) => Boolean(issue?.labels?.includes(LOCK_LABEL))

const OPEN = 'open'
const BLOCKED = 'blocked'

const childrenOf = (issues, id) => issues.filter((issue) => issue.parent === id)

/* Every issue this module is willing to read, the merge lock filtered out. */
const realIssues = (issues) => [...issues].filter((issue) => !isLockIssue(issue))

/* The whole subtree under `id`, deepest first: for every direct child, its own
   descendants are collected before the child itself, so a parent never
   precedes a child still waiting to be visited. Siblings keep the order
   `issues` was given in, which is the tracker snapshot's own order — nothing
   here re-sorts it. */
function deepDescendants(issues, id) {
  const out = []
  for (const child of childrenOf(issues, id)) {
    out.push(...deepDescendants(issues, child.id))
    out.push(child)
  }
  return out
}

/* The mirror image of the traversal above: every child before its own
   descendants, so a parent always precedes a child still waiting to be
   visited — which is what lets `unlockIds` below put `id` in front of
   everything else and keep the same guarantee one level down, rather than
   only at the root. Siblings keep the order `issues` was given in, the same
   as `deepDescendants`; reversing that function's own list was tried and
   rejected, because reversing the whole list also reverses the order between
   unrelated siblings, which is not what "shallowest first" is asking for. */
function shallowDescendants(issues, id) {
  const out = []
  for (const child of childrenOf(issues, id)) {
    out.push(child)
    out.push(...shallowDescendants(issues, child.id))
  }
  return out
}

/* Which ids Block would write, in the order to write them.

   Every open descendant, deepest first, then `id` itself if it is open too —
   a task with no children returns just itself, so the one caller in
   `DesktopApp.vue` needs no separate case for a lone task against an epic.
   A descendant sitting anywhere else — claimed (`in_progress`), merged and
   waiting on review (`ready_to_merge`), already `parked`, `deferred`,
   `closed`, `pinned`, `hooked` or a project's own custom status — is left
   exactly where it is: a run has already claimed it, or it is out of `bd
   ready`'s reach for a reason of its own, and this feature answers only for
   what a run could otherwise take. */
export function lockIds(issues, id) {
  const list = realIssues(issues)
  const ids = deepDescendants(list, id)
    .filter((issue) => issue.status === OPEN)
    .map((issue) => issue.id)
  const root = list.find((issue) => issue.id === id)
  if (root && root.status === OPEN) ids.push(id)
  return ids
}

/* Which ids Unblock would write, in the order to write them.

   `id` itself first, if it is locked — the epic, on the case this exists for
   — and then every descendant still carrying the lock, shallowest first, so
   nothing comes back before the epic — or the intermediate parent, one level
   further down the same tree — that was holding it. What happens to a
   descendant once it is written `open` — landing in Ready or staying in the
   computed Blocked column behind an unfinished `blocks` dependency — is
   `boardColumns`' rule and not this module's. */
export function unlockIds(issues, id) {
  const list = realIssues(issues)
  const root = list.find((issue) => issue.id === id)
  const ids = root && root.status === BLOCKED ? [id] : []
  ids.push(
    ...shallowDescendants(list, id)
      .filter((issue) => issue.status === BLOCKED)
      .map((issue) => issue.id)
  )
  return ids
}

/* Whether an ancestor of `id` — its parent, or any issue further up the
   `parent` chain — is itself locked. This is what greys Unblock on a
   descendant of a locked epic, and the same fact greys Ready in that card's
   own Move to… — one lock, read the same way through two doors of the same
   menu. It answers only about ancestors, never about `id` itself: a locked
   epic is not its own parent, and asking the wrong question here would grey
   the epic's own Unblock row along with its children's. */
export function parentBlocked(issues, id) {
  const list = realIssues(issues)
  const byId = new Map(list.map((issue) => [issue.id, issue]))
  let current = byId.get(id)
  while (current?.parent) {
    const parent = byId.get(current.parent)
    if (!parent) return false
    if (parent.status === BLOCKED) return true
    current = parent
  }
  return false
}
