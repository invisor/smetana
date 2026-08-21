/* The worker's sessions on the front end. Components know only this store; only
   it knows Tauri exists.

   Not all of them are agents. A session is either a CLI agent, which the agents
   panel draws a row for and the centre's one Agent tab shows, or the person's
   own shell, which has no row at all and a centre tab of its own. `sessions`
   holds both, because the worker does and there is no second list to keep in
   agreement with it; `isShellSession` is the whole of the difference.

   The split follows cost: session state arrives as events for every session
   at once — it is cheap, and needed even for an agent nobody is looking at.
   Output bytes flow only for the active session, and nothing here keeps
   them: their consumer is xterm.js, and the truth lives in the ring in
   Rust. */
import { computed, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { basename } from '../paths.js'
import { runsState } from './runs.js'
import { settings } from './settings.js'
import { isLockIssue, trackerState } from './tracker.js'

/* The one word that tells a person's own shell from an agent, as `SessionWork`
   and `WorkKind` in `src-tauri/src/terminal/model.rs` spell it on the wire.

   A constant rather than the literal at each site, because two shapes carry it
   now and both have to read it the same: a session, whose `work` is the whole
   variant, and a `SessionMark`, which carries the variant alone. Two literals
   would be two things to rename, and the failure is silent in both directions —
   the rail counting shells, or the panel drawing them. */
const SHELL_WORK = 'shell'

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

/* Every session the worker holds, by id, reduced to the project it belongs to
   and the state it is in. Beside `terminalState.sessions` rather than inside
   it, and deliberately: that list is the active project's and must stay that
   way — the agents panel draws it, and a row for a project this window is not
   pointed at would offer a remove button that kills somebody else's process.
   This map is the other question, which only the project rail asks: which
   projects have something going on.

   Marks rather than a count per project. The events that maintain it deliver
   one session at a time, so a store holding only totals could not tell whether
   the session that just left `needs-you` was the last loud one in its project. */
const marks = reactive(new Map())

/* The one fact a project's tile draws, per project path: `loud` if something is
   waiting on somebody there, `live` if something is working, `idle` otherwise.

   `loud` wins over `live`: a project with something waiting on a person is the
   reason the rail exists, and it must not be hidden by another session in the
   same project getting on with its work.

   **A person's own shells are not counted**, and that is what `mark.kind` is
   on the mark for: a shell that rings the bell reaches `needs-you` exactly as
   an agent does, and while the mark carried only an id, a project and a state,
   such a shell lit its project's tile loud while the scope bar's counter — which
   filters through `isShellSession` — read zero, two numbers about one project
   on one screen. This map now drops the same population that one does, by the
   same word. What the tile still does not say is *which* agent or how many of
   which kind; `components/shell/projectState.js` holds the words beside it to
   what it can support, and its header carries that reasoning.

   `starting` counts as live for the reason it counts in `hasAgentSession`: a
   spawn takes about a second, and a tile that stayed grey through it would
   leave the button somebody pressed with no visible effect. `idle` counts as
   neither — it is a live process with nothing to say, which `toUiState` reads
   as `ready` and the design system reads as quiet. */
export const projectStates = computed(() => {
  const out = {}
  for (const mark of marks.values()) {
    if (mark.kind === SHELL_WORK) continue
    const row = (out[mark.project] ??= { state: 'idle', live: 0, loud: 0 })
    if (mark.state === 'needs-you') row.loud += 1
    else if (mark.state === 'running' || mark.state === 'starting') row.live += 1
  }
  for (const row of Object.values(out)) {
    row.state = row.loud ? 'loud' : row.live ? 'live' : 'idle'
  }
  return out
})

/* A path with no session the rail counts under it is `idle`, and that is an
   ordinary answer rather than a missing one: it is what every project reads as
   in a window that has just opened, what a project the worker has never touched
   reads as forever, and what a project holding nothing but the person's own
   shells reads as while they work in them. */
export const projectState = (project) => projectStates.value[project]?.state ?? 'idle'

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

/* Whether a session is the person's own shell rather than an agent. One
   variant of `SessionWork` is the whole of the difference — see
   `SessionWork::Shell` in `src-tauri/src/terminal/model.rs` — and it is asked
   here rather than spelled out at each of the three places that care, because
   the three have to agree: a shell has no row in the agents panel, does not
   count towards the centre's Agent tab, and gets a centre tab of its own.

   Written as "is a shell" and not "is an agent" on purpose: work this front end
   has never heard of is an agent, which is the reading that keeps a session
   visible. `projectStates` above asks the same question of a mark, which
   carries the kind without the work around it, and reads the same constant. */
export const isShellSession = (session) => session?.work?.kind === SHELL_WORK

/* The agent sessions, and the shells, out of the one list the worker keeps.
   Both are derived rather than stored for the reason the tab row is: a second
   array beside `sessions` would be a copy to hold in agreement with it, and
   `loadSessions` already replaces the one. */
const agentSessions = () => terminalState.sessions.filter((s) => !isShellSession(s))
export const shellSessions = computed(() => terminalState.sessions.filter(isShellSession))

/* Where the selection goes when what it named is gone: the newest agent, or
   nothing. The newest, because a new session always takes the highest id and so
   is the one most recently started; and never a shell, for the reason
   `selected` below gives. */
const lastAgent = () => agentSessions().at(-1)?.id ?? null

/* Whether this project has an agent at all — a live one, or one still coming
   up. What hangs off it is the centre's Agent tab (`hasAgentTab` in tabs.js):
   the tab exists exactly as long as there is an agent to draw in it.

   `starting` counts, and not for tidiness: a spawn takes about a second, and a
   tab that appeared only when the worker answered would leave the button
   somebody pressed with no visible effect for that second — the same reason
   `starting` exists for the panel at all. A shell has no start ticket to add
   here, and needs none: it opens a tab of its own, which is not this one. */
export const hasAgentSession = computed(
  () => agentSessions().length > 0 || visibleStarts().length > 0
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
  if (intent.kind === 'resolveConflict') {
    return { kind: 'resolveConflict', repo: intent.repo, theirs: intent.theirs }
  }
  if (intent.kind === 'newTask') {
    return {
      kind: 'newTask',
      text: intent.draft.text,
      issueType: intent.draft.issue_type ?? null,
      priority: intent.draft.priority ?? null,
      /* Spelled the same on both sides of the wire, unlike `issue_type`: the
         draft panel draws it as a row of its own, and a placeholder without it
         would drop that row for the second the start lasts and then grow it
         back when the session lands. */
      parent: intent.draft.parent ?? null
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
  /* Shorter than the menu row it is started from ("Answer questions"), and
     deliberately: the id sits beside it in mono, so the row already reads
     "Answering smetana-8av" and the word "questions" would only push the id
     toward the ellipsis. */
  resolveTask: 'Answering',
  /* The one caption about a repository rather than an issue. "Conflict" and
     not "Resolving a conflict": the identifiers beside it are what say which
     one, and a row 252px wide spends every character it has on them. */
  resolveConflict: 'Conflict',
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

   The actor filter is what keeps two concurrent runs' rows apart: a run's
   session writes with its own bd actor (`BEADS_ACTOR`, smetana-4fh), and a
   `--claim` stamps that actor as the issue's **assignee**, so "everything
   in_progress" — which was the whole filter while a project held one run — would
   caption both rows with both batches' work. The actor's shape is `run_actor` in
   src-tauri/src/terminal/model.rs, written out here a second time because Rust
   holds the only other copy; drift costs a caption going quiet, which the row
   survives as a bare "Agent".

   It is `assignee` and not `owner`, and confusing the two is the whole of
   smetana-a5b: `owner` is the issue's owner and a claim never touches it, so
   while this read `owner` the filter matched nothing on any board and every run
   row read a bare "Agent" — the reported symptom. Both fields ride on the issue
   (`Issue` in src-tauri/src/tracker/model.rs); only this one answers "who is
   holding it right now".

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
      (issue) => issue.status === 'in_progress' && issue.assignee === actor && !isLockIssue(issue)
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
  // The two that are about one named issue, and so caption themselves with it.
  if (kind === 'editTask' || kind === 'resolveTask') {
    return { label: CAPTION[kind], tasks: [work.id] }
  }
  /* The two identifiers this one is about, in mono beside the word: which
     repository — its folder's name, since the absolute path is most of a row
     on its own and the panel already says which project this is — and the
     branch that was being brought in. */
  if (kind === 'resolveConflict') {
    return { label: CAPTION[kind], tasks: [basename(work.repo ?? ''), work.theirs].filter(Boolean) }
  }
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

/* Agent sessions first, in the worker's own order, then whatever is still
   starting. The shells are not here at all: this list is the agents panel, and
   a shell is not an agent — it has no work, no state anybody draws and nothing
   to say about itself. It is on screen as a centre tab and nowhere else.

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
  ...agentSessions().map((session) => ({
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

/* How many of this project's agents are alive — the scope bar's agents counter,
   and the list above minus the rows that have finished.

   `exited` is the one state that does not count. A session that fell over
   yesterday is still a row somebody may want to read, which is why it stays in
   the list at all, but counting it as running is how a number in the bar stops
   meaning anything. Every other state counts, `needs-you` among them: an agent
   waiting for an answer is the reason a person is looking at this bar, and a
   counter that dropped by one the moment attention was demanded would be
   pointing away from the thing it exists to point at.

   Starts count as well, through the same `visibleStarts` the rows use rather
   than a second copy of the rule — a spawn takes about a second, the row is
   drawn for that second, and a counter that waited for the worker would
   disagree with the list beside it for exactly as long.

   Sessions come through `agentSessions` for the same reason and not through
   `terminalState.sessions`: that list holds the person's own shells too, and a
   shell is not an agent — it has no row in the panel this number is read
   against, so counting one would put the bar and the list one apart with
   nothing on screen to explain the difference. This is the same exclusion
   `agentRows` makes, through the same function deliberately: the two are one
   sentence in the product — "how many agents are running" and "which agents are
   running" — and a second spelling of "not a shell" here is exactly how they
   would come to disagree. That is not hypothetical. This counter and the shell
   sessions arrived on two branches at once and merged without a textual
   conflict, each correct alone, and the number was wrong the moment they met.

   Deliberately a count and not `agentRows.value.length`: the rows carry
   captions and elapsed times, `now` ticks every thirty seconds, and this number
   has no business being recomputed by the clock. */
export const liveAgentCount = computed(
  () => agentSessions().filter((s) => s.state !== 'exited').length + visibleStarts().length
)

/* The same agents, split the way the scope bar's headline needs them: how many
   are waiting on the person, and how many of the rest are alive.
   `components/shell/headline.js` turns this into the one sentence at the top of
   the window.

   Through `agentSessions` and `liveAgentCount`, deliberately, and this is the
   whole reason the computed lives here rather than in the view. The obvious
   source was `projectStates` above — the rail's map, which already answers
   `loud`/`live` per project — and it stays the wrong one. When this was
   written the reason was that the map counted a person's own shells: a shell
   that finished a build and rang the bell reached `needs-you` like any other
   session, and the headline would have said "1 agent needs you", loudly and
   with a triangle, about a project with no agent in it, beside an agents
   counter correctly showing nothing. The mark carries a kind now and the map
   drops shells by it, so that half is gone; what is left is the half below —
   the sentence and the counter beside it have to agree by construction, and the
   map is not what the counter is built from. It also holds no start tickets, so
   a project's first second would read as empty in the sentence and as one agent
   in the counter.

   `live` is the counter minus the waiting ones rather than a filter of its own,
   which is what keeps the sentence and the counter beside it in agreement by
   construction. They sit about one gap apart in the same bar and say the same
   noun, so any second spelling of "alive" here is a pair of numbers that
   disagree in front of somebody — the failure this file already carries a
   paragraph about, one merge too late. The subtraction is exact today, every
   `needs-you` session being one of the non-exited ones the counter adds up; the
   clamp is there so that if that ever stops being true the sentence goes quiet
   instead of announcing "-1 agents running". */
export const agentCounts = computed(() => {
  const loud = agentSessions().filter((s) => s.state === 'needs-you').length
  return { loud, live: Math.max(0, liveAgentCount.value - loud) }
})

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

   The third and fourth entries are keyed on the kind the Rust error itself
   carries rather than on ours, and they earn the exception by being the two
   failures in this list a person can act on. "Nothing was created" is true of a
   machine with no agent installed and tells them nothing — and since filing a
   task is now an agent session rather than a write into the tracker, that is no
   longer a missing convenience but the only way to put a card on the board. It
   is a function because the names of the agents looked for belong to Rust:
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
  }),
  /* `TerminalError::BadCwd` — a folder the file tree's menu asked for a shell
     in that is not a folder inside the project any more. Reachable without
     anybody doing anything odd: the tree is refreshed on window focus and not
     by a watcher, so a folder an agent deleted or renamed since the last listing
     is still a row somebody can right-click. The generic write error would say
     "Nothing was created, removed, or sent", which is true and tells them
     nothing about which of their two windows is out of date. */
  badCwd: () => ({
    title: 'That folder is gone',
    description:
      'Smetana could not start a shell there. The tree may be out of date — refresh it.'
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
/* Which session the worker is streaming output to this window, which is the
   *other* thing `activeId` used to mean. Splitting it out is what lets a
   terminal tab draw a shell without moving the selection in the agents panel:
   the pane says what it is attached to (`TerminalView`'s `sessionId` prop), and
   the field below says which agent a person picked. While one field served
   both, attaching to a shell would have highlighted a row that does not exist
   and taken the Agent tab off the agent it was showing.
   Not reactive, and not in `terminalState`: it is transport bookkeeping, the
   same as `seq` and `attaching` above it, and nothing draws it. */
let streaming = null
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
  /* The first read of the marks, and the only one. Everything after it arrives
     on the two listeners below, which the worker emits for every session of
     every project already. */
  try {
    for (const mark of await invoke('terminal_marks')) marks.set(mark.id, mark)
  } catch (err) {
    /* A rail whose dots are all idle is the cost, and it is the right cost:
       nothing else in this window depends on this read, and failing the whole
       of initTerminals over it would take the agents panel down with it. */
    console.error('[terminals] reading session marks failed:', err)
  }
  await listen('terminal:state', (event) => {
    const session = event.payload
    /* Before anything else, because `upsert` returns early for a session of
       another project and the mark is wanted for exactly those: the rail draws
       a dot for every project, and this window is pointed at one of them. */
    marks.set(session.id, {
      id: session.id,
      project: session.project,
      state: session.state,
      /* The path that matters most for the kind: the first read is one
         snapshot, and every shell opened after it arrives here. Left off, such
         a shell is an agent as far as `projectStates` is concerned and goes on
         lighting the tile for good. */
      kind: session.work?.kind
    })
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
    /* Beside the filter below, and for every project rather than this one: the
       worker announces a removal wherever it happened, and a mark left behind
       would keep a dot lit on a tile whose process is gone. */
    marks.delete(id)
    terminalState.sessions = terminalState.sessions.filter((s) => s.id !== id)
    if (terminalState.activeId === id) {
      terminalState.activeId = lastAgent()
    }
  })
  await listen('terminal:output', (event) => {
    const { id, seq: next, data } = event.payload
    /* The session this window attached to, and not the selected agent: the two
       are the same on the Agent tab and deliberately different on a terminal
       tab. The worker sends output for one session at a time anyway; this
       guard is what keeps the last chunks of the one just left off the screen
       of the one just opened. */
    if (id !== streaming) return
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
      terminalState.activeId = lastAgent()
    }
    terminalState.lastError = null
  } catch (err) {
    if (terminalState.project !== project) return
    report('read', err)
  }
}

/* Whether what `activeId` names is still something a person can be looking at:
   an agent in the list, or a start that has not answered yet. A start counts —
   it is the newest thing in the panel and the reason the human is watching at
   all, and repairing the selection past it would hand the terminal back to some
   older agent one moment before the new one arrives.

   An agent and not any session: `activeId` is the row a person picked in the
   agents panel, and a shell has no row there. One that ever landed in this
   field would highlight nothing in the panel while the Agent tab drew somebody
   else's shell — the two meanings this field used to carry, back again. */
function selected() {
  const id = terminalState.activeId
  if (id == null) return false
  if (isStarting(id)) return visibleStarts().some((ticket) => ticket.id === id)
  return agentSessions().some((s) => s.id === id)
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

/* A shell of the person's own. A worker session like any other and not an
   agent: no intent, no agent id, no profile — see `terminal_shell` in
   `src-tauri/src/terminal/commands.rs`.

   Nothing here touches `activeId`: a shell has no row in the agents panel to
   select, and the tab it opens comes from the session appearing in the list.
   That is also why there is no start ticket to match `createSession`'s. A
   ticket buys the second between the press and the worker's answer, and it buys
   it for a panel that would otherwise draw an empty state over an agent
   somebody had just asked for; a tab that is not there yet draws nothing at
   all, so there is nothing to cover.

   The refusal is reported and not rethrown, the way `removeSession`'s is: there
   is no dialog to keep open and nothing on screen to take back — the toast is
   the whole of what a caller could do with it.

   `cwd` is where inside the project the shell starts, as a path relative to the
   root. The file tree's menu is the only caller that names one, and `null` is
   what this function meant before there was one: the project's own root. It is
   checked on the Rust side with `resolve_within`, like every other path this app
   takes from the front end, so a path leading outside the project is refused and
   no session is made — see `shell_cwd` in `src-tauri/src/terminal/service.rs`.
   Not checked here as well: two copies of that rule is one to keep true, and the
   one that matters is the one standing next to the spawn. */
export async function createShell(project, cwd = null) {
  try {
    const session = await invoke('terminal_shell', { project, cwd })
    upsert(session)
    terminalState.lastError = null
    return session
  } catch (err) {
    report('write', err)
    return null
  }
}

export async function removeSession(id) {
  try {
    await invoke('terminal_remove', { id })
    terminalState.sessions = terminalState.sessions.filter((s) => s.id !== id)
    if (terminalState.activeId === id) terminalState.activeId = lastAgent()
    terminalState.lastError = null
  } catch (err) {
    report('write', err)
  }
}

/* Attaching hands back the whole ring, and the subscriber must repaint from
   scratch: whatever it was showing before is either another session's past,
   or a piece of this same session already folded into the snapshot. */
export async function attach(id) {
  /* Deliberately not `activeId = id`, which is what this used to do. Attaching
     is the transport's half; which agent is selected is the person's, and a
     shell in a terminal tab attaches without being either. The one caller that
     also means "select this" says so itself (`selectAgent` in
     DesktopApp.vue). */
  streaming = id
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

/* The two things a session can be to this window are two fields, and detach
   touches exactly one of them. `activeId` is "which agent the person picked":
   it has to survive leaving the terminal tab, because the agent list highlights
   its row from that same field, and switching tabs must not un-pick it — detach
   never touches it, and neither does attach. `streaming` is "which session the
   worker is pushing output to this window", and that is what a view's unmount
   ends: the worker must stop sending bytes nobody is listening to.

   The id argument is what keeps that stop from misfiring, on both sides of the
   wire. Switching sessions is two separate IPC calls with no ordering guarantee
   at the worker, so a detach must name the session it is leaving: without a
   name, the old session's detach arriving after the new session's attach would
   leave the worker with no active session, and output for the session the person
   is now looking at would silently stop arriving. No error, no event — the pane
   just goes still. `streaming` is cleared under the same condition and for the
   same reason: only when it still names the session being left. Clearing it
   unconditionally would drop the output of whichever session overtook this one
   on the floor, which is the same defect one field further in. */
export async function detach(id) {
  if (id == null || isStarting(id)) return
  /* Only if it is still this one. A detach and an attach reach the worker in no
     guaranteed order, and forgetting unconditionally would drop the output of
     the session that overtook this one on the floor — the front-end half of the
     very defect the id argument exists for. */
  if (streaming === id) streaming = null
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

/* A file dragged over the window, and where it was let go.

   Tauri intercepts file drops before the webview sees them — `dragDropEnabled`
   is on by default and this app leaves it on — so there is no `dragover` and no
   `drop` to listen for in a component: the gesture arrives as a window event
   carrying absolute paths. That is why the subscription is here at all, beside
   `watchDrops` in attachments.js, which is the same event read for the other
   consumer: only a store may import Tauri.

   What this hands over is a point in CSS pixels and the paths, and no opinion
   about whose drop it is. `payload.position` is physical, so it is divided by
   the device pixel ratio here — the one place that knows the event's units —
   and what comes out is measured from the top left of the viewport, which is
   exactly the coordinate space `document.elementFromPoint` reads. Deciding
   whether the point is inside a particular pane is the pane's own business, and
   has to be: two subscribers on one window event need no arbiter as long as a
   hit test cannot give them both the same drop, and it cannot.

   `paths` rides along with `over` too, because the enter event is the only one
   that carries them and a caller wanting to say how many are coming has nowhere
   else to read it; the events in the middle of a drag carry `null`.

   In a browser there is no webview to ask, and getCurrentWebview throws before
   the subscription — a normal mode, the same one attachments.js reads a throw
   as, so it is logged at debug and nothing else happens. */
export function watchSessionDrops({ over, leave, drop } = {}) {
  let webview
  try {
    webview = getCurrentWebview()
  } catch {
    console.debug('[terminals] no webview: drops are a Tauri-only gesture')
    return () => {}
  }
  let stop = null
  let stopped = false
  webview
    .onDragDropEvent(({ payload }) => {
      /* Anything that is not the drag being over the window ends it, which is
         `leave` and also whatever a future Tauri adds beside it: forgetting the
         response is the safe reading of an event this code does not know. */
      if (payload.type !== 'enter' && payload.type !== 'over' && payload.type !== 'drop') {
        leave?.()
        return
      }
      const { x, y } = payload.position.toLogical(window.devicePixelRatio)
      const at = { x, y, paths: payload.paths ?? null }
      if (payload.type === 'drop') drop?.(at)
      else over?.(at)
    })
    .then((unlisten) => {
      stop = unlisten
      /* The view unmounted while the subscription was still on its way. */
      if (stopped) stop()
    })
    .catch((err) => console.error('[terminals] listening for drops failed:', err))

  return () => {
    stopped = true
    if (stop) stop()
  }
}
