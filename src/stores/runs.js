/* Whether the active project is set up for runs, and with what. The seventh
   file in this directory that knows Tauri exists; components see a reactive
   object and two computeds.

   Deliberately small, like git.js: this is a file read, there is no worker
   behind it, and freshness comes from switching projects, from window focus,
   and from any of this project's sessions starting or stopping work — the last
   being the only channel that fires while somebody watches a setup agent
   write the file without leaving the window. The rule for that one is
   components/run/configFreshness.js. */
import { computed, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { sameScope } from '../components/run/runScopes.js'

const NONE = { state: 'missing' }

export const runsState = reactive({
  project: null,
  /* The back end's own state object: missing | broken | ok. Kept as it
     arrived rather than unpacked into flags, so a state this front end has
     not heard of cannot silently read as one of the others. */
  config: NONE,
  /* Every `Run` the worker sent for this project, whole, oldest first — a
     project holds several at once now, and each is named by its own `token`,
     which is what tells one run's event from another's. Kept whole for the
     reason `config` is: the panel reads `state.kind` and `stopping`, and
     unpacking those into flags is where a state nobody has heard of starts
     reading as one somebody has. A stopped run stays in the list until the
     project changes or a run of the same scope replaces it, because the
     reason it stopped is what somebody came back to read. */
  runs: [],
  /* What this machine can drive a browser with, for this project — four facts,
     kept whole like the two above. Null means nobody has asked yet, and it is a
     third thing from "asked and found nothing": the run dialog opens before the
     answer lands, and blocking its live-check toggle on a fact nobody has
     established would switch it off on every open. `liveCheckBlock` in
     components/run/browserTools.js is where that distinction is spent. */
  browserTools: null
})

/* One run replaced or learned about. By token, which is the only name that is
   never two runs': a late event for a run that ended cannot land on the one
   that started after it. Insertion keeps the list oldest-first the way the
   worker answers `run_state`, so a bar segment does not jump when a response
   and an event interleave. */
function upsert(run) {
  const at = runsState.runs.findIndex((r) => r.token === run.token)
  if (at !== -1) {
    runsState.runs[at] = run
    return
  }
  runsState.runs = [...runsState.runs, run].sort((a, b) => a.token - b.token)
}

/* An offer to set the project up, not a warning: most projects are here, and
   `broken` is deliberately excluded — a file that exists and cannot be parsed
   is something to fix, and running the setup over it would write across
   somebody's work. The `project` check is not redundant with the `config`
   check: clearing the project resets `config` to the very same `missing`
   NONE a genuinely unconfigured project has, so without it "no project open"
   would read as "this project needs setting up" and the dialog would be
   offered for nothing. */
export const needsSetup = computed(() => runsState.project !== null && runsState.config.state === 'missing')

export const configError = computed(() =>
  runsState.config.state === 'broken' ? runsState.config.message : null
)

/* Guarded against its own stale response exactly as git.js and terminals.js
   are: two calls can be in flight with no ordering guarantee, and the last
   response winning over the last call would show one project's configuration
   under another project's name. */
export async function loadConfig(project) {
  /* The runs go with the project they belonged to, and they go here rather
     than in loadRun: this is the function that moves `runsState.project`, so
     this is the only moment at which a run on screen is provably somebody
     else's. Leaving them for loadRun to overwrite would show the old project's
     runs under the new name for as long as that call takes. */
  if (runsState.project !== project) {
    runsState.runs = []
    /* Per project the same way the run is: `.mcp.json` is the project's own file
       and busy-ness is about the runs beside this one, so an answer read for
       another project says nothing here. Cleared rather than kept as a guess —
       null is "nobody has asked", which is exactly the truth after a switch. */
    runsState.browserTools = null
  }
  runsState.project = project
  if (!project) {
    runsState.config = NONE
    return
  }
  try {
    const config = await invoke('project_config', { project })
    if (runsState.project !== project) return
    runsState.config = config
  } catch (err) {
    if (runsState.project !== project) return
    /* Not a folder's fault: every real outcome is a state, so reaching here
       means the call itself failed. We fall back to "not configured", which
       offers the setup rather than claiming a configuration we do not have. */
    console.error('[runs] reading the project config failed:', err)
    runsState.config = NONE
  }
}

/* The runs in this project, if any. Called on mount and on switching projects,
   and guarded the same way loadConfig is and for the same reason. The answer
   replaces the list wholesale: it is the worker's whole truth for this
   project, and anything on screen that is not in it has ended and been read —
   or belongs to a project this one no longer is. */
export async function loadRun(project) {
  if (!project) {
    runsState.runs = []
    return
  }
  try {
    const runs = await invoke('run_state', { project })
    if (runsState.project !== project) return
    runsState.runs = runs ?? []
  } catch (err) {
    console.error('[runs] reading the run state failed:', err)
  }
}

/* What there is on this machine to drive a browser with, and whether anything
   else is already driving one. Read when the run dialog opens rather than on
   every project switch: nobody needs it until they are looking at the toggle,
   and the answer includes busy-ness, which is only true for as long as it takes
   to read.

   Guarded against its own stale response exactly as loadConfig and loadRun are,
   and it matters more here than for most: the answer decides whether a control
   is disabled, so a slow read for a project somebody has already left would
   block the toggle in the project they moved to, over a machine state that was
   never about it. */
export async function loadBrowserTools(project) {
  if (!project) {
    runsState.browserTools = null
    return
  }
  try {
    const tools = await invoke('browser_tools', { project })
    if (runsState.project !== project) return
    runsState.browserTools = tools
  } catch (err) {
    /* Every real outcome is a state, so reaching here means the call itself
       failed. Back to null — "nobody has established anything" — which leaves
       the toggle live: a run that then fails inside its check is where things
       were before this existed, and it is the cheaper of the two mistakes
       against taking a working live check away over a broken IPC call. */
    console.error('[runs] reading what can drive a browser failed:', err)
    if (runsState.project !== project) return
    runsState.browserTools = null
  }
}

/* Start one. The settings object is passed through untouched — it is the
   shape Rust deserializes, snake_case included, and translating it here would
   put the field names in two places. Throws what the worker refused with, so
   the dialog can say which of its own fields is the problem: a project with no
   configuration, a damaged one, or settings that do not go together. */
export async function startRun(project, settings) {
  const run = await invoke('run_start', { project, settings })
  if (runsState.project === project) {
    /* The new run takes over its scope's slot: a stopped run of the same
       scope stays on screen only until its successor exists, exactly what the
       single-run store did by overwriting its one slot. Stopped runs of other
       scopes are left alone — their reasons have not been read against this
       start. */
    runsState.runs = runsState.runs.filter(
      (r) => !(r.state.kind === 'stopped' && sameScope(r.settings.scope, run.settings.scope))
    )
    upsert(run)
  }
  return run
}

/* Cooperative, and this returning is not the run being over: the batch in
   flight finishes first. What comes back has `stopping` set, and the run's own
   event says when it has actually stopped. Named by the run's token — the
   stop has to reach exactly the run whose button was pressed — and a `null`
   answer is a run that ended before the stop arrived, which changes nothing
   on screen. */
export async function stopRun(token) {
  const run = await invoke('run_stop', { token })
  if (run && run.project === runsState.project) upsert(run)
  return run
}

/* Subscribed once, at start-up, exactly as initTracker is — and like the
   tracker's health event, `run:state` can fire before the webview is
   listening, which is what `run_state` is for.

   The project check is the stale-response guard in its other form: an event
   is not a response to anything, so nothing orders it against a project
   switch, and a batch that ends just as somebody moves to another project
   would otherwise put its run under the new project's name. Per run the guard
   is the token: `upsert` lands the event on the run it names and no other,
   and the worker only ever emits for runs still in its map, so an ended run's
   late report cannot come back through here. */
export async function initRuns() {
  await listen('run:state', (event) => {
    const run = event.payload
    if (!run || run.project !== runsState.project) return
    upsert(run)
  })
}
