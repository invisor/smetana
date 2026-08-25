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
logic (entry sorting, the `..` check, the name check, the binary sniff, the ceilings: 1000 entries
per directory, 2 MB per file) and carries most of the tests; `fs.rs` is the disk; `commands.rs` is
thin commands over it — the reads (`files_list`, `files_read`, `files_stat`) and the writes
(`files_write`, `files_create`, `files_mkdir`, `files_trash`).

**A listing is a `read_dir` and one spawn of git**, and that second half is new: the module spawned
nothing at all for most of its life. `list_dir` ends by asking `git check-ignore -z --stdin` which of
the entries it just read are ignored, so the tree can draw those rows muted the way VS Code's explorer
does. Everything the header above says still holds; only the cost moved.

**The numbers are measured, not guessed** — Apple Silicon, macOS, warm cache, the call in the shape
`mark_git_ignored` makes it. An ordinary listing of a couple of dozen names in this repository (index:
508 files) is **7 ms median**; a full `MAX_ENTRIES` listing of 1000 names in a small repository is
**16 ms**. **It is the index that scales it and not the listing**, because `check-ignore` consults the
index — which is what buys the `git add -f` case below — so the cost follows the size of the
repository: the same 1000-name listing against a 50 000-file index came to **0.5 s**. A wide folder in
a monorepo is the worst case worth knowing, and what it costs is rows arriving a beat late.

**Two multipliers, and together they are why `files_list` runs off the async runtime.** `catchUp` in
`DesktopApp.vue` re-lists every open folder on window focus, and `refreshDirs` in `stores/files.js`
fires them as a `Promise.all` — so a focus is N concurrent `files_list` calls, one git spawn each,
against a runtime holding one worker per core. `files_list` therefore does its work in
`spawn_blocking` and not in the body of its `async fn`, which is `vcs/commands.rs`'s rule stated in
bold and naming this module as the victim: every IPC call in the app shares that runtime, so a git
that is merely slow would take workers out of everything else on screen with nothing saying why. That
rule did not apply here while a listing was a sub-millisecond `read_dir`; it applies now, and it is
the one thing in this feature that is easy to leave out and expensive to leave out.

Four decisions inside that one call, each of which is the whole reason it works. It is made **after**
the `MAX_ENTRIES` truncation, because the ceiling exists so a click on `node_modules` cannot wedge the
render and asking git about forty thousand names on their way to the bin would spend the whole saving.
The **working directory is the folder being listed** — the absolute path `resolve_within` has already
vouched for — which is the entire multi-repository story: git walks up from there and finds whichever
repository owns the folder, so a nested repository, a worktree and a project of several repositories
side by side are all served right, with no line here knowing which folder is whose, no
`repos::discover` and no `project.toml`. **Inheritance is free**: git reports `.bin` inside an ignored
`node_modules` as ignored on its own, so every listing answers for itself and no flag is carried down
the tree. And **git answers rather than a matcher of ours** — a re-inclusion under an excluded parent
(`.claude/*` then `!.claude/rules/`), a pattern anchored to the repository root, an ignore file at
every level plus `.git/info/exclude` plus a person's global `core.excludesFile`, and the index, which
is what leaves a file added with `git add -f` at full strength. A second implementation would agree
for a week and drift afterwards, and the drift would surface as a row in the wrong colour with nothing
to point at.

**Nothing about it can reach a person.** `check-ignore` exits 1 when nothing matched and 128 when the
folder is in no repository at all, and both mean the same to the tree: no row drawn muted, no toast,
nothing in `filesState.lastError`. A folder outside git is an ordinary state, the standing `git.rs`
already takes for the branch in the scope bar, and the worst outcome of any failure here is the tree
exactly as it looked before this existed.

**Neither of those two reaches the log either.** Exit 1 is `run::git_maybe_fed`'s `absent` argument
and never was an error. 128 arrives as `VcsError::Git { status: 128, .. }` and is matched by that
code, because the alternative is a line per open folder on every window focus, forever, in every
project nobody has put under git — and a channel that noisy says nothing when a real failure needs it.
What that gives up is real and small: 128 is git's code for any fatal, so an unreadable index or a
permissions problem is now as quiet as an ordinary folder. The failures that do **not** recur still
speak — `VcsError::NoGit` and a read that hit `READ_CEILING` are logged to stderr.

`run::git_maybe_fed` is `git_maybe` with bytes written to the child's standard input, and it exists
for this one caller. `bounded` gives every other git call `/dev/null` there on purpose — git with an
inherited stdin waits on the prompt it opened, and there is no terminal on this process to answer in —
so the feed is a pipe that function owns, written **after** both output readers are running and then
dropped, the drop being the end of file. The alternative was the same names as arguments, and it fails
on Windows: a listing is up to `MAX_ENTRIES` names, which a directory of long names pushes past the
32 767 characters `CreateProcess` accepts, precisely in the directories worth asking about.

What fills the row is `Entry::ignored` through `treeNodes()` in `stores/files.js`, as
`git: 'ignored'` — the **only** value `FileTreeRow`'s `git` prop is ever given in the product.
It draws `--git-ignored` at `opacity: 0.7` with no status letter and no tooltip, since `GIT.ignored.l`
is the empty string. The prop's five other kinds — modified, added, deleted, untracked, conflict —
have no source and want a different one, a `git status` per repository with a freshness question this
has none of. So does a muted file **tab** in the centre, which VS Code also draws and this does not:
that is `stores/tabs.js`, outside the tree altogether. And there is no watcher on `.gitignore` — an
edit reaches the tree on the next window focus or refresh, like every other change on disk here.

Three rules in `fs.rs` are load-bearing. Every path is resolved with `resolve_within`, which
canonicalizes and refuses anything that lands outside the root — without it a symlink inside the
project would open the whole disk. A path that does not exist yet cannot be canonicalized at all, so
making something goes through `resolve_new_within` instead: it canonicalizes the **parent**, which
does exist, checks the name separately (`reject_bad_name` — empty, `.`, `..`, a separator) and
refuses a name already taken. Splitting the argument into a folder and a name is not a convenience,
it is the check: a path joined whole could carry a `..` past the directory the guarantee was just
made about, and `reject_bad_name` is what makes the join safe. Its last clause demands exactly one
ordinary `Path` component, which is what refuses `.`, `..`, a root, and — on Windows — a drive
prefix: `Path::join` follows `PathBuf::push`, where a prefixed path *replaces* the receiver, so
`C:evil.txt` would land outside the project entirely. Four clauses stand in front of that one and
none is spare. Two are the cases it cannot see: on unix a backslash is an ordinary character and
`C:` is an ordinary name, so `Path::new("a\b.js")` and `Path::new("C:evil.txt")` are each one
`Normal` component, and both are cut by hand on every platform, the way `reject_traversal` cuts the
same two shapes for the same reason — delete either and the guard is gone from every unix build
while every test still passes on Windows. A third is the trim, which is the only thing that refuses
a name of nothing but spaces: `Path::new("   ")` is one ordinary `Normal` component, so the last
clause takes `""` and never `"   "`. The fourth, the `/`, the last clause would catch on its own.

**Deleting takes that same shape, and not `resolve_within`.** Canonicalizing the last component is
right for reading and wrong for destroying: a symlink is drawn in the tree as an ordinary row
(`list_dir` asks `file_type`, which does not follow one), so deleting `node_modules/.bin/vite` would
take the package's real script; and a link pointing at the project's own root resolves to the root,
as does the literal `.`, at which point the app throws away the folder it is looking at. So the
parent is resolved, the last component is checked as a name, and what is joined back on goes to the
trash **un-canonicalized**. There is no list of root spellings anywhere — a name that is one
ordinary component cannot be the folder it sits in, whatever it is spelled like.

**On macOS the trash is asked for by a method the crate does not default to.** `trash::delete` there
is `DeleteMethod::Finder`, an `osascript` subprocess driving Finder over Apple Events, and it cannot
delete a symbolic link at all: it exits 0, prints nothing, and leaves the link where it was — which
in this repository is every row under `node_modules/.bin`. It also needs an Apple Events grant, and
`tauri.conf.json` declares no macOS bundle block, so a signed and hardened build has no
`NSAppleEventsUsageDescription` to ask with. `platform_trash` in `fs.rs` sets `NsFileManager`
instead, which is `trashItemAtURL`: no subprocess, no permission, a link removed as a link. The cost
is Finder's "Put Back" on some systems, and dragging an entry out of the Trash is still the
platform's ordinary means.

Whatever the platform, the entry is checked once more **after** the call, and a deletion that did
not happen comes back as a refusal. A row that says it went and is still in the tree when the folder
is re-read a moment later is worse than a toast, and this is the net under whichever platform grows
that behaviour next.

And a write only happens when the file's `mtime` still equals the
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

**Delete goes to the system trash** (the `trash` crate), never `remove_dir_all`: a deletion somebody
can undo from their own file manager is a smaller promise than a permanent one, and this is a tree
of somebody's sources. A folder goes with everything under it, which is what a trash means. After
it, `DesktopApp.vue` closes every tab over what is gone — `stale` in the editor is about a file that
changed, not one that stopped existing, and a buffer over nothing has nowhere to save — and it
filters `tabList` by `kind`, because the Agent tab and the shell tabs are not paths. A diff tab is
found through `diffTabs` instead — its id is a repository and a path with a zero byte between them
and would never match — and matched in the **tree's** path space rather than on the bare `tab.path`,
which is relative to a repository and not to the project. The record's pair is joined and put
through `relativeTo`, the conversion `loadDiff` already makes for the same reason; `null`, a
repository outside this project, matches nothing. In a project of several repositories the bare
field would leave a diff open over a file that is gone and close one over a file that is not.

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

Two of its verbs make something, and neither opens a dialog: `FileTree.vue` puts a **draft row**
where the entry will be — `FileTreeDraftRow.vue`, a field at the depth of the folder it is going
into — and Enter commits, Esc and losing the focus cancel. The row's position is the answer to
"where is this going", which a modal in the middle of the screen cannot give. The draft is that
component's own state and deliberately never a node in `nodes`: the tree is rebuilt from
`files_list` whenever `catchUp` re-reads a folder, and a draft mixed into that list would vanish
mid-word. What the typed name comes to is `files/newEntry.js`, three outcomes rather than two —
an empty field is a cancel and says nothing, a name no entry can carry is a refusal with a sentence,
and neither goes near Rust.

Delete asks a second time **in the row itself**: the first pick redraws it as "Click again to
confirm" and leaves the panel up, the second deletes, and anything else — Esc, a click outside, a
scroll, another row — leaves nothing done. That is the one change this work made to the shared
machinery: an item may carry `keepOpen`, and `PointerMenu.pick` emits without closing for it alone,
which narrows the close-before-emit order that file's header explains rather than lifting it. The
flag is on the item and not on the component because one menu holds rows of both sorts, and it is
also what the handler branches on — closing clears the caller's armed flag before the pick is
emitted, so the item is the only copy of "which pick is this" that cannot be stale by the time it
is read.

Four more decisions in it are worth knowing before changing any of them. The menu
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
would be a second quoting rule to keep correct — and it types into
`terminalState.activeId` or into nothing at all, never into "the newest live
agent", because the safety of the gesture is that a person watches the text land
and a path delivered to a session they are not looking at sits in somebody
else's half-written prompt with nothing on screen to say it went there. That is
also why the item's flag is `canAttach` and not the store's wider
`hasAgentSession`, which counts a start ticket and an exited session — and why
the greyed row has two sentences rather than one. Narrowing to the selection
buys a state where the refusal is true and "no agent to type into" is not: an
agent finishing while another runs leaves the selection on the finished one, and
nothing moves it back, so the row says "select an agent first" whenever there is
one to pick. `hasLiveAgent` carries only that difference and never the greying.
And the clipboard branch is chosen *before* the call the way `openExternal`
chooses its own: `mockBackend.js` refuses every unknown command loudly, so a
plugin call in `npm run dev` does not fail in a way worth falling back from — it
fails always, and both copy items would be uncheckable there.

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

**The row is dragged into whatever order somebody wants, and the order is one row rather than four
lists.** A terminal tab can stand between two files and a diff in front of all of them; the pinned
run — the board, and the Agent tab while it exists — does not move and nothing can be put to the left
of it, which is also what keeps the `afterPinned` slot's `+` button beside that run. The whole of the
rule is `src/components/shell/tabOrder.js`, another of the pure family and shaped exactly like
`kanban/columnOrder.js`: `orderTabs` reconciles a stored order against the row that actually exists,
`moveTab` is one tab from one index to another, and `neighbourIn` answers which tab takes over when
one is closed. What is stored is `tabOrder` on the project (`.claude/rules/settings.md`) and it is a
hint, never the truth — an id nothing matches shifts nobody, which is the ordinary state after a
restart, when the diffs and the shells in it are gone. Nothing sweeps those entries on the way in:
the next drag rewrites the field whole from the tabs standing at that moment, so the file cleans
itself up and can never outgrow the row.

Two things about it are easy to undo by accident. **Exactly two places write to `tabOrder`** — the
drag's commit, and `openFile`'s preview replacement, which puts the incoming path at the outgoing
one's index for the reason it already splices `openTabs` in place: without it, the first single click
after a rearrangement finds the new file unknown to the order and sends it to the end, so walking a
folder by single clicks would drag the preview tab across the row on every press. And `neighbourIn`
is asked about the **movable** part of the row, deliberately not the whole of it: with the board
always present there would be no "nobody left" case, and closing the last file tab while an agent
runs would land on the Agent tab instead of the board.

The gesture is `TabBar.vue`'s and is `KanbanBoard.vue`'s in almost every decision — a draft order
that lives only while the pointer is down, the row rearranging live under the pointer with nothing
following the cursor and nothing transformed, capture on the scrolling strip, Esc abandoning,
`pointercancel` and `lostpointercapture` ending it too, and the same pair of page-wide guards.
HTML5 `draggable` is **not** an option here and the reason is not taste: `dragDropEnabled` is on in
`tauri.conf.json`, so Tauri intercepts a drag at the window level before the webview sees it, and
`stores/attachments.js` and `stores/terminals.js` both hang on `onDragDropEvent` — turning it off
would take dropped files away from the terminal and from the new-task dialog.

Two places where copying the board verbatim was wrong, and both cost a real defect before they were
found.

**The capture is not taken on the press.** Pointer capture retargets the compatibility mouse events,
so a capture in `pointerdown` sends the `mouseup` to the strip, has the `click` dispatched at the
nearest common ancestor — the strip again — and `Tab.vue`'s `@click` and `@dblclick` then never fire:
a single click stops selecting a tab and a double click stops promoting a preview. The board does
capture on the press and is right to, because a column header carries no click of its own. So a press
here only **arms** — it remembers the tab and the box it was pressed in — and the capture is taken
only when the pointer leaves that box sideways. A release inside it was an ordinary click all along.

**The latch is on the cell the pointer is in, not on the held tab's.** Columns are one width, so
after a swap the pointer is over the held column again by construction; a tab is as wide as its label
up to 200px, so a narrow tab dragged through a wide one comes to rest *beside* the pointer rather
than under it. Latching the held tab's box is therefore a no-op in exactly the case that needs it —
measured at 4 alternations over 34px of travel as the row is drawn, and 11 over 30px with the wide
tab at the 200px cap. What is latched is whichever cell the pointer ended up in, measured a tick
after the swap because the row has not been redrawn at the moment the draft changes. Leaving that
cell is then always a move in the direction of travel.

Terminal tab numbering is **not** touched by any of this and should not be: `terminalTabs` numbers by
position among the shell sessions rather than among the tabs, so a row reading `[Terminal 2][Terminal
1]` is an accepted outcome. Numbering that followed the row would rename a tab under the hand that
just moved it.

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
