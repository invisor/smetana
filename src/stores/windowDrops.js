/* One subscription to the window's drag-drop event, shared by every pane that
   wants a file dropped on it. One of the files in `src/` that know Tauri
   exists — see the list in CLAUDE.md rather than a number written here, since
   an ordinal is written once and the list keeps growing under it.

   Tauri intercepts a file drop before any webview sees it — `dragDropEnabled`
   is on by default and this app leaves it on — so there is no `dragover` and no
   `drop` for a component to listen for: the gesture arrives as an event
   against the *window*, carrying absolute paths, and only a store may hear it
   (only a store may import Tauri). Before this file existed that subscription
   was written out twice, once in `terminals.js` for the terminal pane and once
   in `attachments.js` for the new-task dialog, agreeing on everything down to
   the browser case's log line because one was written by copying the other.
   `smetana-h8vq` is what a third copy, for the conversation panel, would have
   made three — so the lifecycle moved here instead, and the two older stores
   now call it rather than repeat it. `.claude/rules/terminal.md` and
   `.claude/rules/attachments.md` carry the fuller history.

   **This module is a subscription, not an arbiter.** It has no opinion about
   whose drop a point belongs to, and it is not supposed to gain one:
   `smetana-3j8` rejected a shared dispatcher for exactly this event, and nothing
   here reopens that. Two consumers on one window's drops are kept apart by
   never both being mounted over the same point at once — a hit test on the
   asker's own side, as `TerminalView.vue`'s `insideHost` and
   `ConversationView.vue`'s own do, or a webview that draws only one thing, as
   the new-task dialog's does — never by asking here first.

   What this hands over is a point in CSS pixels from the top left of the
   viewport — exactly the space `document.elementFromPoint` reads — and the
   paths, on whichever of `over`, `leave` or `drop` the event answers to.
   `paths` rides along with `over` only on the enter event, the one that
   carries them; the events in the middle of a drag carry `null`.

   Tauri types the event's position `PhysicalPosition` on every platform, and on
   two of the three that is not what is in it — `dropPoint.js` carries the whole
   of that argument and does the arithmetic; this file only asks the back end
   which of the two arrived, once per window, since it is a fact about the
   build and cannot change while the app is running. Asking here rather than
   inside the event handler is also what lets the subscription wait for the
   answer before it starts listening, rather than converting a point whose
   units it does not know.

   In a browser there is no webview to ask, and `getCurrentWebview` throws
   before the subscription — an ordinary mode rather than a failure, logged
   once at `console.debug` and answered with a function that unsubscribes
   nothing, so a caller can treat every environment the same way. */
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { dropSpaceFromPlatform, viewportPoint } from '../dropPoint.js'

let askedDropSpace = null
function dropSpace() {
  if (!askedDropSpace) {
    askedDropSpace = invoke('drag_drop_space')
      .catch((err) => {
        console.debug('[windowDrops] nothing to ask which units a drop arrives in:', err)
        return null
      })
      .then(dropSpaceFromPlatform)
  }
  return askedDropSpace
}

/**
 * Subscribe to the window's drag-drop event.
 *
 * `over` is called on both `enter` and `over`, `drop` on `drop`, and `leave`
 * on that event and on anything else a future Tauri version adds beside it —
 * forgetting the response is the safe reading of an event this code does not
 * know. Each of the three is optional; a caller that only wants the drop
 * itself passes only `drop`.
 *
 * Returns an unsubscribe function that is always safe to call, including
 * before the subscription has actually landed: `onDragDropEvent` is a promise
 * a few `listen` calls deep, and a view can unmount while it is still in
 * flight. The `stopped` flag is what keeps that case from leaking a listener
 * nobody can reach any more.
 */
export function watchWindowDrops({ over, leave, drop } = {}) {
  let webview
  try {
    webview = getCurrentWebview()
  } catch {
    console.debug('[windowDrops] no webview: drops are a Tauri-only gesture')
    return () => {}
  }
  let stop = null
  let stopped = false
  dropSpace()
    .then((space) =>
      webview.onDragDropEvent(({ payload }) => {
        if (payload.type !== 'enter' && payload.type !== 'over' && payload.type !== 'drop') {
          leave?.()
          return
        }
        const { x, y } = viewportPoint(payload.position, space, window.devicePixelRatio)
        const at = { x, y, paths: payload.paths ?? null }
        if (payload.type === 'drop') drop?.(at)
        else over?.(at)
      })
    )
    .then((unlisten) => {
      stop = unlisten
      /* The caller unmounted while the subscription was still on its way. */
      if (stopped) stop()
    })
    .catch((err) => console.error('[windowDrops] listening for drops failed:', err))

  return () => {
    stopped = true
    if (stop) stop()
  }
}
