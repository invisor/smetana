/* Playing one of the four notification sounds. The half of this feature that
   touches the DOM, kept apart from `sounds.js` so the rules stay reachable by a
   test — the same split `appearance.js` and `views/useAppearance.js` make.

   One `Audio` per sound, made on first use and kept: a new element per event
   would re-fetch nothing (the file is in the bundle) but would leave a growing
   pile of media elements behind over a night of a hundred agent questions.

   `play()` returns a promise that may reject, and the rejection is warned about
   and otherwise swallowed. A system webview may refuse audio that no gesture
   asked for, and whatever WKWebView and WebKitGTK decide, an app that throws
   because a sound would not play is worse than a quiet one. This is also the one
   part of the feature a browser cannot verify: `npm run dev` is Chrome-shaped
   and proves nothing about the webview the app actually runs in. */
import { shouldPlay } from './sounds.js'

/* Vite resolves each of these to a URL and emits the file into the bundle. The
   map is keyed by the ids in `sounds.js`, so a sound added there without a file
   here is caught by the guard below rather than by a broken `Audio`. */
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

const players = new Map()

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
  try {
    let player = players.get(id)
    if (!player) {
      player = new Audio(file)
      players.set(id, player)
    }
    /* The same sound twice in a row — two agents asking within a second — must
       be heard twice, and an element already playing ignores a second `play()`.
       Rewinding first is what makes the second one audible. */
    player.currentTime = 0
    const started = player.play()
    if (started && typeof started.catch === 'function') {
      started.catch((err) => {
        console.warn('[chime] the sound would not play:', err)
      })
    }
  } catch (err) {
    /* A webview with no media support at all, or a test environment whose
       `Audio` is a stub. Neither is a reason to take a caller down: every call
       site here is a store reacting to an event that matters more than the
       noise. */
    console.warn('[chime] no sound:', err)
  }
}
