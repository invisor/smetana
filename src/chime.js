/* Playing one of the four notification sounds. The half of this feature that
   touches the DOM, kept apart from `sounds.js` so the rules stay reachable by a
   test — the same split `appearance.js` and `views/useAppearance.js` make.

   **It plays through Web Audio, and never through `new Audio`. That is the
   whole reason this file looks the way it does, and undoing it is believed to
   bring back a macOS permission dialog.** The account: an `HTMLMediaElement` in
   WKWebView plays through AVFoundation, which registers a Now Playing session;
   for an unsandboxed app macOS bills that as a reach for the person's media
   library and raises the dialog somebody actually saw — "smetana" would like to
   access Apple Music, your music and video activity, and your media library —
   with no explaining sentence under it, since the bundle declares no
   `NSAppleMusicUsageDescription` and deliberately never will (smetana-i4w). It
   arrives at whatever moment a run ended or an agent asked a question, which is
   to say in the middle of reading somebody else's output, and there is no true
   answer to it: this app has nothing to do in anybody's music library.
   Declaring the key would only make an unwarranted request look warranted. An
   `<audio>` element, a `new Audio(...)`, and `HTMLMediaElement` under any other
   name are all the same trigger. Web Audio is not one of them, and that half is
   a fact about the API rather than a belief: an `AudioContext` is no
   `HTMLMediaElement` and registers nothing with the Now Playing info centre.
   What is unconfirmed is the other half — that the media element path is what
   raised the dialog.

   **That account was never confirmed, and this is the sentence that says so.**
   The TCC log the task asked for (`log stream --predicate 'subsystem ==
   "com.apple.TCC"' --info`, after `tccutil reset MediaLibrary
   com.invisor.smetana`) was not observed; what is implemented here is the
   hypothesis of smetana-i4w taken as the diagnosis, because the treatment is
   cheap and the file is better this way regardless.

   So if the dialog survives this change, there are two worlds and they want
   opposite things. Either Web Audio turns out to be the same trigger, which is
   the one case the task kept a reserve for: playing the sound from Rust
   (`NSSound`), which costs IPC per noise and a platform fork where there is now
   one file for every platform, and was therefore a second move rather than the
   first. Or the fault is not the sound at all — a native panel, a PTY child,
   the trash — which is the audit of the app's other permission requests that
   smetana-i4w deliberately left out of scope, and in that world moving the
   sound into Rust buys the fork and changes nothing whatever. The TCC log is
   what tells the two apart, and it is the first move either way: run it before
   concluding this file is innocent, and before concluding it is guilty.

   What Web Audio costs here, and it is the shape of the rest of the file. An
   `AudioBufferSourceNode` is single-use — it cannot be rewound and played
   again — so "one player per sound" becomes **one decoded buffer per sound**,
   made on first use and kept, with a fresh node per playback. The old
   `currentTime = 0` rewind simply has nothing to correspond to: a new node is
   a new playback, so the same sound twice within a second is heard twice by
   construction rather than by remembering to rewind.

   The failure path is kept: a rejection is warned about and otherwise
   swallowed. A system webview may refuse audio no gesture asked for — with Web
   Audio the refusal takes the shape of a context stuck in `suspended` rather
   than of a rejected `play()` — and whatever WKWebView and WebKitGTK decide, an
   app that throws because a sound would not play is worse than a quiet one.
   This is also the one part of the feature a browser cannot verify:
   `npm run dev` is Chrome-shaped and proves nothing about the webview the app
   actually runs in. */
import { shouldPlay } from './sounds.js'

/* Vite resolves each of these to a URL and emits the file into the bundle. The
   map is keyed by the ids in `sounds.js`, so a sound added there without a file
   here is caught by the guard below rather than by a failed fetch. */
import sound1 from './assets/sounds/sound-1.mp3'
import sound2 from './assets/sounds/sound-2.mp3'
import sound3 from './assets/sounds/sound-3.mp3'
import sound4 from './assets/sounds/sound-4.mp3'

const FILES = {
  'sound-1': sound1,
  'sound-2': sound2,
  'sound-3': sound3,
  'sound-4': sound4
}

/* One context for the whole window, made on first noise rather than at import:
   a context created while the module graph evaluates is one more thing a webview
   may hold open for a window that never makes a sound at all. */
let context = null

/* Id to a promise of its decoded buffer — the lazy-and-cached shape the file had
   before, one step later in the pipeline. The promise rather than the buffer is
   what is stored, so two events arriving while the first decode is still in
   flight share it instead of fetching twice. */
const buffers = new Map()

/* How long to wait for a suspended context to start before giving up on this
   one sound. See `ring` for why the wait is bounded at all; the number itself is
   loose on purpose — it is long enough that no machine loses a sound to being
   busy, and short enough that what comes out of it is still an answer about the
   event that asked. A notification later than this is about a moment that has
   passed anyway. */
const RESUME_WAIT_MS = 1000

function audioContext() {
  if (context) return context
  const Context = typeof window === 'undefined' ? undefined : window.AudioContext
  if (!Context) throw new Error('this webview has no Web Audio')
  context = new Context()
  return context
}

function bufferFor(id, file, ctx) {
  const held = buffers.get(id)
  if (held) return held
  /* `decodeAudioData` detaches the array buffer it is given, which is harmless
     here: every fetch produces its own, and only the decoded result is kept. */
  const pending = fetch(file)
    .then((response) => {
      if (!response.ok) throw new Error(`${file} answered ${response.status}`)
      return response.arrayBuffer()
    })
    .then((bytes) => ctx.decodeAudioData(bytes))
  /* A failure must not become the cached answer for the rest of the session:
     forget it, so the next event tries again. The caller still sees this
     rejection — this handler only unlatches the cache. */
  pending.catch(() => {
    if (buffers.get(id) === pending) buffers.delete(id)
  })
  buffers.set(id, pending)
  return pending
}

async function ring(id, file) {
  const ctx = audioContext()
  /* A system webview may hand back a context in `suspended` when no gesture has
     been made yet, and every call site here is an event rather than a press, so
     resuming is what turns that into a sound at all.

     **The wait is bounded, and that is load-bearing rather than defensive.**
     `resume()` on a context a webview will not yet let start need not reject at
     all: the spec has it held until the context is allowed to run, which is what
     a Chromium-shaped webview does — WebView2, and `npm run dev`. It is the held
     case this race is about; WebKit has historically answered a blocked resume
     with a rejected `NotAllowedError` instead, and that one the outer catch in
     `chime` already takes. Awaiting the held case flatly parks one whole `ring`
     call per event behind it, and the gesture that finally arrives releases
     every one of them into the same turn: a night of notifications heard as a
     single burst. Racing a timer is what lets the wait end without an answer,
     and the `running` check below is then reachable and turns it into one
     console line and no sound. Do not simplify this to a bare
     `await ctx.resume()`, and do not delete the check under it as unreachable —
     between them they are the whole of what keeps the pile-up from being
     audible. */
  if (ctx.state === 'suspended') {
    await Promise.race([
      ctx.resume(),
      new Promise((settle) => setTimeout(settle, RESUME_WAIT_MS))
    ])
  }
  const buffer = await bufferFor(id, file, ctx)
  if (ctx.state !== 'running') {
    console.warn('[chime] the sound would not play: the audio context is', ctx.state)
    return
  }
  const source = ctx.createBufferSource()
  source.buffer = buffer
  source.connect(ctx.destination)
  source.start()
}

/* Whether the person is looking at this document right now. This is the whole
   of what `notifications.onlyWhenUnfocused` means, and the question is
   deliberately synchronous and asked at the moment of the noise: no listener,
   no stored flag, no new event across IPC. Both call sites live in the main
   window's stores, so "this document" is the main window and nothing else —
   the settings window is a second `WebviewWindow` with a document of its own.
   The consequence is named rather than discovered: somebody working in an open
   settings window with the app behind it still hears the sound. Reading the
   focus of both windows would need the main one to be told about the other's,
   which is a channel of state bought for a word.

   A document with no `hasFocus` at all — a webview stranger than any this ships
   in, or a test stub — answers "not focused", so the sound plays. The option
   can then only fail towards what the app did before it existed. */
const documentFocused = () =>
  typeof document !== 'undefined' && typeof document.hasFocus === 'function'
    ? document.hasFocus()
    : false

export function chime(id, { unlessFocused = false } = {}) {
  /* Asked only when the answer can matter, which is also what keeps the preview
     on the settings window free of it: `pick` there calls this with no options
     at all, so nothing is asked of any document and the chosen sound plays.
     `shouldPlay` reads this third argument only under the second, so the
     short-circuit changes no answer it could give — only whether a document was
     asked a question nothing would have done with. */
  const focused = unlessFocused ? documentFocused() : false
  if (!shouldPlay(id, unlessFocused, focused)) return
  const file = FILES[id]
  if (!file) return
  /* Fire and forget: making a noise is asynchronous now — a fetch and a decode
     stand between the call and the sound — while every caller is a store
     reacting to an event that matters more than the noise, and none of them has
     anything to do with the answer. A webview with no Web Audio at all, a file
     that would not decode, a context that refuses to resume: all of it lands
     here, is written to the console, and takes nobody down. One failure can
     print more than once — two events waiting on the same failed decode both
     see it — which is the ordinary cost of not letting either of them throw. */
  ring(id, file).catch((err) => {
    console.warn('[chime] no sound:', err)
  })
}
