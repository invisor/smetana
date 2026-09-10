/* The rules about a path that belong to no one part of this front end: what a
   path is called, what folder it sits in, what it is called from inside a
   folder, and whether a row in the tree is a path at all.

   What they have in common is the test for belonging here, and it is the test to
   apply before adding another: each is pure — no Vue, no Tauri, no DOM — and
   each is wanted by more than one part of the interface at once, so there is no
   "under" to file it beneath. A rule with a single consumer belongs next to that
   consumer instead.

   `basename` was three copies before, and they disagreed where it showed: the
   one in stores/projects.js split on both separators, `basenameOf` in
   stores/files.js split on `/` alone, and a third in the run dialog's tooltip
   answered '' for a root path where the other two answer the path itself. All
   three name a folder or a file on screen, so the disagreement was visible — a
   project at a root path rendered as an empty gap in a sentence.

   Not in a store, then, and not next to any one consumer: they are wanted by two
   stores, by a pure component module of the branchChoice.js family, and by a
   `.vue` file — and a component importing a store to borrow one regex would have
   taken Vue's one import rule with it. */

/* The path separator differs per system, and WebView2 is among the target
   webviews: we split on both, otherwise on Windows the whole path would become
   the project's name.

   Splitting on `\` costs one exotic case on Unix, where a backslash is a legal
   character in a filename: `a\b.txt` is named `b.txt` here. That is the cheaper
   half of the trade — it misnames a file almost nobody has, in a modal's
   sentence, against misnaming every project on Windows.

   `filter(Boolean)` is what makes a trailing separator harmless; `?? path` is
   for the string that has nothing left after it — a bare `/` is called `/`,
   because a name is more use than an empty gap in a sentence. */
export const basename = (path) => path.split(/[/\\]/).filter(Boolean).pop() ?? path

/* What folder a path sits in, and `null` when it names no folder above it at
   all — a bare name with no separator in it, or a root.

   Both callers are a system file dialog's starting directory, in two stores at
   once, which is what puts it here rather than beside either of them:
   `defaultPath` overrides the panel's own memory of where it was last opened,
   so a store that wants the panel to open where the last choice was made has to
   work that folder out from the path the panel handed back.

   `null` rather than a guess, for the reason `relativeTo` answers `null`: there
   is no folder above a bare name, and the caller opens its dialog with no
   `defaultPath` at all — which is the behaviour the panel already had.

   Both separators again. A trailing one is dropped first so `/a/b` and `/a/b/`
   answer alike, and a head left holding nothing but a root keeps its separator:
   `''` is not the root and `C:` on its own is not a folder, so `/a` answers `/`
   and `C:\a` answers `C:\`. */
export function dirname(path) {
  if (!path) return null
  const trimmed = path.replace(/[/\\]+$/, '')
  const cut = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'))
  if (cut < 0) return null
  const head = trimmed.slice(0, cut)
  return head === '' || head.endsWith(':') ? head + trimmed[cut] : head
}

/* Every folder a path sits under, from the root down: `a/b/c.txt` answers
   `['a', 'a/b']`, and a bare name with no folder above it answers `[]`.

   Here rather than beside either of its callers for this file's own test: the
   tree's reveal wants it in `DesktopApp.vue`, where the stores live, and the
   rule itself is pure string work that a `.vue` file is the one thing no runner
   here can reach. `dirname` directly above answers the same question one step
   at a time and is deliberately not what this is built on — that one takes both
   separators and keeps a root's own, which is a different path space from this
   one's.

   **The closer neighbour is `parentOf` in `components/files/fileMenu.js`** — one
   separator, the tree's own space, this very question one step at a time — and
   naming it is the point, because it is the unification somebody will reach for
   later and the answer is not in either body. The reason that settles it is the
   layering: this file cannot import from `components/`, which imports from here —
   `fileMenu.js` re-exports `absolutePath` off this very module — so building on
   it would be a cycle. And the reason it is not the other way round either, with
   `parentOf` lifted up here and this built on it, is that the two disagree where
   it shows: `parentOf` answers `''` for a top-level path, the root, because the
   folder to re-read after a delete is a real place; this one drops the root from
   its answer entirely, because the root is expanded by construction and `''` in
   `project.expanded` would be an entry in `settings.json` naming the project
   itself. Two functions in two directories, then, and what keeps them apart is
   that one answer, not a count of callers.

   **The tree's space, and one separator only** — the mark this shares with
   `isUnder` rather than with its two neighbours above. Every path that reaches
   this is relative to the project and written with `/`, whatever the platform,
   because that is what `stores/files.js` writes and what `project.expanded`
   holds; the answers go straight into that list. A `\` accepted here would cut
   `a\b.txt` into a folder nobody has on a system where that is one ordinary
   filename.

   Order is the root first, which is the order the one caller walks them in. It is
   not a safety property and must not be read as one: that caller issues the
   directory reads without awaiting them one by one, and what makes the
   interleaving harmless is the tree being rebuilt from a reactive map rather than
   anything about this list — `revealInTree` in `views/DesktopApp.vue` carries
   that reasoning.

   `filter(Boolean)` is what keeps a trailing separator from producing an empty
   ancestor — `''` is the root, which is expanded by construction and would be a
   row nothing draws — and it is also what makes the answer the same for `a/b`
   and `a/b/`: the folders above the thing named, never the thing itself. */
export function ancestors(path) {
  if (!path) return []
  const parts = path.split('/').filter(Boolean)
  parts.pop()
  const out = []
  for (const part of parts) out.push(out.length ? `${out[out.length - 1]}/${part}` : part)
  return out
}

/* What a path is called from inside a folder, and `null` when it is not inside
   it at all.

   The one caller today is the Git panel's diff: a change arrives as a path
   inside one of the project's repositories, and the file behind it is read
   through `files_read`, which takes a path relative to the project root and
   refuses anything outside it. A repository named in `[project].repos` can sit
   anywhere at all — `../shared` is a legal entry — so "not inside" is an
   ordinary answer here and not a failure to be papered over with a guess.

   Both separators again, and for the reason above: the two paths come from Rust
   in the platform's own form while everything relative in `stores/files.js` is
   written with `/`. Comparing them without normalising would leave every
   repository on Windows looking like somebody else's folder. A trailing
   separator on the root is dropped so `/p` and `/p/` answer alike; the root
   itself answers `''`, which is what `files_list` already calls it. */
export function relativeTo(root, path) {
  if (!root || !path) return null
  const slashes = (value) => value.replace(/\\/g, '/')
  const base = slashes(root).replace(/\/+$/, '')
  const full = slashes(path)
  if (full === base) return ''
  return full.startsWith(`${base}/`) ? full.slice(base.length + 1) : null
}

/* Whether a path is a folder itself or something inside it — the question every
   verb that acts on a subtree asks, and the fourth copy of it is what put it
   here. The three it replaces are the delete and the move in `DesktopApp.vue`
   (close the tabs under it, rewrite `expanded`, rewrite the selection), the
   paste refusal in `components/files/fileClipboard.js`, and the sweep's
   fold-away in `stores/files.js`. Written out, each was
   `other === folder || other.startsWith(`${folder}/`)`, and the trailing
   separator is the whole of the rule: a bare `startsWith` makes `src-tauri` a
   child of `src`, so a delete of `src` would have closed the tabs of a folder
   nobody touched.

   **The folder comes first, the way `relativeTo` and `absolutePath` take their
   root first**, and the argument order is the only thing about this that a
   reader cannot check by looking at it.

   One separator and deliberately not both, which is what marks it out from its
   neighbours above: every caller compares paths in the tree's own space, where
   `stores/files.js` writes `/` whatever the platform, or absolute paths against
   each other in the platform's. A `\` accepted here would let `a\b` claim
   `a\b\c` on a system where that is one ordinary filename, and no caller has
   a mixed pair to reconcile — `relativeTo` is what converts, and it has already
   run by the time this is asked.

   The tree spells the root `''`, and one caller does reach this with it as the
   **folder**: `pasteRecord` in `DesktopApp.vue` puts every absolute clipboard
   path through `relativeTo`, which answers `''` for the project folder itself,
   so copying that folder in Finder asks `canPasteInto` about an empty one. The
   answer, for whatever path is asked about, is
   `path === '' || path.startsWith('/')` — the root, and anything spelled
   absolutely — which is what the written-out copy answered too, so the shape is
   preserved rather than chosen. The other three never ask: nothing deletes or
   moves the root, and the fold-away in `stores/files.js` refuses `''` in front
   of this call for a reason of its own. */
export const isUnder = (folder, path) => path === folder || path.startsWith(`${folder}/`)

/* The whole path, given the project's root and a path inside it — `relativeTo`
   the other way round, and here for the same reason it is: two parts of the
   front end want it at once. The file tree's menu wants it for the verbs that
   leave this window, since a path handed to `revealItemInDir` names whatever
   sits under the process's own working directory otherwise; and
   `stores/files.js` wants it for the system clipboard, which has no notion of
   a project and takes absolute paths in both directions.

   The separator is the root's own and never the tree's. Everything relative in
   `stores/files.js` is written with `/` whatever the platform, while the root
   arrives from Rust in the platform's form — so a path copied on Windows would
   read `C:\Users\you\dev\app/src/main.rs` if the two were simply joined. The
   root is the only evidence here of which system this is, which is why the
   question is asked of it rather than of the navigator: a root holding a
   backslash and no forward slash is a Windows path and nothing else is.

   No root is answered with the path unchanged rather than with a guess: that
   is the state before a project is open, and a path invented there would name
   something under whichever folder the app was launched from. */
const separatorOf = (root) => (root.includes('\\') && !root.includes('/') ? '\\' : '/')

export function absolutePath(root, path = '') {
  if (!root) return path
  const sep = separatorOf(root)
  const base = root.replace(/[/\\]+$/, '')
  if (!path) return base || root
  return `${base}${sep}${path.split('/').join(sep)}`
}

/* The mark on the "…N more" stub row a truncated directory listing ends with,
   and the test for one. A zero byte, because no filesystem lets one into a
   name, so a real path cannot collide with a stub's.

   Here rather than in `stores/files.js`, which invents the row, for the reason
   at the top of this file: `FileTree.vue` has to recognise a stub too now. Its
   context menu offers verbs about a path on disk — open a shell there, copy it,
   show it in the file manager — and a stub names nothing at all, so the menu
   must not open on one. A component importing the store to borrow the test
   would have pulled Tauri into a `.vue` file, which is the one import rule this
   front end has. The store re-exports it under the name it has always had, so
   nothing that used it there moved. */
export const STUB_MARK = '\u0000'

export const isStubPath = (path) => typeof path === 'string' && path.includes(STUB_MARK)
