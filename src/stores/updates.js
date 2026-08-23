/* Where the update machine is, mirrored into the front end.

   A store for the reason `app.js` is one rather than for holding anything of
   its own: it is one of the only files in `src/` that knows Tauri exists, and
   the About tab is a component. There is no logic here — `src-tauri/src/
   updates.rs` owns the state machine whole, this reads the value it hands over
   and passes it on, and `components/settings/update.js` turns it into words.

   **Both windows keep one of these, and neither is the other's copy.** The
   settings window is a second webview with its own module graph, so it asks the
   same command and hears the same event on its own — which is the whole of why
   a window opened halfway through a download draws the download rather than an
   empty row. Nothing here is sent between the windows, and there is no
   equivalent of `settings.js`'s three-event contract to keep: Rust is the one
   writer and both windows are readers.

   The import of `notifications.js` goes one way and must stay that way. Runs do
   the same job through a cycle — `notifications.js` reads `runsState` and
   `runs.js` calls back into it — and that cycle is load-bearing enough to carry
   a warning in two files about what a module-scope `watch` would do to the
   built app. Nothing forces one here: the state travels *into* `syncUpdateCard`
   as an argument, so this module imports that one and that one knows nothing
   about this. Do not "improve" it into a store the bell reads. */
import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { syncUpdateCard } from './notifications.js'

/* `updates.rs`'s `EVENT`, and the same string on both sides. */
export const UPDATES_STATE = 'updates:state'

export const updatesState = reactive({
  /* Rust's tagged value whole, in its own shape, or `null` when there is
     nobody to ask — a browser, or a build whose answer this one cannot read.
     Kept whole for the reason `runs.js` keeps its `config` whole: a field this
     front end has not heard of must not silently read as one it has, and the
     rule that draws it is the one place the tag is interpreted. */
  state: null
})

/* Whether anything has been heard from the event yet. The subscription is made
   before the first read, so the two can land in either order, and an event that
   arrives first is the newer of the two — the read is answered from the same
   machine a moment later and would put the older picture back. The same
   reasoning `heard` carries in `SettingsWindow.vue`. */
let heard = false

const adopt = (state) => {
  updatesState.state =
    state && typeof state === 'object' && typeof state.kind === 'string' ? state : null
  /* The bell's half. Called on every state this store adopts, including the
     first read and including `null`, because the card has to go the moment the
     update stops being one — and "there is nobody to ask" is one of the ways
     that happens. */
  syncUpdateCard(updatesState.state)
}

/* Subscribe, then ask. Answers with the way to stop listening, the shape every
   subscription in `src/stores/` answers with.

   Neither half throws. In a browser `updates_state` is answered with `null` by
   `mockBackend.js` rather than refused, so `npm run dev` reaches this with no
   error at all; a real failure is logged at debug and leaves the state `null`,
   which draws the same nothing. An update is the one subject where saying
   nothing is always safe: the app goes on running the version it has. */
export async function initUpdates() {
  let stop = null
  try {
    stop = await listen(UPDATES_STATE, (event) => {
      heard = true
      adopt(event.payload)
    })
  } catch (err) {
    console.debug('[updates] nobody to hear from (a browser, there is no Tauri):', err)
  }
  try {
    const state = await invoke('updates_state')
    if (!heard) adopt(state)
  } catch (err) {
    console.debug('[updates] nobody to ask about updates (a browser, there is no Tauri):', err)
    if (!heard) adopt(null)
  }
  return stop ?? (() => {})
}

/* The press on About's check button. Never rejects — `updates_check` in Rust
   answers with the state that stopped it rather than failing, and a check that
   starts and then fails says so through the event. The answer is adopted
   because it is the state the machine is in *now*: a press that was refused
   because a download is already going draws that download immediately, with no
   wait for an event that is not coming. */
export async function checkForUpdate() {
  try {
    adopt(await invoke('updates_check'))
  } catch (err) {
    console.error('[updates] the check did not start:', err)
  }
}

/* The press on Install. It rejects with `UpdateError`'s `{kind, detail}` and
   this deliberately does not catch it: the refusal is the answer — the run
   gate, a development build, an install that would not go through — and the
   window turns it into a sentence with `installRefusal`. The same division
   `startRun` keeps with `runFailure` one file over.

   There is nothing to adopt on success: the app is on its way out. The state
   stays `ready` on a refusal, which is Rust's decision and the right one — what
   was downloaded is still downloaded and the press is still there to make
   again. */
export async function installUpdate() {
  await invoke('updates_install')
}
