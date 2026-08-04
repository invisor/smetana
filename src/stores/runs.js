/* Whether the active project is set up for runs, and with what. The seventh
   file in this directory that knows Tauri exists; components see a reactive
   object and two computeds.

   Deliberately small, like git.js: this is a file read, there is no worker
   behind it, and freshness comes from switching projects and from a setup
   session finishing. */
import { computed, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const NONE = { state: 'missing' }

export const runsState = reactive({
  project: null,
  /* The back end's own state object: missing | broken | ok. Kept as it
     arrived rather than unpacked into flags, so a state this front end has
     not heard of cannot silently read as one of the others. */
  config: NONE
})

/* An offer to set the project up, not a warning: most projects are here, and
   `broken` is deliberately excluded — a file that exists and cannot be parsed
   is something to fix, and running the setup over it would write across
   somebody's work. With no project open there is nothing to offer either:
   `config` still reads as `missing` (the same NONE a cleared project falls
   back to), so the project itself has to be part of the check. */
export const needsSetup = computed(() => runsState.project !== null && runsState.config.state === 'missing')

export const configError = computed(() =>
  runsState.config.state === 'broken' ? runsState.config.message : null
)

/* Guarded against its own stale response exactly as git.js and terminals.js
   are: two calls can be in flight with no ordering guarantee, and the last
   response winning over the last call would show one project's configuration
   under another project's name. */
export async function loadConfig(project) {
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
