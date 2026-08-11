/* The tracker's state in the front end. Components know only this store; it
   alone knows about Tauri. */
import { computed, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/* bd and the design system call the same thing by different names. RESERVED in
   status.js is ready/running/done, in bd it is open/in_progress/closed. The only
   overlap is blocked. Without the translation cards would lose their glyphs and
   drift into generated hash colours. Everything else, including custom
   statuses, goes into normalizeStatus as is — and that is the intended
   behaviour. */
const UI_STATUS = { open: 'ready', in_progress: 'running', closed: 'done' }

export const toUiStatus = (name) => UI_STATUS[name] ?? name

const OPEN = 'open'
const CLOSED = 'closed'
const BLOCKED = 'blocked'

/* The merge lock is coordination between two leads, not work: the `merging`
   skill creates an ordinary bd issue carrying this label to serialize merges,
   and nobody using the app needs to see it. So it is left out of every list on
   screen — the board below, the claimed tasks in terminals.js and a remembered
   selection in views/DesktopApp.vue — and nowhere else.

   The label is one string with copies in several places, and the copies are not
   equal. In code it is here and `LOCK_LABEL` in src-tauri/src/runs/queue.rs,
   which keeps the lock out of a run's queue. The bundled skills under
   src-tauri/resources/smetana/skills/ carry the literal as well —
   merging/SKILL.md is the one that mints it (`bd create ... -l smetana-lock`),
   and the skills that must never take a lock as work name it in order to avoid
   it. So renaming the label has a direction: the minting side has to move, or
   the board goes on growing locks under a string nothing here hides. The
   duplication itself is accepted, the same way terminals.js holds the
   `smetana-run-` actor prefix beside terminal/model.rs; the cost of drift is a
   card reappearing on the board, not lost data.

   The filter is on the way out to the interface, never on the way in.
   `trackerState.issues` keeps the lock whole, because `holds` below treats an
   issue missing from the board as a satisfied blocker and queue.rs deliberately
   leaves the lock in the blocking set so that anything wired to depend on it
   fails closed. Dropping it from the store would quietly undo both. */
export const LOCK_LABEL = 'smetana-lock'

export const isLockIssue = (issue) => Boolean(issue?.labels?.includes(LOCK_LABEL))

export const trackerState = reactive({
  ready: false,
  /* A project switch is under way. While it is, deltas are ignored: they may
     belong to the old folder or to the new one, and the truth arrives as the
     command's answer — a snapshot in full. */
  switching: false,
  generation: 0,
  columns: [],
  issues: new Map(),
  health: { state: 'ok' },
  lastError: null
})

export const issueById = (id) => trackerState.issues.get(id)

/* Parentage in bd is expressed as a parent-child dependency, and that lands in
   dependency_count. Blockers cannot be counted from the counters — every child
   issue would get a false "blocked by 1". We count edges of type blocks; bd
   gives only the outgoing ones, so we assemble the reverse side ourselves.

   The ids and not merely how many: a card's tooltip names the tasks that block
   it, and "1 task blocks this one" is the one thing a person looking at a
   blocked card already knows. The counts below are `.length` of these, so the
   number and the names cannot disagree — they are one fact projected twice. */
const dependencyEdges = computed(() => {
  /* A blocker only blocks while it is unfinished, so a closed one is dropped
     here and the card stops being blocked the moment it closes. One absent from
     the board counts as satisfied too — the same rule `runs/queue.rs` applies,
     and the alternative is a card stuck behind a reference nobody can resolve. */
  const holds = (id) => trackerState.issues.has(id) && trackerState.issues.get(id).status !== CLOSED

  const blockedBy = new Map()
  const blocking = new Map()
  for (const issue of trackerState.issues.values()) {
    const edges = (issue.dependencies ?? []).filter((d) => d.type === 'blocks' && holds(d.depends_on_id))
    if (edges.length) blockedBy.set(issue.id, edges.map((d) => d.depends_on_id))
    for (const edge of edges) {
      const downstream = blocking.get(edge.depends_on_id) ?? []
      downstream.push(issue.id)
      blocking.set(edge.depends_on_id, downstream)
    }
  }
  return { blockedBy, blocking }
})

export const boardColumns = computed(() => {
  const { blockedBy, blocking } = dependencyEdges.value
  const buckets = new Map(trackerState.columns.map((c) => [c.name, []]))

  for (const issue of trackerState.issues.values()) {
    /* Before the bucketing rather than inside one column: a free lock is `open`
       and would sit in Ready, a held one is `in_progress` and would sit in
       Running, so there is no single column to hide it from. */
    if (isLockIssue(issue)) continue

    const blockedByIds = blockedBy.get(issue.id) ?? []
    const blockingIds = blocking.get(issue.id) ?? []

    /* Blocked is a column, not a status anybody writes. bd has no `blocked`
       status on these issues — they are `open` with an unfinished blocker, and
       `bd ready` works out the difference on every query rather than storing
       it. The board does the same: closing a blocker moves its dependants into
       Ready by itself, with nothing to update and nothing that can be left
       stale. Storing it instead would put a write between a blocker closing and
       the work becoming available. */
    const column = blockedByIds.length && issue.status === OPEN ? BLOCKED : issue.status

    // A status that is not in bd's set still has to be visible.
    if (!buckets.has(column)) buckets.set(column, [])
    buckets.get(column).push({
      id: issue.id,
      title: issue.title,
      status: toUiStatus(column),
      /* bd's own word, untranslated: the card's badge is the tracker's
         vocabulary, not the design system's, and a custom type has to survive
         the trip to be drawn at all. */
      type: issue.issue_type ?? undefined,
      blockedBy: blockedByIds.length,
      blockedByIds,
      blocks: blockingIds.length,
      blockingIds,
      spawnedFrom: issue.parent ?? undefined
    })
  }

  return [...buckets].map(([name, tasks]) => ({ status: toUiStatus(name), tasks }))
})

function applyDelta(delta) {
  if (delta.columns) trackerState.columns = delta.columns
  for (const issue of delta.upserted) trackerState.issues.set(issue.id, issue)
  for (const id of delta.removed) trackerState.issues.delete(id)
  trackerState.generation = delta.generation
}

/* Back-end errors are diagnostics: their text speaks bd's language and is
   addressed to whoever fixes things, not to whoever works. The user is shown a
   short explanation of what exactly did not work, and the full text stays in
   the console. Reads and writes also stopped sharing one caption: a read error
   under a "could not save" heading lied about what was happening. */
const ERRORS = {
  read: {
    title: 'Could not read the tracker',
    description: 'The board may be out of date. It will catch up on the next change.'
  },
  write: {
    title: 'Could not save to the tracker',
    description: 'Nothing was written. The board shows what the tracker has.'
  }
}

function report(kind, error) {
  console.error(`[tracker] ${kind} failed:`, error)
  trackerState.lastError = ERRORS[kind]
}

function applySnapshot(snapshot) {
  trackerState.columns = snapshot.columns
  trackerState.issues.clear()
  for (const issue of snapshot.issues) trackerState.issues.set(issue.id, issue)
  trackerState.generation = snapshot.generation
  trackerState.ready = true
}

/* tracker_resync can now reject (bd failed) — in that case it must not be
   treated as a success and the state must not be wiped: we leave the board as
   it was and remember the error. */
export async function resync() {
  try {
    applySnapshot(await invoke('tracker_resync'))
    trackerState.lastError = null
  } catch (err) {
    report('read', err)
  }
}

/* A project switch. The command's answer is the new folder's snapshot in full,
   so we roll it out the way resync() does: with a clear, otherwise the previous
   project's issues would stay on the board. */
export async function setProject(path) {
  trackerState.switching = true
  try {
    applySnapshot(await invoke('tracker_set_project', { path }))
    trackerState.lastError = null
  } catch (err) {
    report('read', err)
  } finally {
    trackerState.switching = false
  }
}

/* bd init in the active project's directory. Success brings the board; a
   refusal goes upwards — the caller has something to show the person. */
export async function initBd() {
  trackerState.switching = true
  try {
    applySnapshot(await invoke('tracker_init'))
    trackerState.lastError = null
  } catch (err) {
    report('write', err)
    throw err
  } finally {
    trackerState.switching = false
  }
}

/* Whether these folders have a tracker is a question for the filesystem, not
   for bd. */
export async function probeProjects(paths) {
  try {
    return await invoke('tracker_probe', { paths })
  } catch (err) {
    console.error('[tracker] probe failed:', err)
    return paths.map((path) => ({ path, tracked: true }))
  }
}

/* health's message is diagnostics: it speaks bd's language. What goes to the
   interface is a short text derived from `state` alone, and the detail stays
   where it is looked for while debugging. */
function setHealth(health) {
  trackerState.health = health
  if (health.state !== 'ok') console.warn('[tracker] health:', health.state, health.message ?? '')
}

export async function initTracker() {
  await listen('tracker:health', (event) => {
    setHealth(event.payload)
  })
  /* The generation grows by one with every delta. A gap means an event was
     lost — we take a snapshot in full. */
  await listen('tracker:delta', (event) => {
    if (trackerState.switching) return
    const delta = event.payload
    if (trackerState.ready && delta.generation > trackerState.generation + 1) {
      resync()
      return
    }
    applyDelta(delta)
  })

  /* tracker:health fires microseconds after start — before the webview manages
     to subscribe. The listener above catches everything that happens
     afterwards; this command is the only way to learn the state that was sent
     before the subscription. */
  try {
    setHealth(await invoke('tracker_health'))
  } catch (err) {
    report('read', err)
  }

  /* The snapshot and the deltas travel by different routes: while the
     command's answer flies back to the webview, the watcher manages to send a
     delta and advance the generation. The snapshot is then the past, and it
     must not be rolled out. It would put old values over new ones, lose
     deletions the delta had already applied, and roll the generation counter
     back; the next delta usually fixes that with a gap in the numbering, but if
     the tracker has gone quiet there will not be one, and Rust already holds
     the new value — its full sweep will see no discrepancy and send nothing.
     So a stale snapshot is ignored entirely, and a fresh one replaces the
     state the way resync() does: with a clear, otherwise those same deletions
     are lost.

     Ignoring it cannot be the end of it, though. What the board holds at that
     point is one delta's worth of issues and nothing else — the watcher had no
     reason to mention any of the rest. The gap check will not repair it,
     because from here the generations run consecutively, and neither will the
     back end, for the reason just given. So the discarded snapshot is asked
     for again. A second delta can overtake that request as well — nothing here
     stops it — but ready is true by the end of it, so the gap check is armed
     and this path is then no worse off than every other resync in this file.
     The first snapshot is the one that has nothing behind it at all, which is
     why the recovery belongs exactly here. It is awaited so ready is not
     announced over a board that is missing most of itself. */
  const snapshot = await invoke('tracker_snapshot')
  if (snapshot.generation >= trackerState.generation) applySnapshot(snapshot)
  else await resync()
  trackerState.ready = true
}

/* Entries in the Map are replaced wholesale rather than mutated, so a
   reference comparison says nothing about the contents — only a JSON-by-value
   comparison tells "the same thing" from "somebody changed it". */
function sameIssue(a, b) {
  return JSON.stringify(a) === JSON.stringify(b)
}

/* A write takes about two seconds. The optimistic value is applied at once —
   the user sees their edit without waiting, and that is the only indication of
   what is happening: there is no separate "in flight" mark on a card and there
   should not be, colour in this system belongs to status, not to the fact of a
   write.

   If the write fails, rolling back needs care: over those two seconds the card
   may have been updated by the watcher or by another write. We roll back only
   if the current value still equals what this very call wrote — a comparison by
   value, not by reference. If it is already different, somebody else's changes
   matter more than our rollback, and we simply remember the error. */
async function write(id, optimistic, run) {
  const before = trackerState.issues.get(id)
  const optimisticValue = before && optimistic ? { ...before, ...optimistic } : null
  if (optimisticValue) trackerState.issues.set(id, optimisticValue)
  trackerState.lastError = null

  try {
    const issue = await run()
    trackerState.issues.set(issue.id, issue)
    return issue
  } catch (error) {
    if (optimisticValue && sameIssue(trackerState.issues.get(id), optimisticValue)) {
      trackerState.issues.set(id, before)
    }
    report('write', error)
    throw error
  }
}

export function updateIssue(id, patch) {
  const optimistic = {}
  if (patch.title !== undefined) optimistic.title = patch.title
  if (patch.status !== undefined) optimistic.status = patch.status
  if (patch.priority !== undefined) optimistic.priority = patch.priority
  // bd's `-a` flag sets the assignee, and the issue carries `assignee` as its
  // own field beside `owner` — two different people (smetana-a5b): `owner` owns
  // the issue, `assignee` is whoever holds it right now, which is what a
  // `--claim` writes. This once applied the patch to `owner`, so an assignee
  // edit painted over the owner on screen until the delta arrived and undid it.
  if (patch.assignee !== undefined) optimistic.assignee = patch.assignee
  return write(id, optimistic, () => invoke('tracker_update', { id, patch }))
}

export function closeIssue(id, reason = null) {
  return write(id, { status: 'closed' }, () => invoke('tracker_close', { id, reason }))
}

export function reopenIssue(id) {
  return write(id, { status: 'open' }, () => invoke('tracker_reopen', { id }))
}

/* The one write whose result is an absence, so it cannot go through write():
   there is no issue coming back to put in the Map. The optimism is the same
   idea, inverted — the card goes at once and comes back if bd refused — and so
   is the care about the rollback: over those two seconds the watcher or another
   write may have put a newer version of the issue in, and restoring our stale
   copy over it would undo somebody else's change. If anything is there under
   that id when we come back, it is more current than what we removed. */
export async function deleteIssue(id) {
  const before = trackerState.issues.get(id)
  trackerState.issues.delete(id)
  trackerState.lastError = null

  try {
    await invoke('tracker_delete', { id })
  } catch (error) {
    if (before && !trackerState.issues.has(id)) trackerState.issues.set(id, before)
    report('write', error)
    throw error
  }
}
