/* Agent sessions on the front end. Components know only this store; only it
   knows Tauri exists.

   The split follows cost: session state arrives as events for every session
   at once — it is cheap, and needed even for an agent nobody is looking at.
   Output bytes flow only for the active session, and nothing here keeps
   them: their consumer is xterm.js, and the truth lives in the ring in
   Rust. */
import { computed, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { runsState } from './runs.js'
import { settings } from './settings.js'
import { isLockIssue, trackerState } from './tracker.js'

export const terminalState = reactive({
  sessions: [],
  /* Starts the worker has not answered yet. A session takes about a second to
     come back — a spawn, and on the first one of a run a login shell as well —
     and until it does there is nothing in `sessions` to draw, so the panel drew
     its empty state at the very moment somebody had just asked for an agent.
     Kept beside the sessions rather than inside them: everything else here
     treats that list as "what the worker has", and a row with no process
     behind it must not reach code that would send it an id. */
  starting: [],
  activeId: null,
  ready: false,
  project: null,
  lastError: null
})

/* The last start the worker answered for: `{ ticket, session }`, or null until
   one lands.

   This exists because one move of `activeId` is a *continuation* and every
   other is a repair, and from outside this file they are indistinguishable. A
   ticket becoming a session keeps its place in the panel and changes only its
   id; a project switch or a removed session moves the selection to a different
   row altogether. Both look like "activeId is not what it was", and the ticket
   is gone from `starting` by the time anyone could ask. So anything holding an
   id from before the swap — the right column's focus, in `DesktopApp.vue` —
   would have to treat a handover as a loss and let go of a row the person is
   still looking at. The store is the only party that knows which happened, so
   it says so, and says nothing about what should be done with it: what follows
   a selection is the view's business.

   One slot rather than a log: `activeId` names one row, so only the most recent
   handover can still be somebody's place in the panel, and keeping the rest
   would be a map that only ever grows. */
export const lastHandover = ref(null)

/* The id of the newest session a run started in this panel, or null until one
   arrives.

   A run's sessions are the one kind this window does not ask for: the run
   worker sends `TerminalRequest::Create` to the terminal worker itself
   (`src-tauri/src/runs/service.rs`), so there is no ticket, no handover and no
   caller — the whole of what the front end ever sees is a state event about a
   session that was not there a moment ago. Selection has to follow it here or
   nowhere, and "nowhere" is what it was: the row appeared unselected while the
   terminal went on showing whichever agent was there before, which after the
   first batch is one that has already exited.

   Announced as well as followed, for the reason `lastHandover` is: what the
   selection moving *means* for the rest of the screen — which centre tab is in
   front, which side panel — is the view's business, and this is the only party
   that can tell a run's own start from any other state event. One slot, same as
   above: only the most recent can still be somebody's place in the panel. */
export const lastRunStart = ref(null)

/* Whether an id names a start rather than a session. Ticket ids are strings and
   the worker's are numbers, so the two can never be confused for one another —
   which is what lets `activeId` carry either without anything having to ask
   which kind it is holding before comparing it. */
export const isStarting = (id) => typeof id === 'string'

/* The starts that belong to the project on screen. The same rule `upsert`
   applies to a session, and it is needed for the same reason: a spawn takes
   about a second, a person can switch project inside it, and a row for an agent
   starting somewhere else drawn under this project's name is exactly the
   confusion the session list refuses to allow. The start itself is not
   cancelled — it is somebody's agent and it keeps coming up; it simply belongs
   to the panel it was asked for. */
const visibleStarts = () =>
  terminalState.starting.filter(
    (ticket) => !terminalState.project || ticket.project === terminalState.project
  )

/* The session's internal state and the design system's status are different
   vocabularies, and the translation lives here, the way bd's status
   translation lives in tracker.js. */
export function toUiState(session) {
  if (session.state === 'exited') return session.exitCode === 0 ? 'done' : 'failed'
  if (session.state === 'starting') return 'running'
  if (session.state === 'idle') return 'ready'
  return session.state
}

/* Negative input is ordinary, not a bug to let through: the clock below
   ticks every thirty seconds, so a session created between ticks has a
   startedAt in the future of the time this row is measured against. Floor of
   a negative number rounds away from zero, which is how a fresh agent
   showed "-1h -1m". An agent's age is never less than nothing. */
export function formatElapsed(ms) {
  const minutes = Math.max(0, Math.floor(ms / 60000))
  const hours = Math.floor(minutes / 60)
  return hours ? `${hours}h ${String(minutes % 60).padStart(2, '0')}m` : `${minutes}m`
}

/* Ticks once every thirty seconds: the time in an agent's row is measured in
   tens of minutes, and second-level precision would serve nobody there.
   Started lazily from initTerminals(), not at module scope: the module
   loads once for a window's lifetime in the app, but the test harness
   reloads it per test, and an interval nobody clears would outlive every
   test that started one. */
const now = ref(Date.now())
let clockStarted = false

function startClock() {
  if (clockStarted) return
  clockStarted = true
  setInterval(() => (now.value = Date.now()), 30000)
}

/* What a start will call its work once it is a session, worked out from the
   very intent that is being sent. The `kind` tags on this side and on
   `SessionWork`'s are the same words by construction — `Intent::work` in
   `src-tauri/src/agents/mod.rs` maps one onto the other — so the placeholder
   row and the session's own row say the same thing, and the handover a second
   later changes nothing on screen. This function is the mirror of that one and
   has to keep agreeing with it field for field, which is why the draft is
   spelled out here rather than passed through: `issue_type` is the name bd and
   the dialog use, `issueType` is the name that comes back over the wire, and a
   placeholder holding the first would draw Auto over a type somebody chose for
   the one second before the session lands.

   What an intent carries and this does not is the agent's briefing rather than
   anything drawn: the paths of the images attached to a task, the brainstorming
   switch, a run's settings. */
const workOf = (intent) => {
  if (intent.kind === 'editTask') return { kind: 'editTask', id: intent.id }
  if (intent.kind === 'newTask') {
    return {
      kind: 'newTask',
      text: intent.draft.text,
      issueType: intent.draft.issue_type ?? null,
      priority: intent.draft.priority ?? null
    }
  }
  return { kind: intent.kind }
}

/* The prose half of a row's caption. Sentence case, and every one of them is
   what the session is *for* — the process behind it is `claude-7`, and that
   name is deliberately not on a row any more: five of them said nothing about
   who was doing what. */
const CAPTION = {
  bare: 'Agent',
  newTask: 'Creating a task',
  editTask: 'Editing',
  setup: 'Project setup'
}

/* The issues a run's session has taken, if this session is one of a run's.

   There is no channel that says so: the agent claims an issue by running
   `bd update <id> --claim` itself, which sets the assignee and moves it to
   in_progress, and the app hears about it only as the tracker changing under
   the watcher. So the connection is made here, from the two halves that are
   already on the front end — each run knows which session is working, the
   tracker knows what is in progress and under whom. An explicit "this session
   claimed this issue" would be steadier, and it needs the agent to tell the
   app; until then this is the honest reconstruction rather than a guess.

   The owner filter is what keeps two concurrent runs' rows apart: a run's
   session writes with its own bd actor (`BEADS_ACTOR`, smetana-4fh), and bd
   stamps that actor as the issue's owner, so "everything in_progress" — which
   was the whole filter while a project held one run — would caption both rows
   with both batches' work. The actor's shape is `run_actor` in
   src-tauri/src/terminal/model.rs, written out here a second time because Rust
   holds the only other copy; drift costs a caption going quiet, which the row
   survives as a bare "Agent".

   The merge lock is claimed under that same actor while a batch merges, and it
   is coordination rather than work, so it is left out here exactly as the board
   leaves it out — see `isLockIssue` in tracker.js.

   Sorted, so a second issue appearing does not reorder the first. */
function claimedBy(sessionId) {
  /* Before the find, not after: a run between batches carries `session: null`,
     and a null id would land on it. */
  if (sessionId == null) return []
  const run = runsState.runs.find((r) => r.session === sessionId)
  if (!run) return []
  const actor = `smetana-run-${sessionId}`
  return [...trackerState.issues.values()]
    .filter(
      (issue) => issue.status === 'in_progress' && issue.owner === actor && !isLockIssue(issue)
    )
    .map((issue) => issue.id)
    .sort()
}

/* A row's caption, in two pieces because they are set differently: `label` is
   prose and belongs in sans, `tasks` are identifiers and belong in mono. The
   component is what knows that; this only says which is which.

   A run with nothing claimed yet reads as a bare agent does, and that is the
   truth rather than a fallback — it is an agent, and there is no work to name
   until it takes some. Work this front end has never heard of lands there too:
   a row that says "Agent" is still a row. */
function captionOf(work, claimed) {
  const kind = work?.kind
  if (kind === 'editTask') return { label: CAPTION.editTask, tasks: [work.id] }
  if (kind === 'run' && claimed.length) return { label: null, tasks: claimed }
  return { label: CAPTION[kind] ?? CAPTION.bare, tasks: [] }
}

/* Everything a row says about the work behind it: the caption the agents panel
   draws, plus the two things the panel on the right needs to open that work
   when the row is picked.

   `work` rides whole rather than being unpacked into flags, the same choice
   `runsState.config` makes for the same reason: a kind this front end has never
   heard of must stay unrecognisable instead of quietly reading as one it knows.
   It is on the row rather than looked up from `terminalState.sessions` because
   half the rows are not sessions — a start has a ticket and no session behind it
   for about a second, and the draft has to be drawable in that second too.

   `claimed` is the run's own list, computed here so the caption and the right
   panel cannot disagree about it and so the tracker is walked once per row
   rather than twice. Empty for everything that is not a run, and for a run that
   has taken nothing yet. */
function describeWork(work, sessionId) {
  const claimed = work?.kind === 'run' && sessionId != null ? claimedBy(sessionId) : []
  return { work: work ?? null, claimed, ...captionOf(work, claimed) }
}

/* Sessions first, in the worker's own order, then whatever is still starting.
   A new session always takes the highest id, so a start belongs at the bottom
   both before and after it lands, and the row a person is watching does not
   move under them when it becomes real.

   A row carries what the panel draws and nothing else. It used to carry the
   process name — `claude-7` — and the pending question too, for the block the
   right panel drew over the task card; that block is gone, a person answers in
   the terminal itself, and neither field had another reader. `needs-you` does
   not depend on either: it comes from the session's own state, so the triangle
   in AgentList is untouched by their absence.

   Nothing here names the agent any more, which also means nothing on screen
   does: `agents::pick` may start something other than the configured agent,
   and `Session.agent` is the only record of what it picked. A start says what
   it is doing instead of an elapsed time, which is also why it needs no
   separate state: `running` already draws the live dot, and `starting` in the
   corner says the rest. */
export const agentRows = computed(() => [
  ...terminalState.sessions.map((session) => ({
    id: session.id,
    ...describeWork(session.work, session.id),
    state: toUiState(session),
    elapsed: formatElapsed(now.value - Date.parse(session.startedAt))
  })),
  ...visibleStarts().map((ticket) => ({
    id: ticket.id,
    ...describeWork(ticket.work, null),
    state: 'running',
    elapsed: 'starting',
    starting: true
  }))
])

/* Exactly one output subscriber exists at a time — the terminal view. A Set,
   not a single field, so unsubscribing never depends on who mounted last. */
const sinks = new Set()

export function subscribeOutput(cb) {
  sinks.add(cb)
  return () => sinks.delete(cb)
}

function push(bytes, meta = {}) {
  for (const sink of sinks) sink(bytes, meta)
}

function decode(base64) {
  const binary = atob(base64)
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i)
  return bytes
}

/* Worker errors are diagnostics: their text speaks the worker's language and
   is meant for whoever fixes things, not whoever is waiting on a session.
   The interface gets a short explanation of what didn't work; the raw text
   stays in the console — the same split tracker.js makes between its read
   and write errors. Two kinds cover most of it here too: reading (list,
   attach, detach, resize) and writing (create, remove, write).

   The third entry is keyed on the kind the Rust error itself carries rather
   than on ours, and it earns the exception by being the one failure in this
   list a person can act on. "Nothing was created" is true of a machine with
   no agent installed and tells them nothing — and since filing a task is now
   an agent session rather than a write into the tracker, that is no longer a
   missing convenience but the only way to put a card on the board. It is a
   function because the names of the agents looked for belong to Rust:
   agents::IDS is the only copy of that list and the error carries it in its
   message, so nothing here has to hold a second one. */
const ERRORS = {
  read: {
    title: 'Could not read the terminal',
    description: 'The session list may be out of date. It will catch up on the next change.'
  },
  write: {
    title: 'Could not complete the action',
    description: 'Nothing was created, removed, or sent.'
  },
  noAgent: (looked) => ({
    title: 'No coding agent is installed',
    description: `Smetana looked for ${looked} on your PATH. It starts one to file a task and to edit an issue, so install one and try again.`
  })
}

function report(kind, error) {
  console.error(`[terminal] ${kind} failed:`, error)
  /* A cause the worker named wins over the generic pair, when this store has
     words for it. The typeof guard, rather than a plain lookup, is what keeps
     a future Rust kind called `read` or `write` from resolving to the generic
     entry's own object and being called with it. */
  const cause = ERRORS[error?.kind]
  terminalState.lastError = typeof cause === 'function' ? cause(error.message) : ERRORS[kind]
}

/* The number of the last chunk delivered for the active session. A gap means
   a lost event — then we take the truth whole, the way the tracker takes a
   snapshot on a generation gap. */
let seq = 0
let attaching = null
/* Ticket ids only have to be unique within this window's lifetime, and nothing
   ever reads the number. */
let tickets = 0

/* Answers whether the session ended up in the list, which is not the same as
   whether it exists: one belonging to another project is dropped on purpose,
   and the caller has to know that so it does not point the selection at a row
   nobody can see. */
function upsert(session) {
  if (terminalState.project && session.project !== terminalState.project) return false
  const index = terminalState.sessions.findIndex((s) => s.id === session.id)
  if (index === -1) {
    terminalState.sessions.push(session)
    terminalState.sessions.sort((a, b) => a.id - b.id)
  } else {
    terminalState.sessions[index] = session
  }
  return true
}

export async function initTerminals() {
  startClock()
  await listen('terminal:state', (event) => {
    const session = event.payload
    /* Asked before the upsert, because the upsert is what makes it false: a
       session nobody has seen before is a start, anything else is one of the
       many state events a live session goes on emitting — a question, an exit —
       and following those would drag a person off whatever row they had picked
       every time the agent they are not watching moved.
       `upsert`'s own answer is the other half: a run belonging to another
       project is not in this panel, and selecting a row nobody can see would
       black the terminal out with no way back to it. */
    const arrived = !terminalState.sessions.some((s) => s.id === session.id)
    if (!upsert(session)) return
    /* Only a run's. Everything else with a start behind it is `createSession`'s
       to place, and it has a rule this cannot see — a person who picked another
       agent while one was starting keeps their place. */
    if (arrived && session.work?.kind === 'run') {
      terminalState.activeId = session.id
      lastRunStart.value = session.id
    }
  })
  await listen('terminal:removed', (event) => {
    /* A session the worker took out of its map, on anybody's ask. The case
       this listener exists for is a removal this window never asked for — a
       run killing the session of a batch that stopped on a question
       (smetana-8pe) — where without it the row would keep the session's last
       emitted state, `needs-you` with a question nobody can answer behind a
       process that is gone, and over a night those dead loud rows would
       accumulate past the 1–2 budget. After this window's own removeSession
       the row is already gone and both steps below are no-ops, which is what
       lets one event serve both callers. The selection repair mirrors
       removeSession's for the same reason it exists there: a selection left
       naming a vanished row would black the terminal out. */
    const { id } = event.payload
    terminalState.sessions = terminalState.sessions.filter((s) => s.id !== id)
    if (terminalState.activeId === id) {
      terminalState.activeId = terminalState.sessions.at(-1)?.id ?? null
    }
  })
  await listen('terminal:output', (event) => {
    const { id, seq: next, data } = event.payload
    if (id !== terminalState.activeId) return
    if (next !== seq + 1) {
      // Fired from an event listener, not awaited by anyone: attach() no
      // longer throws (it reports instead), but the .catch() stays as a
      // second line of defence — a lost event must never surface as an
      // unhandled rejection, whatever else about this function changes.
      attach(id).catch(() => {})
      return
    }
    seq = next
    push(decode(data))
  })
  terminalState.ready = true
}

/* A project switch can start while an earlier call is still awaiting its
   invoke — a click on a different project row, or the activePath watcher
   firing again before the first call's response lands. The response that
   arrives second is not necessarily the one that was asked for second, so
   whichever call's request no longer matches terminalState.project when it
   wakes up has lost the race and must drop its result outright, not merge
   it: a stale response written into `sessions` while `project` already
   names the new project would map old session ids onto a different
   project's agents, and clicking a row's remove button would kill the
   wrong project's process with no error anywhere — the same class of loss
   `stale` guards against in the files layer. There is no way to tell which
   of a stale response's rows are still valid without asking again, and
   asking again is exactly what the next loadSessions call already does. */
export async function loadSessions(project) {
  terminalState.project = project
  /* What the list already held when the question went out. Anything that
     appeared while it was travelling is younger than the answer, and the
     answer is not entitled to speak about it — see below. */
  const asked = new Set(terminalState.sessions.map((s) => s.id))
  try {
    const sessions = project ? await invoke('terminal_list', { project }) : []
    if (terminalState.project !== project) return
    /* A session started while this request was in the air is not in the reply:
       the worker had not made it yet when it was asked. Replacing the list
       wholesale would drop that session and, through the selection repair
       below, the person's place in it too — an agent started seconds ago would
       vanish from the panel and the terminal would say "No agent selected",
       with nothing anywhere to say why and nothing due to arrive that would put
       it back. So a row that both is missing from the reply and was not there
       when the reply was asked for is carried over instead of dropped. A row
       that was there before the question and is not in the answer really is
       gone, and goes. */
    const late = terminalState.sessions.filter(
      (s) => !asked.has(s.id) && !sessions.some((fresh) => fresh.id === s.id)
    )
    terminalState.sessions = sessions
    for (const session of late) upsert(session)
    if (!selected()) {
      terminalState.activeId = terminalState.sessions.at(-1)?.id ?? null
    }
    terminalState.lastError = null
  } catch (err) {
    if (terminalState.project !== project) return
    report('read', err)
  }
}

/* Whether what `activeId` names is still something a person can be looking at:
   a session in the list, or a start that has not answered yet. A start counts —
   it is the newest thing in the panel and the reason the human is watching at
   all, and repairing the selection past it would hand the terminal back to some
   older agent one moment before the new one arrives. */
function selected() {
  const id = terminalState.activeId
  if (id == null) return false
  if (isStarting(id)) return visibleStarts().some((ticket) => ticket.id === id)
  return terminalState.sessions.some((s) => s.id === id)
}

/* The one write that still rejects: its caller turns a failed spawn into
   something the human sees, and an agent asked for that never appeared needs
   to say why — swallowing the error here would leave nothing to show.

   The intent, not a prompt: which words reach the agent depends on which agent
   it is, and that decision lives in Rust, in agents/. The store's job is to
   say what the session is for. */
export async function createSession(project, intent = { kind: 'bare' }) {
  const ticket = { id: `start-${(tickets += 1)}`, project, work: workOf(intent) }
  terminalState.starting.push(ticket)
  /* Where the selection goes back to if nothing starts. Not "the last session"
     — that is a repair, and this is a person's own place in the panel, which a
     failed start has no business moving. */
  const before = terminalState.activeId
  terminalState.activeId = ticket.id
  try {
    const session = await invoke('terminal_create', { project, agent: settings.agent, intent })
    const kept = upsert(session)
    /* The handover, and the whole point of the ticket. Only if nobody has moved
       since: a person who picked another agent while this one was starting has
       said what they want to look at, and an answer arriving afterwards does not
       overrule them. `kept` is the other half — a project switch during the
       start leaves this session belonging to somewhere else, and pointing the
       selection at a row that is not in the panel would black the terminal out
       with no way back to it. */
    if (terminalState.activeId === ticket.id) terminalState.activeId = kept ? session.id : before
    /* Announced after the move, so anything reading both sees a consistent
       pair, and only when the session is actually in this panel: a start whose
       project was switched away from under it handed nothing over here, and
       saying it did would point a follower at a row that is not on screen.
       Announced whether or not the selection was on the ticket, because it is
       a fact about the panel rather than about the selection — who cares is
       for the reader to decide. */
    if (kept) lastHandover.value = { ticket: ticket.id, session: session.id }
    terminalState.lastError = null
    return session
  } catch (err) {
    /* Nothing started, so nothing is drawn: the row goes with the ticket in the
       `finally` below and the selection goes back where it was. The caller keeps
       its dialog open on this rejection and the toast says why — an agent that
       never existed must not be left on screen looking startable. */
    if (terminalState.activeId === ticket.id) terminalState.activeId = before
    report('write', err)
    throw err
  } finally {
    terminalState.starting = terminalState.starting.filter((t) => t.id !== ticket.id)
  }
}

export async function removeSession(id) {
  try {
    await invoke('terminal_remove', { id })
    terminalState.sessions = terminalState.sessions.filter((s) => s.id !== id)
    if (terminalState.activeId === id) terminalState.activeId = terminalState.sessions.at(-1)?.id ?? null
    terminalState.lastError = null
  } catch (err) {
    report('write', err)
  }
}

/* Attaching hands back the whole ring, and the subscriber must repaint from
   scratch: whatever it was showing before is either another session's past,
   or a piece of this same session already folded into the snapshot. */
export async function attach(id) {
  terminalState.activeId = id
  /* A start has no ring, no seq and no id the worker would recognise: asking
     about it would come back as `no session` and report a failure at the one
     moment nothing has failed. The view attaches again on its own when the
     selection becomes a real id, which is what the ticket exists to lead to. */
  if (isStarting(id)) return
  const current = invoke('terminal_attach', { id })
  attaching = current
  try {
    const { data, seq: at } = await current
    if (attaching !== current) return
    seq = at
    push(decode(data), { reset: true })
    terminalState.lastError = null
  } catch (err) {
    // A newer attach already overtook this one; its outcome, not this
    // rejection, is what the store and the screen should reflect.
    if (attaching !== current) return
    report('read', err)
  }
}

/* activeId carries two different meanings, and detach is only allowed to
   touch one of them. It is "which agent the human selected" — that has to
   survive leaving the terminal tab, because the agent list highlights its
   row from this same field, and switching tabs must not un-pick it. It is
   also "which session the worker is currently streaming to this window",
   and that is what a view's unmount ends: the worker must stop pushing
   output nobody is listening to.
   The id argument is what keeps that stop from misfiring: switching agents
   is two separate IPC calls with no ordering guarantee at the worker, so a
   detach must name the session it is leaving. Without a name — or if this
   function cleared activeId unconditionally — the old session's detach
   arriving after the new session's attach would leave the worker with no
   active session, and output for the session the human is now looking at
   would silently stop arriving. No error, no event — the terminal just
   goes still. Selection is not the transport's to forget, though: it stays
   whatever it was, so the next mount can reattach to it. */
export async function detach(id) {
  if (id == null || isStarting(id)) return
  try {
    await invoke('terminal_detach', { id })
    terminalState.lastError = null
  } catch (err) {
    report('read', err)
  }
}

/* Typing into a pane whose agent has not started is dropped rather than queued,
   and that is the same answer `terminal_create` gives by handing the prompt over
   as an argument instead of writing it after the spawn: there is no input to
   write to yet, and bytes sent into one go nowhere with nothing to say they
   did. */
export async function send(id, data) {
  if (isStarting(id)) return
  try {
    await invoke('terminal_write', { id, data })
    terminalState.lastError = null
  } catch (err) {
    report('write', err)
  }
}

export async function resize(id, cols, rows) {
  if (isStarting(id)) return
  try {
    await invoke('terminal_resize', { id, cols, rows })
    terminalState.lastError = null
  } catch (err) {
    report('read', err)
  }
}
