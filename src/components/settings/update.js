/* What the About tab says about updating, and which control it offers.

   Another of the `branchChoice.js` family, and the sibling of `storage.js` one
   file over: the whole of one rule, pure, with no Vue and no DOM in it, living
   under the part of the interface it is a rule about. The reason is the one
   that family always gives — a `.vue` file is the one thing no test in this
   repository can reach, so a sentence a person reads before pressing a button
   that restarts their app is worth pinning outside the component that draws it.

   **The state machine is not here.** It is `src-tauri/src/updates.rs`, whole,
   and this file only reads the tagged value that machine hands over. Nothing
   below decides a transition, remembers a state or infers one from another:
   given the same value it answers the same words, and every state it can be
   asked about is one Rust put on the wire.

   The seventh kind is this file's own, and it is the honest name for a shape
   that never came from Rust: `unavailable`. A browser has no back end to ask
   (`stores/updates.js` answers `null`), and a build newer than this one may
   have a `kind` this front end has never heard of. Both are drawn as nothing at
   all rather than as `idle` — an "up to date" said by a window that never asked
   anybody would be the app claiming to know something it does not. */
import { formatBytes } from './storage.js'

/* Nowhere to ask, or an answer in a shape this build does not know. */
export const UNAVAILABLE = 'unavailable'

/* `UpdateState`'s six tags, snake_case as serde writes them. A list rather than
   a set of `if`s because the whole point of a tag is that an unknown one
   matches nothing — see the header. */
const KINDS = ['idle', 'checking', 'available', 'downloading', 'ready', 'failed']

/* Which of the seven this is. Everything below goes through it, so there is one
   place that decides what an unheard-of answer means. */
export function updateKind(state) {
  return KINDS.includes(state?.kind) ? state.kind : UNAVAILABLE
}

/* Whether About draws anything about updates at all. False in a browser, which
   is the same answer `appVersion()` gives by returning `null` and the tab draws
   as a dash: a screen with nobody behind it says nothing rather than offering a
   button that cannot act. */
export function updatesKnown(state) {
  return updateKind(state) !== UNAVAILABLE
}

/* The version waiting to be installed, or `null` when nothing is. The bell's
   card is built on this (`components/notifications/notifications.js`), so what
   counts as "an update is ready" is decided once and in one place.

   `Machine::ready` fills the version from what the check found and falls back to
   an empty string if it somehow has none, so an empty one is treated as absent
   here — a sentence naming version "" would be worse than one naming none. */
export function readyVersion(state) {
  if (updateKind(state) !== 'ready') return null
  return typeof state.version === 'string' && state.version ? state.version : null
}

/* A count of bytes that arrived in a shape worth drawing. Missing is not zero,
   the distinction `projectBytes` draws next door — but a download that has not
   reported a chunk yet genuinely has received nothing, so the absence is read
   as 0 here and only for `received`. `total` keeps the other reading: `None`
   until the server says how long the body is, and a bar with no end is a truer
   drawing than one invented (`updates.rs`). */
const counted = (value) =>
  typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null

/* How far a download has got, as a whole percent, or `null` when nothing can
   say. Rounded down, so it never says 100% while bytes are still coming. */
function percent(received, total) {
  if (total === null || total <= 0) return null
  return Math.min(100, Math.floor((received / total) * 100))
}

/* The line under the label: what is happening, in one sentence.

   Sentence case throughout, and the version is named wherever there is one —
   "an update" is what the app says when it genuinely does not know which. */
export function updateLine(state) {
  const kind = updateKind(state)
  if (kind === 'idle') {
    return 'No update is waiting. Smetana looks for one about once a day and downloads it quietly.'
  }
  if (kind === 'checking') return 'Looking for a new version…'
  if (kind === 'available') {
    const version = typeof state.version === 'string' && state.version ? state.version : null
    return version
      ? `Smetana ${version} was found; the download is starting.`
      : 'A new version was found; the download is starting.'
  }
  if (kind === 'downloading') {
    const received = counted(state.received) ?? 0
    const total = counted(state.total)
    const share = percent(received, total)
    if (total === null) return `Downloading — ${formatBytes(received)} so far.`
    return `Downloading — ${formatBytes(received)} of ${formatBytes(total)}${share === null ? '' : ` (${share}%)`}.`
  }
  if (kind === 'ready') {
    const version = readyVersion(state)
    return version
      ? `Smetana ${version} is downloaded and ready. Installing restarts the app.`
      : 'A new version is downloaded and ready. Installing restarts the app.'
  }
  if (kind === 'failed') {
    /* Rust's own words, which are already written for a person and already
       sentence case (`because` frames them that way). A message of our own here
       would throw away the only part of this that says what actually went
       wrong. */
    const message = typeof state.message === 'string' ? state.message.trim() : ''
    return message || 'The last check did not finish.'
  }
  return ''
}

/* The one control the row offers, or `null` for the states that have nothing to
   press. `verb` is the event the component emits, so the label — which is prose
   and will change — is never what the acting code switches on.

   Two of the six offer nothing, and both are flows already in hand: `available`
   lasts for the two statements between finding a release and asking for its
   first byte, and `downloading` finishes by itself. `checking` offers the
   button disabled rather than nothing at all, so the row does not lose a
   control for the length of a network round trip and then grow one back.

   `ready` offers Install unconditionally, and that is deliberate: a run going
   somewhere else is the one thing that refuses it, the refusal is Rust's to
   make and it arrives carrying the projects it named. A control drawn dead here
   on a guess would be inert with nothing said — and it would be wrong, since
   this window cannot see a run in a project nobody is looking at. */
export function updateAction(state) {
  const kind = updateKind(state)
  if (kind === 'idle' || kind === 'failed') {
    return { verb: 'check', label: 'Check for updates', disabled: false }
  }
  if (kind === 'checking') return { verb: 'check', label: 'Checking…', disabled: true }
  if (kind === 'ready') return { verb: 'install', label: 'Install and restart', disabled: false }
  return null
}

/* Why an install did not happen, in words, from `UpdateError`'s `{kind, detail}`
   — the same shape `runFailure` in `DesktopApp.vue` reads and for the same
   reason: a refusal that cannot say what is in the way sends somebody to guess.

   The two that carry somebody else's message (`runs`, `install`) are passed
   through as they are, which is what `runFailure` does with a broken config: the
   borrowed half is the only part that says what actually went wrong. The two
   this app writes itself are rewritten in this app's voice rather than quoted,
   since the `thiserror` text is a fragment built for a log line and never
   travels on the wire anyway — `#[serde(tag, content)]` carries the tag and the
   fields, and nothing of `#[error]`. */
export function installRefusal(err) {
  if (!err) return null
  /* A channel that broke rather than a refusal that was made: `invoke` rejects
     with whatever it was given, and not every failure on the way to Rust is one
     of the five below. */
  if (typeof err === 'string') return err
  const detail = err.detail
  if (err.kind === 'run_live') {
    const projects = typeof detail?.projects === 'string' && detail.projects ? detail.projects : null
    return projects
      ? `A run is going in ${projects}. Installing restarts the app, which would end it.`
      : 'A run is going. Installing restarts the app, which would end it.'
  }
  if (err.kind === 'nothing_ready') return 'There is no downloaded update to install.'
  if (err.kind === 'development_build') return 'A development build does not replace itself.'
  if (typeof detail === 'string' && detail) return detail
  if (typeof err.message === 'string' && err.message) return err.message
  return 'The update could not be installed.'
}
