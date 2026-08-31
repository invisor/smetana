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
   that clips rather than wraps.

   A path here may also be **absolute**, naming somewhere outside the project
   entirely, which is what a file copied in Finder ordinarily is — see
   `pasteSource` below. Nothing special is done for one and nothing needs to be:
   an absolute path is not a prefix of any folder in the tree, so it greys
   nothing, which is the direction these two copies of the rule are allowed to
   disagree in. Rust is what refuses the case this cannot see — a folder from
   outside that *holds* the project. */
export function canPasteInto({ clipboard = null, folder = '' } = {}) {
  const paths = clipboard?.paths ?? []
  if (paths.length === 0) return { ok: false, reason: 'empty' }
  const inside = paths.some((path) => folder === path || folder.startsWith(`${path}/`))
  if (inside) return { ok: false, reason: 'intoSelf' }
  return { ok: true, reason: null }
}

/* Which of the two clipboards a paste acts on, when they disagree.

   There are two, and they are not the same kind of thing. The tree's own
   record (`filesState.clipboard`) is the only one that knows an entry was
   **cut**, because on macOS there is no cut for files at all — Finder decides
   the move at paste time with Cmd+Opt+V and writes nothing to the pasteboard.
   The machine's holds whatever was last copied anywhere, which is the whole
   point of it.

   Three answers, and the middle one is the design. Copying inside the tree
   writes to the system clipboard as well, so at the moment of a paste the two
   normally name the same paths — and when they do, the internal record wins,
   because it is the one carrying the mode. When they disagree, something was
   copied somewhere else more recently, and that is what the person means to
   paste; it arrives as a copy unless the platform stated otherwise, which
   Windows and Linux can and macOS cannot.

   Both sides are absolute here and deliberately not in the tree's spelling:
   the system clipboard has no notion of a project, and a path from it may name
   somewhere else on the disk entirely, which is an ordinary paste and not an
   error. Turning the answer back into a tree path — or finding that it is not
   in this project — is the caller's, where the root is known. */
export function pasteSource({ internal = null, system = null } = {}) {
  const systemPaths = system?.paths ?? []
  if (systemPaths.length === 0) return internal ?? null
  const internalPaths = internal?.paths ?? []
  const same =
    internalPaths.length === systemPaths.length &&
    internalPaths.every((path, at) => path === systemPaths[at])
  if (same) return internal
  return { paths: [...systemPaths], mode: system.mode === 'cut' ? 'cut' : 'copy' }
}
