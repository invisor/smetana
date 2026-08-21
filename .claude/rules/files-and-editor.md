---
paths:
  - "src-tauri/src/files/**"
  - "src/components/files/**"
  - "src/stores/files.js"
  - "src/stores/tabs.js"
---

# Files: the tree and the editor

The left panel's tree and the file tabs in the centre read the active project's directory through
`src-tauri/src/files/`, and it is deliberately the opposite of the tracker: no worker, no queue, no
watcher. `read_dir` costs milliseconds and holds no state, so there is nothing for a queue to guard —
the same reasoning that keeps settings out of a worker. `model.rs` is the vocabulary and the pure
logic (entry sorting, the `..` check, the binary sniff, the ceilings: 1000 entries per directory,
2 MB per file) and carries most of the tests; `fs.rs` is the disk; `commands.rs` is four thin
commands — `files_list`, `files_read`, `files_write`, `files_stat`.

Two rules in `fs.rs` are load-bearing. Every path is resolved with `resolve_within`, which
canonicalizes and refuses anything that lands outside the root — without it a symlink inside the
project would open the whole disk. And a write only happens when the file's `mtime` still equals the
one the front end was given; otherwise it is refused as `stale` and nothing is touched, because
Cmd+S on a tab opened an hour ago would otherwise erase an agent's work. That is also why
`read_text` takes the `mtime` **before** it reads the bytes: content and time cannot be read
atomically, and of the two ways to be wrong, a false `stale` costs one question while a stale mtime
sent forward as fresh costs somebody's work.

Freshness comes from window focus, not from a watcher: a second watcher subsystem in Rust, with its
own lifecycle and error reporting, costs more than the sweep in `catchUp` (`DesktopApp.vue`), which
re-lists the open directories and re-stats the open tabs whenever the window is focused — plus the
refresh button next to the project list.

What a row is drawn with comes from `src/catppuccinIcon.js` and not from this component — see
`CLAUDE.md` for why it sits at the top of `src/` — and the tabs in the centre and the Git panel's
change list resolve from the same table, so one file looks the same wherever it is named. It is an
`<img>` carrying a `data:` URL rather than an `Icon`, so the row reads the theme itself
(`src/documentTheme.js`) and hands it down: a palette applied in JS is the only way these icons
follow `data-theme` at all, since nothing inside a `data:` URL is reachable by the stylesheet.

A row answers a secondary click with a menu of its own, and the space below the
last row answers with the same menu about the project's root — without that
second half, a project whose first screen is nothing but unopened folders has no
way to reach the verbs that make a file. The items are `files/fileMenu.js`'s, a
pure module for the reason the whole of that family is one; the panel is a single
`PointerMenu` held by `FileTree.vue`, the way `ProjectRail` and `BranchList` each
hold one for their list; and what a verb *does* is `DesktopApp.vue`'s, because
the stores live there and a component that imported one would be the second
exception to a rule with exactly one. The two halves are joined by hand — a
`kind` renamed on one side draws perfectly and does nothing when pressed, the
same seam `newTabMenu.js` and `onNewTab` have — so the test pins the producing
side.

Four decisions in it are worth knowing before changing any of them. The menu
never moves the selection: a right click is a question about a row, not a visit
to it, so the row under the panel takes the *hover* surface rather than the
selected one, which would claim the selection had moved. Open in terminal is a
shell tab of this app's own rather than Terminal.app — the app already has shell
sessions, and a second emulator would put the person in a window outside every
notion of a session there is here — which is what the optional `cwd` on
`terminal_shell` is for; it is checked with `resolve_within` beside the spawn
(`shell_cwd` in `terminal/service.rs`) and nowhere else, so there is one copy of
that rule. Attach to agent is the drag-and-drop gesture by another route and goes
through the same `dropText`, because a second way to write a path into a prompt
would be a second quoting rule to keep correct. And the clipboard branch is
chosen *before* the call the way `openExternal` chooses its own: `mockBackend.js`
refuses every unknown command loudly, so a plugin call in `npm run dev` does not
fail in a way worth falling back from — it fails always, and both copy items
would be uncheckable there.

The "…N more" stub row is deliberately not one of the rows that opens it. Every
verb on the menu is about something on disk and a stub names nothing, so
`isStubPath` moved out of `stores/files.js` and up to `src/paths.js`, where a
component can reach it; the store re-exports it under the name it always had.

`src/stores/tabs.js` owns the centre's tabs — order, which one is temporary, which is active, the
buffers and their dirtiness — and knows nothing about Tauri; the disk is `files.js`. Not all of that
row is files, and two of its kinds are **derived rather than remembered**: the Agent tab, which exists
only while the project has an agent session, and one tab per shell session. Both come from
`terminals.js` and both are `.claude/rules/terminal.md`'s to explain; what matters here is that
neither is in `openTabs`, so nothing about them survives a restart, and a consumer of `tabList` has to
switch on `kind` rather than assume a tab id is a path. The split is by
lifetime: the list of open tabs survives a restart and lives in settings, the buffers do not and live
here. The mechanics are VS Code's: a single click opens a preview tab that the next single click
replaces in place, a double click makes it permanent, and so does the first edit — which is what
makes "a preview tab is never dirty" true. A buffer is
`{ text, original, mtime, error, saveError, stale, loading }`, and three of those keep a person's
text safe: `loading` refuses edits and writes until the first read comes back (a character typed into
a not-yet-read buffer would otherwise become the whole file on the next save), `stale` asks instead
of choosing when the file moved under a dirty tab, and `error` locks the field without throwing away
anything already typed.

The field itself is CodeMirror 6, assembled by hand under `src/components/files/editor/`: `theme.js`
(chrome and syntax highlighting, entirely on tokens — see the styling exception in `CLAUDE.md`), `extensions.js`
(an explicit extension list instead of `basic-setup`, which would have pulled in autocomplete, a
linter and code folding), `languages.js` (a map from file extension to a dynamic `import()`, one
chunk per language, loaded the first time a file of that type opens and cached after) and `states.js`
(a non-reactive `Map` from path to `{ state, scrollTop }`, so a tab keeps its caret, selection, undo
history and scroll position across being switched away from and back).

The theme is one theme for both app themes and both densities. Every value in it is a token
reference, so the browser repaints it on its own when `data-theme` changes — the editor is never
rebuilt. `EditorView.theme()`'s `{ dark: true }` flag is deliberately not passed: it would raise the
`EditorView.darkTheme` facet, which the base themes bundled with the search panel and
special-character rendering watch for, and they would substitute their own hardcoded colours through
`&light`/`&dark`. `theme.js` is exhaustive instead. Bracket matching is repainted too, but not for
that reason: its base theme is a flat, unconditional colour that never watches the facet at all.

Three more decisions in that area look like they could be tidied away and are load-bearing, each
paid for with a real defect. The `tabList` watcher that prunes abandoned states (`DesktopApp.vue`)
runs with `flush: 'post'`: with the default `pre` the cleanup runs before `FileEditor` has saved the
outgoing tab's state via `putState`, so the save re-inserts the entry just removed and the closed
file reopens carrying the caret, scroll position and undo history of its previous life while looking
perfectly fresh. The compartments (`editor/compartments.js`) live at module scope, not inside
`FileEditor.vue`: a compartment is a key, not a value — the value lives in the `EditorState`, which
outlives the component instance that created it — so per-instance compartments would mean a state
restored by a later instance carries keys that instance never registered, and `reconfigure` against
them would silently do nothing. And `replaceDoc`'s `Transaction.addToHistory.of(false)`: content
arriving from disk is not a person's edit, and letting it enter the undo history means one Cmd+Z on a
freshly opened file empties the document, the emptiness lands in the buffer, and the next save writes
it to disk. This deliberately makes Reload after `stale` non-undoable too — that choice is offered up
front by the Keep mine button, not recoverable afterward by undo.

`adoptState` in `FileEditor.vue` is the single place a cached state is installed, and it earned that
by being two places first — `onMounted` and the watcher's path-change branch each decided
independently what "adopt" meant, and both times they disagreed a person's edits went missing.
Anything a saved state closes over that belongs to a component instance — today the update listener
that turns CodeMirror transactions into `emit('update:modelValue')`, itself kept in a compartment for
exactly this reason — has to be re-pointed at the live instance on adoption, and doing that in one
function is what stops the two call sites from drifting apart again.

An unknown file extension and a language chunk that fails to load (offline, a broken deploy) are both
ordinary outcomes in `languages.js`, not errors: the file opens as plain text either way, because
losing syntax highlighting is not a reason to break the editor.
