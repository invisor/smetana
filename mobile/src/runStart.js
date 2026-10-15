/* The settings a run is started with from the phone, and whether one may be started at all.

   The phone does not offer the desktop's run window. It starts the run that window opens on,
   so the only choices left are the ones the person makes on a phone: the target branch and the
   press. Every other field is what `RunModal.vue` fills in when it opens on "Run the queue",
   from the same sources and by the same rules (the pure modules it reads - `pickBranch`,
   `needsCutting`, `pauseFrom`, `pausePayload` - are the ones used here):

   - the mode, the priority floor, the pause between batches, the live check and the filing of
     findings are what this project was left at last time (`settings.project.runSettings`,
     which the desktop keeps per project), else the dialog's own opening values;
   - the number of tasks at once is the project's `[defaults].max_parallel_tasks`, every time,
     never what was chosen last (the dialog's own rule);
   - the target branch is `pickBranch`: what the project was left at, its configured default,
     the first of the list.

   The scope is the queue. A task or an epic is started from the desktop's board, where the
   card is. */
import { needsCutting, pickBranch } from '../../src/components/run/branchChoice.js'
import { pauseFrom, pausePayload } from '../../src/components/run/batchPause.js'
import { DEFAULTS_FALLBACK } from '../../src/components/run/projectDefaults.js'

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

/* The payload of `run_start`, or `null` while there is no branch to run into. `branch` is the
   person's own choice, `''` or absent when they have made none. */
export function runSettings({ config, branches, remembered, branch = '' }) {
  const kept = remembered ?? {}
  const defaults = config?.defaults ?? {}
  const target =
    branch && (branches ?? []).some((b) => b?.name === branch)
      ? branch
      : pickBranch(branches, kept.targetBranch, defaults.target_branch)
  if (!target) return null

  /* Queue scope: the dialog turns a remembered Solo into Autopilot for anything but a task. */
  const mode = kept.mode && kept.mode !== 'solo' ? kept.mode : 'auto'
  const pause = mode === 'auto' ? pausePayload(...Object.values(pauseFrom(kept))) : null
  const parallel = Math.min(
    Math.max((defaults.max_parallel_tasks ?? DEFAULTS_FALLBACK.max_parallel_tasks) || 1, 1),
    PARALLEL_MAX
  )
  return {
    scope: { kind: 'queue' },
    mode,
    target_branch: target,
    create_target: needsCutting(branches, target),
    min_priority: kept.minPriority ?? defaults.min_priority ?? DEFAULTS_FALLBACK.min_priority,
    max_parallel_tasks: parallel,
    batch_pause_min: pause?.min ?? null,
    batch_pause_max: pause?.max ?? null,
    /* The dialog switches the live check off when the machine cannot drive a browser
       (`browser_tools`, which is not on the phone's list). Not knowing is the dialog's own
       "nobody has asked" state, and it leaves the switch where it was remembered. */
    live_check: config?.live_check?.mode !== 'none' && (kept.liveCheck ?? true),
    file_findings: kept.fileFindings ?? true,
    instructions: null
  }
}

/* The sentence the confirmation says, so that the press is made knowing what it starts. */
export function describeStart(settings, ready) {
  const parts = [`${modeLabel(settings.mode)} into ${settings.target_branch}`]
  if (settings.mode !== 'solo' && settings.max_parallel_tasks) {
    parts.push(
      settings.max_parallel_tasks === 1
        ? 'one task at a time'
        : `${settings.max_parallel_tasks} tasks at once`
    )
  }
  if (typeof ready === 'number') parts.push(ready === 1 ? 'one task is ready' : `${ready} tasks are ready`)
  return parts.join(', ')
}
