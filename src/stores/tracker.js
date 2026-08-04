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
   gives only the outgoing ones, so we assemble the reverse side ourselves. */
const dependencyCounts = computed(() => {
  const blockedBy = new Map()
  const blocks = new Map()
  for (const issue of trackerState.issues.values()) {
    const edges = (issue.dependencies ?? []).filter((d) => d.type === 'blocks')
    if (edges.length) blockedBy.set(issue.id, edges.length)
    for (const edge of edges) {
      blocks.set(edge.depends_on_id, (blocks.get(edge.depends_on_id) ?? 0) + 1)
    }
  }
  return { blockedBy, blocks }
})

export const boardColumns = computed(() => {
  const { blockedBy, blocks } = dependencyCounts.value
  const buckets = new Map(trackerState.columns.map((c) => [c.name, []]))

  for (const issue of trackerState.issues.values()) {
    // A status that is not in bd's set still has to be visible.
    if (!buckets.has(issue.status)) buckets.set(issue.status, [])
    buckets.get(issue.status).push({
      id: issue.id,
      title: issue.title,
      status: toUiStatus(issue.status),
      /* bd's own word, untranslated: the card's badge is the tracker's
         vocabulary, not the design system's, and a custom type has to survive
         the trip to be drawn at all. */
      type: issue.issue_type ?? undefined,
      blockedBy: blockedBy.get(issue.id) ?? 0,
      blocks: blocks.get(issue.id) ?? 0,
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
     The board would stay wrong, silently. So a stale snapshot is ignored
     entirely, and a fresh one replaces the state the way resync() does: with a
     clear, otherwise those same deletions are lost. */
  const snapshot = await invoke('tracker_snapshot')
  if (snapshot.generation >= trackerState.generation) applySnapshot(snapshot)
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
  // The patch says `assignee` because that is bd's `-a` flag; the issue says
  // `owner` because that is what bd emits. The two names are the same person.
  if (patch.assignee !== undefined) optimistic.owner = patch.assignee
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
