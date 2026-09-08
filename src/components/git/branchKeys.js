/* Which verb a key press in the branch list means, and nothing at all about
   where the press came from or what is done with the answer.

   `files/fileTreeKeys.js`'s shape, deliberately and for its reason: no test in
   this repository can reach a `.vue`, so a rule left inside a component is a
   rule nothing checks — and a table of chords is exactly the half worth
   pinning. `BranchList.vue` is the thin side of it, turning the word that comes
   back into the gesture of that name the list already answers, so the keyboard
   and the pointer cannot come to mean two different things: `switch` is the
   double click's own function, `fold` and `unfold` are the heading's own click,
   and `menu` opens the panel the right click opens.

   **`event.code` and never `event.key`**, the discipline `fileTreeVerb`,
   `onSaveKey`, `onFindKey` and `onPaletteKey` already keep: `event.key` is a
   Cyrillic character under a Russian layout and an upper-case letter under Caps
   Lock, so a shortcut written against it does not fire in either case. `code`
   is the key's place on the board, which is what somebody pressing a key means.
   The arrows read the same either way; the rule is one rule so that the letter
   keys below cannot be the exception nobody noticed.

   Meta, Control and Alt are refused rather than ignored, for that same file's
   reason — ⌘← is "back" and ⌥← is "a word left" on every platform, and a
   handler answering to them would swallow keys it was never given. Shift is the
   one modifier with a verb behind it, and exactly one: `Shift+F10` is the
   platform's own "open the context menu here", which is why it is here at all
   rather than in the row's `contextmenu` handler. */

/**
 * The verb a press means, or `null` for every other key.
 *
 * `event` is read for five fields and touched no other way, so a test hands
 * over a plain object and the component hands over the real thing. The row is
 * described rather than passed: whether it is a folder and whether that folder
 * is open are the whole of what the arrows depend on, and a rule that took the
 * row itself would be a rule about `branchTree.js`'s shape.
 *
 * The two horizontal arrows are not each other's mirror, and that is the tree
 * pattern rather than an oversight. `ArrowLeft` closes an open folder and
 * otherwise goes **out** — to the heading the row sits under, which is what
 * makes a deep `fix/legacy/…` walkable without the pointer. `ArrowRight` opens
 * a closed folder and otherwise means nothing at all: a branch row has no
 * children to go into, and moving somewhere on a press whose meaning is "in"
 * would be inventing a direction this list does not have.
 */
export function branchVerb(event = {}, { folder = false, expanded = false } = {}) {
  const { code = '', metaKey = false, ctrlKey = false, altKey = false, shiftKey = false } = event
  if (metaKey || ctrlKey || altKey) return null
  /* Shift carries one press and refuses the rest, rather than being ignored:
     Shift+ArrowDown extends a selection in every list on every platform, and
     this list has no selection to extend. */
  if (shiftKey) return code === 'F10' ? 'menu' : null
  if (code === 'ArrowUp') return 'up'
  if (code === 'ArrowDown') return 'down'
  if (code === 'ArrowLeft') return folder && expanded ? 'fold' : 'parent'
  if (code === 'ArrowRight') return folder && !expanded ? 'unfold' : null
  if (code === 'Enter') return 'switch'
  /* The key a PC keyboard has and a Mac one does not, which is why `Shift+F10`
     above is the one that has to work everywhere. */
  if (code === 'ContextMenu') return 'menu'
  return null
}

/**
 * Whether a press is the find chord — ⌘F, or Ctrl+F on a machine without a
 * command key.
 *
 * It is here rather than in the component because of what reads it: the Git
 * panel takes this chord for its own filter field, and `onFindKey` in
 * `DesktopApp.vue` gives it up inside that panel and opens the command palette
 * everywhere else. Two halves of one decision, and a second spelling of the
 * chord in either of them would be a key that is intercepted on one side and
 * not answered on the other.
 *
 * Alt and Shift are refused for `branchVerb`'s reason: ⌘⇧F and ⌘⌥F are other
 * shortcuts, and this is not them.
 */
export function isFilterChord(event = {}) {
  const { code = '', metaKey = false, ctrlKey = false, altKey = false, shiftKey = false } = event
  if (code !== 'KeyF') return false
  if (altKey || shiftKey) return false
  return metaKey || ctrlKey
}
