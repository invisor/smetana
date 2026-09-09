/* What a key press in a dialog window means, which today is one key and one
   question about it.

   Pure, with no Vue and no DOM in it, and a module of its own for the reason
   `git/changeKeys.js`, `git/branchKeys.js` and `files/fileTreeKeys.js` are one:
   no test in this repository can reach a `.vue`, so a rule left inside
   `DialogWindow.vue` is a rule nothing checks. That file is the thin side of
   it — it holds the one listener and turns the word that comes back into the
   very call the guest's own Cancel makes.

   **`event.key` and not `event.code`**, which is where this parts company with
   that family, deliberately. The discipline there is about letters: `event.key`
   is a Cyrillic character under a Russian layout and an upper-case one under
   Caps Lock, so a chord written against it fires in neither case. Escape
   produces no character on any layout, both fields answer `Escape` for it, and
   every other Escape in this tree — `ImageWindow.vue`, `MenuButton.vue`,
   `PointerMenu.vue`, `Dropdown.vue`, `CommandPalette.vue`, `BranchPicker.vue` —
   reads `key`. One vocabulary for one key beats a discipline that buys nothing
   here.

   Modifiers are refused rather than ignored, which is that family's rule kept:
   a handler answering to ⌘Esc or ⌥Esc would swallow a press the platform may
   have meant for itself, and none of the four has anything to do here.

   **A press something inside the dialog has already answered is not this
   window's**, and `defaultPrevented` is how that is known. Every panel that
   opens inside a dialog — the run dialog's `Dropdown`, the review window's
   `BranchPicker`, a `PointerMenu`, `BranchSelect`'s naming field — binds Escape
   on its own element and calls `preventDefault` without stopping propagation,
   so by the time the press reaches the document it is marked as answered and
   the panel has closed itself. One Escape undoes one thing, which is
   `MenuButton`'s own words about its submenu, and this is that rule one level
   out: the branch list closes and the window it is drawn in stays.

   **Three answers and not two.** `refuse` is a press that was this window's and
   is not being acted on, because the dialog has taken its way out away for as
   long as it is writing — `discard-change` while the discard is in flight, and
   every other guest while its own `busy` holds. Escape must not become the way
   around a refusal Cancel and the cross already make, and it is `refuse` rather
   than `null` so that the caller can still cancel the press: a key this window
   has decided about is not one anything else in the page should also act on. */

/**
 * What a key press means to a dialog window: `close`, `refuse`, or `null` for
 * every press that is not this window's.
 *
 * `event` is read for six fields and touched no other way, so a test hands over
 * a plain object and the component hands over the real thing. `closable` is the
 * dialog's own answer, which reaches the caller from `overlays/Modal.vue` — the
 * one component every guest draws — and defaults to a way out, since a window
 * nobody can close is the worse of the two failures.
 */
export function dialogVerb(event = {}, { closable = true } = {}) {
  const {
    key = '',
    defaultPrevented = false,
    metaKey = false,
    ctrlKey = false,
    altKey = false,
    shiftKey = false
  } = event
  if (key !== 'Escape') return null
  if (metaKey || ctrlKey || altKey || shiftKey) return null
  if (defaultPrevented) return null
  return closable ? 'close' : 'refuse'
}
