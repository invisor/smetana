/* Which verb a key press on a row of the Changes section means, and nothing at
   all about where the press came from or what is done with the answer.

   `files/fileTreeKeys.js`'s shape and `git/branchKeys.js`'s, deliberately and
   for their reason: no test in this repository can reach a `.vue`, so a table
   of chords left inside a component is the half of a keyboard nothing checks.
   `ChangeList.vue` is the thin side of it, turning the word that comes back
   into the gesture of that name the list already answers — `open` is the click's
   own handler and `menu` opens the panel the right click opens, so the pointer
   and the keyboard cannot come to mean two different things.

   **`event.code` and never `event.key`**, the discipline `fileTreeVerb`,
   `branchVerb`, `onSaveKey`, `onFindKey` and `onPaletteKey` all keep:
   `event.key` is a Cyrillic character under a Russian layout and an upper-case
   letter under Caps Lock, so a shortcut written against it fires in neither
   case. `code` is the key's place on the board, which is what somebody
   pressing a key means.

   Meta, control and alt are refused rather than ignored, for that same file's
   reason — ⌘Enter, ⌃Enter and ⌥Enter each mean something somewhere, and a
   handler answering to them would swallow presses it was never given. Shift is
   the one modifier with a verb behind it, and exactly one: `Shift+F10` is the
   platform's own "open the context menu here", which is why it is here at all
   rather than in the row's `contextmenu` handler. Every other shifted press
   comes back `null` **without** the caller cancelling anything, which is what
   leaves Shift+Tab still walking out of the list.

   There are no arrow verbs, and their absence is a decision rather than a gap
   — see `ChangeList.vue`'s own header for what it costs and why it is where it
   is. A row that has nothing to open is the other half of the same restraint:
   Enter is refused there rather than reinterpreted. */

/**
 * The verb a press means, or `null` for every other key.
 *
 * `event` is read for five fields and touched no other way, so a test hands
 * over a plain object and the component hands over the real thing. The row is
 * described rather than passed: whether there is a file behind it is the whole
 * of what Enter depends on, and a rule that took the change itself would be a
 * rule about `vcs::model::Change`'s shape.
 *
 * `openable` is `changeMenu.js`'s own answer, asked by the caller through the
 * one predicate the click, the menu row and this key all share — a folder
 * record has nothing to diff, so Enter on one is `null` and not a press that
 * opens a tab over a name.
 */
export function changeVerb(event = {}, { openable = false } = {}) {
  const { code = '', metaKey = false, ctrlKey = false, altKey = false, shiftKey = false } = event
  if (metaKey || ctrlKey || altKey) return null
  /* Shift carries one press and refuses the rest, rather than being ignored:
     Shift+ArrowDown extends a selection in every list on every platform and
     this list has none, and Shift+Tab has to go on leaving. */
  if (shiftKey) return code === 'F10' ? 'menu' : null
  /* The key a PC keyboard has and a Mac one does not, which is why `Shift+F10`
     above is the one that has to work everywhere. */
  if (code === 'ContextMenu') return 'menu'
  if (code === 'Enter') return openable ? 'open' : null
  return null
}
