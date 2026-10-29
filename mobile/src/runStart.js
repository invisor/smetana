/* The form the phone starts a run from, and whether one may be started at all.

   The phone mirrors the desktop's "Run the queue" window (`RunModal.vue`, scope `queue`): the same
   fields, in the same order, filled from the same three sources when the form is opened, and
   turned into the same `run_start` payload when Run is pressed. The pure modules the dialog reads
   (`pickBranch`, `needsCutting`, `pauseFrom`, `pausePayload`, `notesPayload`) are the ones used here.

   Filled on opening (`openForm`), from `config`, `branches` and `remembered`:

   - the mode, the priority floor, the pause between batches, the live check and the filing of
     findings are what this project was left at last time (`settings.project.runSettings`, which
     the desktop keeps per project), else the dialog's own opening values;
   - the number of tasks at once is the project's `[defaults].max_parallel_tasks`, every time,
     never what was chosen last (the dialog's own rule);
   - the target branch is `pickBranch`: what the project was left at, its configured default,
     the first of the list;
   - the notes are empty.

   What is chosen on the phone is remembered nowhere: `settings_save` is not on the bridge's
   list, so the form is filled from the desktop's memory and never writes to it.

   The scope is the queue unless a card's sheet asked for a task or an epic (`runPayload`'s third
   argument), in which case the form is the same and only the scope differs. */
import { needsCutting, pickBranch } from '../../src/components/run/branchChoice.js'
import { pauseFrom, pausePayload, validatePause } from '../../src/components/run/batchPause.js'
import { DEFAULTS_FALLBACK } from '../../src/components/run/projectDefaults.js'
import { notesPayload } from '../../src/components/run/runNotes.js'

/* The dialog's own ceiling on tasks at once. */
export const PARALLEL_MAX = 8

const MODE_LABEL = { auto: 'Autopilot', supervised: 'Crew', solo: 'Solo' }
export const modeLabel = (mode) => MODE_LABEL[mode] ?? mode

/* Why a run cannot be started in this project, or '' when it can. Every state of the project's
   configuration is a state of its own (`runs/config.rs`): a project with no file has not been
   set up, one whose file will not parse has to be fixed first, and either is the desktop's to
   put right - the setup is an agent session writing a file. */
export function startBlock(config) {
  switch (config?.state) {
    case 'ok':
      return ''
    case 'missing':
      return 'This project is not set up for runs yet. Set it up on the desktop.'
    case 'broken':
      return config.message
        ? `The project's run configuration will not parse: ${config.message}`
        : "The project's run configuration will not parse."
    default:
      return 'The run configuration of this project has not been read.'
  }
}

/* Solo is a single task's mode and the scope here is the queue, so it is not offered. */
export const MODES = [
  { value: 'auto', label: 'Autopilot' },
  { value: 'supervised', label: 'Crew' }
]

export const PRIORITIES = [0, 1, 2, 3, 4].map((p) => ({ value: p, label: `P${p} and better` }))

export const PARALLEL = Array.from({ length: PARALLEL_MAX }, (_, i) => ({
  value: i + 1,
  label: i === 0 ? 'One at a time' : `${i + 1} at once`
}))

/* The desktop's own sentences, word for word. */
export const MODE_NOTE = {
  auto: 'Nothing is asked. Anything it cannot resolve is parked with a reason.',
  supervised: 'It takes a single batch, asks when something needs deciding, and stops once the batch is merged.'
}
export const PAUSE_NOTE = 'A random number of minutes in this range, both ends included. 0 and 0 means no pause.'
export const LIVE_CHECK_LABEL = 'Check each task for real before closing it'
export const LIVE_CHECK_NONE_NOTE = 'This project declares no way to check a merged task, so tasks close on a green merge.'
export const FINDINGS_LABEL = 'File what it finds along the way'
export const FINDINGS_NOTE = 'New tasks go to deferred and wait for you \u2014 a run never picks up its own findings.'
export const NOTES_PLACEHOLDER = 'For example: do not commit the result.'
export const NOTES_NOTE = 'Read once, at the start of this run. Not remembered.'
export const QUEUE_DESCRIPTION = 'Everything ready on the board.'

/* Whether the project declares any way to check a merged task. */
export const liveCheckAvailable = (config) => config?.live_check?.mode !== 'none'

/* The values the form's fields hold when it is opened. */
export function openForm({ config, branches, remembered }) {
  const kept = remembered ?? {}
  const defaults = config?.defaults ?? {}
  const branch = pickBranch(branches, kept.targetBranch, defaults.target_branch)
  /* The dialog turns a remembered Solo into Autopilot for anything but a task. */
  const mode = kept.mode && kept.mode !== 'solo' ? kept.mode : 'auto'
  const gap = pauseFrom(kept)
  return {
    branch,
    mode,
    parallel: Math.min(
      Math.max((defaults.max_parallel_tasks ?? DEFAULTS_FALLBACK.max_parallel_tasks) || 1, 1),
      PARALLEL_MAX
    ),
    pauseMin: gap.min,
    pauseMax: gap.max,
    priority: kept.minPriority ?? defaults.min_priority ?? DEFAULTS_FALLBACK.min_priority,
    /* The dialog also switches the live check off when the machine cannot drive a browser
       (`browser_tools`, which is not on the phone's list). Not knowing is the dialog's own
       "nobody has asked" state, and it leaves the switch where it was remembered. */
    liveCheck: liveCheckAvailable(config) && (kept.liveCheck ?? true),
    fileFindings: kept.fileFindings ?? true,
    notes: ''
  }
}

/* The pause fields exist in Autopilot alone: the other mode runs one batch. */
export const hasPause = (form) => form?.mode === 'auto'

/* A sentence per pause field that is wrong, `{}` otherwise and outside Autopilot. */
export const pauseErrors = (form) => (hasPause(form) ? validatePause(form.pauseMin, form.pauseMax) : {})

/* Whether Run may be pressed: a branch to run into and a pause that can be sent. */
export const canRun = (form) => Boolean(form?.branch) && Object.keys(pauseErrors(form)).length === 0

/* The payload of `run_start` for what the form holds, or `null` while Run may not be pressed.
   A `Select` hands back text, so the two numbers are made numbers here. `scope` is the queue
   unless a card asked for itself. */
export function runPayload(form, { config, branches }, scope = { kind: 'queue' }) {
  if (!canRun(form)) return null
  const pause = hasPause(form) ? pausePayload(form.pauseMin, form.pauseMax) : null
  return {
    scope,
    mode: form.mode,
    target_branch: form.branch,
    create_target: needsCutting(branches, form.branch),
    /* A floor belongs to the queue; the server refuses one on a named task or epic. */
    min_priority: scope.kind === 'queue' ? Number(form.priority) : null,
    max_parallel_tasks: Number(form.parallel),
    batch_pause_min: pause?.min ?? null,
    batch_pause_max: pause?.max ?? null,
    live_check: liveCheckAvailable(config) && Boolean(form.liveCheck),
    file_findings: Boolean(form.fileFindings),
    instructions: notesPayload(form.notes)
  }
}

/* The payload for a form left exactly as it opens. */
export const runSettings = ({ config, branches, remembered }) =>
  runPayload(openForm({ config, branches, remembered }), { config, branches })
