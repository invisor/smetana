/* Which verb a key press in the file tree means — and nothing at all about
   where the press came from or what is done with the answer.

   The `fileMenu.js` / `fileClipboard.js` / `renameName.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — no test in
   this repository can reach a `.vue`, so a rule left inside a component is a
   rule nothing checks. `FileTree.vue` is the thin half: it hands this function
   the event and turns the word that comes back into the same `action` the menu
   row of that name already emits, so the two gestures cannot mean different
   things.

   **`event.code` and never `event.key`**, which is the discipline `onSaveKey`,
   `onFindKey` and `onPaletteKey` in `DesktopApp.vue` already record and the one
   thing here that a reader is likely to get wrong: `event.key` is a Cyrillic
   character under a Russian layout and an upper-case letter under Caps Lock, and
   a shortcut written against it simply does not fire in either case. `code` is
   the key's place on the board, which is what somebody pressing ⌘C means.

   Alt and Shift are refused rather than ignored, the same way those three
   refuse them: ⌘⇧C and ⌘⌥C are other shortcuts on every platform, and a handler
   that answered to them would swallow keys it was never given. */
import { isMacUserAgent } from './fileMenu.js'

/* The four the clipboard group owns, by the key's place on the board. Rename is
   not in the table because it is the one verb with no chord: see below. */
const CHORD_VERBS = {
  KeyC: 'copy',
  KeyX: 'cut',
  KeyV: 'paste',
  KeyD: 'duplicate'
}

/**
 * The verb a press means, or `null` for every other key.
 *
 * `event` is read for five fields and is never touched otherwise, so a test
 * hands over a plain object and the component hands over the real thing.
 *
 * Rename is the one verb spelled differently per platform, and that is the
 * platform's rule rather than ours: F2 is rename in every file manager on
 * Windows and Linux and works on a Mac keyboard too, so it is taken everywhere;
 * Enter is rename in Finder and **open** in Explorer and in every Linux file
 * manager, so it is taken on macOS alone. Taking Enter everywhere would give
 * the same key two opposite meanings depending on which machine somebody sat
 * down at.
 *
 * The user agent is asked rather than `@tauri-apps/plugin-os`, for
 * `fileManagerName`'s reason: a command, a permission and an await for one
 * branch, answering nothing at all in `npm run dev`.
 */
export function fileTreeVerb(event = {}, { userAgent = '' } = {}) {
  const { code = '', metaKey = false, ctrlKey = false, altKey = false, shiftKey = false } = event
  if (altKey || shiftKey) return null
  if (metaKey || ctrlKey) return CHORD_VERBS[code] ?? null
  if (code === 'F2') return 'rename'
  if (code === 'Enter' && isMacUserAgent(userAgent)) return 'rename'
  return null
}
