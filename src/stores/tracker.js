/* The tracker's state in the front end. Components know only this store; it
   alone knows about Tauri. */
import { computed, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { orderCards } from '../components/kanban/cardOrder.js'

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
  /* What can be done about a folder the operating system is refusing:
     `'reset'`, `'full-disk-access'` or `'unavailable'`, Rust's `AccessRepair`.

     A property of the **folder** and not of the build, so it is asked again on
     every project switch: `~/Desktop/a` has a button and `~/code/b` has a
     sentence about System Settings, in the same launch.

     `'unavailable'` until Rust says otherwise, deliberately: it is the form that
     offers nothing and claims nothing, and a button drawn where nothing can be
     reset is worse than the sentence alone — which is written to hold up
     without one (`views/folderAccess.js`). */
  folderAccessRepair: 'unavailable',
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
   number and the names cannot disagree — they are one fact projected twice.

   Exported because the command palette needs the same answer:
   `components/shell/commandPalette.js` draws a row's one relation from these
   maps rather than from the issue's own `dependencies` and `dependent_count`, so
   a closed blocker stops blocking in both places at once. */
export const dependencyEdges = computed(() => {
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
      /* bd's own word for what the issue holds, untranslated and not the
         column: Blocked is computed from an unfinished blocker and bd keeps
         such an issue at `open`, so a card's column is not a status anything
         could write back. The card's menu offers to move the issue, and this
         is the value it offers. */
      bdStatus: issue.status,
      /* bd's own word, untranslated: the card's badge is the tracker's
         vocabulary, not the design system's, and a custom type has to survive
         the trip to be drawn at all. */
      type: issue.issue_type ?? undefined,
      /* When bd last saw this issue change — the one of its three dates that is
         always there, which is what lets the board's period setting be one
         sentence with no hole in it (`components/kanban/boardView.js`). Carried
         as bd wrote it: the rule parses it, and a string this front end cannot
         read means show the card rather than hide it. */
      updatedAt: issue.updated_at,
      /* When bd closed the issue, passed through as bd wrote it — `null` and
         all, the same way `updatedAt` above is. It is the key the done column
         is ordered on (`components/kanban/cardOrder.js`), and the field is
         optional in the model, so the rule parses it and falls back rather
         than assuming it is there. Carried on the card and not looked up on
         the issue: the bucket holds cards, and reaching back into another
         structure for the sort key would be the one place here that does. */
      closedAt: issue.closed_at,
      blockedBy: blockedByIds.length,
      blockedByIds,
      blocks: blockingIds.length,
      blockingIds,
      spawnedFrom: issue.parent ?? undefined
    })
  }

  return [...buckets].map(([name, tasks]) => {
    const status = toUiStatus(name)
    return { status, tasks: orderCards(status, tasks) }
  })
})

function applyDelta(delta) {
  if (delta.columns) trackerState.columns = delta.columns
  for (const issue of delta.upserted) trackerState.issues.set(issue.id, issue)
  for (const id of delta.removed) trackerState.issues.delete(id)
  trackerState.generation = delta.generation
}

/* A diagnostic cut to the one line worth putting in front of somebody.

   The last non-empty line rather than the first: a diagnostic ends with what
   went wrong and opens with where it was noticed, and it is the end a person
   can act on. Nothing is lost by the cut — the whole text is still in the
   console, and the whole of the failure is what "Ask an agent" hands over.

   Here rather than in the view because two callers want it and one of them is a
   `.vue` file, which no test in this repository can reach: `views/DesktopApp.vue`
   draws it under the board, and `repairTracker` below puts it in the toast a
   failed repair raises. `String()` rather than a bare `.split`, because a
   rejection that never reached Rust is an `Error` object rather than the string
   Tauri serializes a `TrackerError` into. */
export function lastDiagnosticLine(error) {
  const lines = String(error ?? '')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
  return lines[lines.length - 1] ?? ''
}

/* Back-end errors are diagnostics: their text speaks bd's language and is
   addressed to whoever fixes things, not to whoever works. The user is shown a
   short explanation of what exactly did not work. Most of the full text stays
   in the console and only there; the exception is a repair, whose refusal is an
   answer to a press and carries its own words — see `repair` below. Reads and
   writes also stopped sharing one caption: a read error under a "could not
   save" heading lied about what was happening. */
const ERRORS = {
  read: {
    title: 'Could not read the tracker',
    description: 'The board may be out of date. It will catch up on the next change.'
  },
  write: {
    title: 'Could not save to the tracker',
    description: 'Nothing was written. The board shows what the tracker has.'
  },
  /* A third caption, because neither of the two above is true of a repair.
     "Nothing was written" is the claim `write` makes, and this is the one call
     in the app that irreversibly migrates a database: `bd migrate` failing
     part-way may well have written, and the app has no way to know. So this
     says only what is certain — that the copy, if one was taken, is still
     there, since nothing anywhere removes one.

     Its description is the **tail** of a sentence: `repairTracker` puts the
     failure's own last line in front of it. That is not decoration. A failed
     repair does not reopen the folder, so the sixty-second sweep keeps running
     and its next `bd list` failure overwrites the health message — the line
     under the board reverts, up to a minute later, with nobody having done
     anything. "It is then what is true" holds for health, which is a live
     reading, and not for the answer to a button somebody just pressed, which
     has to outlive the next sweep. So the answer carries its own words. */
  repair: {
    title: 'Could not repair the tracker',
    description: 'Any copy it took is left where it is — nothing removes one.'
  },
  /* And a fourth, because none of the three above is about a permission. This
     call never reaches bd and never touches `.beads`: it asks the operating
     system to forget a stored refusal. So "the board may be out of date" and
     "nothing was written" are both beside the point, and what a person needs to
     know is that the folder is still refused and that nothing of theirs was
     involved either way. */
  access: {
    title: 'Could not reset the permission',
    /* The tail of a sentence, exactly as `repair` above is, and `resetFolderAccess`
       pins the refusal's own last line in front of it. Here that matters more
       than it does there, because the likeliest refusal is one this app wrote
       itself and is the only thing that says what is in the way: a run going in
       another project, which this window cannot see and could not otherwise
       name. A constant caption would turn that into "something went wrong". */
    description: 'The folder is still being refused. Nothing about the tracker or its data was touched.'
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
  /* And the same clear for the semantic answer, which is about the folder being
     left rather than the one being opened. It is the one thing in this store
     that speaks for the agent, so it must never say something the agent did not
     say: `answered` outliving its project leaves the list drawing "Nothing
     matched" about a project nobody asked about — a false statement where the
     old behaviour was merely a group that quietly vanished.

     Before the await and outside the try, so a switch that then fails leaves
     nothing stale behind either: whatever the tracker answers, the question
     this was the answer to is over. Deliberately not in `applySnapshot`, which
     a resync also goes through — that one is the same project's board arriving
     again, and dropping an answer under an unchanged question would be a
     different fault of the same shape. */
  clearSemantic()
  trackerState.switching = true
  try {
    applySnapshot(await invoke('tracker_set_project', { path }))
    trackerState.lastError = null
  } catch (err) {
    report('read', err)
  } finally {
    trackerState.switching = false
  }
  /* After the switch and outside the try, because it is about the folder that
     is open now whether or not its board could be read — and a folder whose
     board could not be read is exactly the case this answer is for. */
  await loadFolderAccessRepair()
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

/* Take a copy of `.beads` and run bd's own migrations over the original.

   The board comes back with the answer and is rolled out here, exactly as
   `initBd` above rolls out what `tracker_init` returns: the worker reopens the
   folder itself once the migrations pass, so there is nothing left for the
   front end to ask for and a resync from here would only be a second full
   sweep of the same directory.

   A refusal is reported and then rethrown rather than swallowed, and that is
   the whole difference between this and `resync`. The caller is a button
   somebody pressed: it has to stop reading "Repairing…", and what bd said has
   to stay on the screen underneath — which it does, and it is now the
   migration's own words rather than an older failure's, because the worker puts
   a failed repair into health before it answers.

   Its own `ERRORS` entry rather than `write`'s: that one promises nothing was
   written, and this is the one call here that can have written and cannot
   know. */
export async function repairTracker() {
  trackerState.switching = true
  try {
    const result = await invoke('tracker_repair')
    applySnapshot(result.snapshot)
    trackerState.lastError = null
    return result
  } catch (err) {
    /* Set here rather than through `report`, and this is the one place in the
       file that builds a caption instead of picking one: `ERRORS.repair`'s
       description is the half that is always true, and the half that is about
       this attempt has to be pinned now — see the note on that entry. The
       console line is `report`'s job everywhere else, so it is kept by hand. */
    console.error('[tracker] repair failed:', err)
    const said = lastDiagnosticLine(err)
    trackerState.lastError = {
      ...ERRORS.repair,
      description: said ? `${said} ${ERRORS.repair.description}` : ERRORS.repair.description
    }
    throw err
  } finally {
    trackerState.switching = false
  }
}

/* The whole of the last tracker failure, for the session started to look at it.

   One call rather than four reads of things the store already half knows, and
   deliberately: bd is what is broken here, so nothing can be asked again once
   the agent has started, and a briefing pieced together from `health.message`
   and a path taken a moment later could describe two different moments. It is
   also where the bd version comes from — the app has exactly one copy of that
   number, and it is in Rust.

   It reports its own refusal, unlike a bare `invoke`, and that is not
   symmetry for its own sake. The caller has already moved the person to the
   agents panel and the terminal by the time this is awaited, so a rejection
   swallowed here is a button that takes somebody somewhere and then does
   nothing, with no line anywhere saying why. `read` is the right half of the
   pair — nothing is written by this call. */
export async function trackerFailure() {
  try {
    return await invoke('tracker_failure')
  } catch (err) {
    report('read', err)
    throw err
  }
}

/* Ask the operating system to forget its stored refusal of the project's
   folder, so that it asks the person again instead.

   Nothing here is about bd, which is the whole reason the state that offers it
   is not `error`: `.beads` is not opened, no migration runs, and there is no
   snapshot to roll out afterwards. On success the app restarts — Rust does that
   itself, as the second half of the repair — so this promise never settles: the
   window is on its way out and the caller's "Resetting…" goes with it.

   A refusal is reported and rethrown, the way `repairTracker` does it and for
   the same reason: the caller is a button somebody pressed, and it has to stop
   saying "Resetting…" when the app is still standing there. */
export async function resetFolderAccess() {
  try {
    await invoke('tracker_access_reset')
  } catch (err) {
    /* Built rather than picked, the same way `repairTracker` builds its own and
       for a sharper reason: every refusal this call can make is one Rust wrote
       for a person to read — a run going somewhere else, a folder no promptable
       permission governs, a `tccutil` that would not run — and the constant
       alone would throw away the only half that says which. The console line is
       `report`'s job everywhere else, so it is kept by hand. */
    console.error('[tracker] resetting the folder permission failed:', err)
    const said = lastDiagnosticLine(err)
    trackerState.lastError = {
      ...ERRORS.access,
      description: said ? `${said} ${ERRORS.access.description}` : ERRORS.access.description
    }
    throw err
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

/* health's message is diagnostics: it speaks bd's language, except where it is
   about the folder rather than about bd. Most of what goes to the interface is
   still a short text derived from `state` alone — the caption and the sentence
   under it are `HEALTH_NOTICE` in `views/DesktopApp.vue`, and
   `views/folderAccess.js` for the one state whose sentence depends on which
   folder it is about — but the message itself is no longer kept from the
   screen. Under
   `error` and under `folder-refused` the view draws its last non-empty line in
   mono beneath that sentence (`healthSaid`), and the whole of the failure goes
   to the agent that the second button on that screen starts. The console line
   below stays: it is the only trace for the states that draw no detail, and it
   carries the message unabridged. */
function setHealth(health) {
  trackerState.health = health
  if (health.state !== 'ok') console.warn('[tracker] health:', health.state, health.message ?? '')
}

/* What can be done about this project's refused folder, asked of Rust.

   Asked always rather than only when health says `folder-refused`: health can
   move to that state at any moment, and a notice that had to wait for a round
   trip before choosing between its three forms would draw the wrong one first.

   Asked again on every project switch, because the answer is about the folder.
   The three other things that reopen a folder — `bd init`, a repair, the
   sixty-second sweep — cannot change the path, so they cannot change this.

   A question that never came back leaves `'unavailable'`, which is the safe way
   round and not an error worth a caption: the person still gets the notice, and
   that form still tells them what to do. */
async function loadFolderAccessRepair() {
  try {
    trackerState.folderAccessRepair = await invoke('tracker_access_repair')
  } catch (err) {
    trackerState.folderAccessRepair = 'unavailable'
    console.warn('[tracker] could not ask what a refused folder needs:', err)
  }
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

  await loadFolderAccessRepair()

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

/* The semantic tier's state. Separate from `trackerState` deliberately: that
   one is the board as bd left it, and this is a question somebody asked about
   it a moment ago. The same shape `vcs.js` gives the commit box's suggest
   button, for the same reason — the field needs to know the question is out,
   and needs the refusal as a sentence when it is refused. */
export const searchState = reactive({
  pending: false,
  error: null,
  ids: [],
  /* Whether an answer has actually come back. Separate from `ids` being empty,
     because those are two different facts and the list draws them differently:
     nothing asked yet draws no group at all, while `NONE` — the agent looked and
     nothing matched — draws the group with nothing in it. Without the
     distinction a `NONE` is a spinner that stops and a list that does not
     change, which is the same indistinguishability `oneshot.rs` refuses a
     zero exit with an empty stdout for. */
  answered: false,
  /* The question the request in flight was sent for, and `null` when there is
     none. This is the stale guard: see `searchSemantic` below. */
  query: null
})

/* A rejection as one sentence the list can draw.

   `OneshotError` serializes to the `{ kind, message }` shape every command in
   this app refuses with, and unlike a bd diagnostic that message is already
   written for a person — six named ways to fail, each one something they can
   act on — so it is passed through rather than translated. That is why this is
   not `report()` above: that one replaces bd's language with a short caption
   and raises the board's own error banner, and neither is right for a question
   about a search nobody else on screen is waiting on. */
function refusal(error) {
  if (error && typeof error === 'object' && typeof error.message === 'string') return error.message
  return String(error)
}

export function clearSemantic() {
  searchState.ids = []
  searchState.error = null
  searchState.answered = false
  /* Nulled rather than left where it was, and that is what makes a withdrawn
     question discard its own answer: an ask still out at this moment is about a
     query nobody can see any more, and the guard below is a comparison against
     this very field. */
  searchState.query = null
}

/* Ask the agent which tasks were meant.

   Two questions never overlap: a second press while one is out is dropped
   rather than queued. An empty answer is an answer and not a failure — it is
   what `NONE` comes back as, the agent having looked and found nothing.

   **Guarded on the query, exactly as `vcs.js`'s `suggestMessage` is guarded on
   its project and repository**, and for a sharper version of the same reason.
   That call is out for as long as a model takes to write a line; this one can
   be out for the whole ninety seconds of `oneshot`'s deadline, and typing while
   waiting is the ordinary thing to do — the field is a search field. Nothing
   here cancels a request already sent, so an answer that lands under a
   different question has to be dropped where it lands, or the "By meaning"
   group would be answering something the person can no longer see. That is the
   design's own rule, and the acceptance criterion written against it.

   `pending` is cleared in `finally` and not behind the guard: a stale answer
   still frees the field, and leaving the flag up would mean the one gesture
   that could fix the situation — asking again — was the one thing dropped. */
export async function searchSemantic(query) {
  const asked = query?.trim()
  if (!asked || searchState.pending) return
  searchState.pending = true
  searchState.error = null
  searchState.answered = false
  searchState.query = asked
  try {
    const ids = await invoke('tracker_search_semantic', { query })
    if (searchState.query !== asked) return
    searchState.ids = ids
    searchState.answered = true
  } catch (error) {
    /* Logged above the guard rather than inside `refusal`, and that order is
       the point: a request that failed while its query moved on is still a
       failure, and the console line is the only trace it will ever leave —
       the sentence itself is deliberately not drawn. */
    console.error('[tracker] semantic search failed:', error)
    if (searchState.query !== asked) return
    searchState.ids = []
    searchState.error = refusal(error)
  } finally {
    searchState.pending = false
  }
}
