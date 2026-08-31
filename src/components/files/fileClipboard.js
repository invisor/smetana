/* What can be pasted where.

   The `fileMenu.js` / `newEntry.js` family: pure, no Vue and no DOM, which is
   the whole reason it is a file of its own — no test in this repository can
   reach a `.vue`, so a rule left inside a component is a rule nothing checks.

   The one question here is the one the back end also asks and answers with
   `intoSelf` (`refuse_into_self` in `files/fs.rs`). Asking it twice is
   deliberate rather than a duplication: this copy is what greys the menu row
   *before* anything is attempted, so the refusal is a label somebody reads
   instead of a toast after a click; the Rust one is what makes the refusal true
   when a path holds a symlink, which these strings cannot see at all. The two
   are allowed to disagree in exactly that direction — this one may say yes
   where Rust says no, never the other way round. */

/* Whether a paste into `folder` is offered at all, and why not when it is not.

   `folder` is a path relative to the project root, `''` being the root itself —
   the tree's own spelling, and `files_list`'s. A clipboard path is compared
   against it as a prefix ending in a separator rather than by `startsWith`
   alone: `src/ab` starts with `src/a` and is a sibling, not a descendant, and
   greying its row would refuse a paste that is perfectly ordinary.

   The reason travels as a machine-readable string and never as a sentence: the
   words are `fileMenu.js`'s, which is the file that has to fit them into a row
   that clips rather than wraps. */
export function canPasteInto({ clipboard = null, folder = '' } = {}) {
  const paths = clipboard?.paths ?? []
  if (paths.length === 0) return { ok: false, reason: 'empty' }
  const inside = paths.some((path) => folder === path || folder.startsWith(`${path}/`))
  if (inside) return { ok: false, reason: 'intoSelf' }
  return { ok: true, reason: null }
}
