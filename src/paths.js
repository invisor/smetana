/* The rules about a path that belong to no one part of this front end: what a
   path is called, what it is called from inside a folder, and whether a row in
   the tree is a path at all.

   What they have in common is the test for belonging here, and it is the test to
   apply before adding a fourth: each is pure — no Vue, no Tauri, no DOM — and
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
