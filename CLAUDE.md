# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

It is tracked, and that is a decision rather than an accident. `.gitignore` carried a blanket
`CLAUDE.md` line for most of this project's life, so the one document describing this architecture
lived in exactly one checkout: every worktree cut for a task started without it, and an agent working
in one only saw these rules because the harness happened to read the copy in the main checkout —
a property of the harness, not of the repository. The cost was paid twice over. Nothing in a worktree
could correct the document when the code moved under it, so it drifted, and by the time anyone
noticed it was naming six stores where the tree held eight and had never heard of two whole
subsystems. The blanket rule was also wider than anyone meant it: the vendored superpowers plugin
ships a `CLAUDE.md` of its own, and it had to be rescued by a negation on the very next line to stay
committed at all. Both lines are gone. A document that is wrong is a thing to fix in a diff, and an
untracked file has no diff.

## Language

**All comments are written in English, without exception.** This holds for every language in the
tree — Rust `//`, `///` and `//!`, JavaScript and Vue `//` and `/* */`, HTML comments in templates,
CSS, shell and config files. Russian was the earlier convention; the whole tree was swept and there
is no Cyrillic left in it, so a Russian comment in a diff is a regression, not a leftover.

The rule reaches past comments to everything else written for a reader inside the code: test names
(`describe`/`it` titles, Rust `#[test]` function names), assertion messages, `panic!`/`expect`
strings, `thiserror` messages, log lines, `console.*` text and fixture strings. Commit messages stay
in Russian, matching the whole history. UI copy is English, sentence case (see Constraints).

## Commands

```sh
npm install          # postinstall fetches the bd sidecar; it warns and continues without one
npm run dev          # http://localhost:5173 — front end alone, backed by the mock
npm run build
npm run preview      # serve the production build
npm run tauri dev    # the actual desktop app: Rust worker, real bd, live board
npm run fetch-bd     # download the bd binary explicitly (fails hard, unlike postinstall)
npm test             # front-end tests (vitest), single run
npm run test:watch   # the same, in watch mode
cd src-tauri && cargo test
```

Two test runners: `npm test` covers the front end's pure logic — the plain modules and the stores —
and `cargo test` covers the Rust side. That used to say "the nine plain modules" and had been wrong
for some time before anybody noticed; `tests/` mirrors `src/`, so the directory is the count and it
cannot drift the way a number written once does. This is the same habit the stores paragraph below
warns about, fixed the same way: name where the list lives, not how long it was on the day somebody
looked. Neither runner covers components: there is no component test runner and no linter or
formatter, so do not invent one, and do not claim a change is "tested" on the basis of a build
succeeding.

Front-end tests live in `tests/`, never next to the source. They mock exactly one thing — the IPC
transport — through the official `mockIPC`, and rebuild the store module graph per test;
`tests/support/stores.js` explains why.

A component change is still verified by eye, in the dev server, with the query parameters the app
reads (`src/App.vue`):

| parameter | values | default |
|---|---|---|
| `theme` | `dark`, `light` | `dark` |
| `density` | `comfortable`, `compact` | `comfortable` |
| `view` | `gallery` | the app |

`?view=gallery` renders every exported component once (`src/views/Gallery.vue`) — the harness for
catching a broken component. Check any component change in all four theme × density combinations,
since both switches only swap CSS custom properties and a component that hardcodes a value will
look correct in exactly one of them.

## Architecture

A desktop app for supervising autonomous AI coding agents: a Tauri 2 shell (`src-tauri/`, Rust)
around a Vue 3 front end (`src/`). The front end is ported from the **Smetana Design System**
(`claude.ai/design`, project `5da5ca35`). Tokens are copied from the design system verbatim;
components are ports of its React sources, keeping prop names, computed styles and behaviour. React
`value`/`onChange` became `v-model` (`modelValue`); React `children` props became named slots. When
something looks odd, the design system is the source of truth — match it rather than "fixing" it.

`src/main.js` → `src/App.vue` → either `views/DesktopApp.vue` (the three-column shell: worktree
files + agents, tab bar over the kanban, task inspector) or `views/Gallery.vue`
(code-split, never in the app bundle). The board is live tracker data, and so are the file tree, the
file tabs, the agents and the branch in the scope bar. What is left on the screen — the rest of the
git state, the dirty-file and agent counters among it — is still fixture state in
`views/desktopAppData.js`. The right column used to end in a log pane fed from that same file; it is
gone, because invented output under a real issue claimed the app knew something it did not, and a
session's actual output is the terminal tab. `LogView` itself stays in the library and in the
gallery — the component is fine, the fixture in the product was not.

That right column draws one of three things, and which one is **derived rather than stored**
(`rightPanel` in `DesktopApp.vue`). `DraftInspector` shows a task still being filed — the person's
own words, read-only, with no issue behind them. `ClaimedTasks` shows what a run has taken, with the
card for whichever of them is picked, the list staying above the card because the choice between them
is the point and a card that replaced the list would take the way back with it. Otherwise it is
`TaskInspector` on the selected issue. Deriving it is what stops the halves drifting: `selectedTask`
is remembered per project in `settings.json`, so a panel choice that wrote to it would turn a glance
at an agent into an edit of a preference, and a stored version had the run case highlighting a card
on the board that the inspector then refused to draw.

### The tracker bridge

The board shows the **bd** issue tracker of the active project's directory — chosen from the project
list, and remembered between runs — and follows it as it changes, no matter who changed it: this
window, an agent, or a person in a terminal. bd has no daemon and no API — its CLI is the API, and
one call costs about two seconds. Hence the shape of `src-tauri/src/tracker/`:

| file | what it does |
|---|---|
| `model.rs` | `Issue`, `ColumnDef`, `Delta`, `Health`, `TrackerError` — the vocabulary the front end sees |
| `bd.rs` | the only file that knows bd's CLI: arguments, spawning, parsing |
| `store.rs` | the in-memory snapshot and the delta computation |
| `watcher.rs` | `notify` on `.beads/`, path filter, failure reporting |
| `service.rs` | the worker: one owner of mutable state, a request queue, deltas and health events |
| `commands.rs` | thin `#[tauri::command]`s that put a request on the queue and await the reply |

`service.rs` is a single tokio task holding the snapshot; commands, watcher ticks and the 60-second
safety sweep meet in one `select!`. Nothing shares mutable state with it — that is what keeps
~2-second bd calls from blocking each other unpredictably. Writes go straight into the snapshot and
out as a delta without waiting for the watcher. `generation` advances by exactly one per emitted
delta; the front end resyncs when it sees a gap. `store.rs` and the argument builders in `bd.rs` are
pure and carry the unit tests.

`Issue` carries every field bd emits, not only the ones the board draws: the panel on the right
(`components/kanban/TaskInspector.vue`) shows all of them, and a field left out of the struct is
invisible there with nothing to say it went missing. Three fields broke that for a while and the
cost was exactly what the rule predicts (smetana-dbr): bd emits `notes`, `design` and
`acceptance_criteria`, the struct had none of them, and `notes` is where `running-tasks` writes the
reason a run parked a task — so the one sentence explaining why the night left a task alone was
readable only through `bd show` in a terminal. All three are back, drawn as prose under the
description in the spec-then-log order (acceptance criteria, design, notes). The one thing still
deliberately dropped is not on `Issue` but on its edges: a dependency in bd's JSON carries its own
`created_at`, `created_by` and `metadata`, and `Dependency` keeps only the ids and the kind,
because the panel draws an edge as a "Blocked by" id and bookkeeping about the edge itself has
nothing there to be drawn as. That panel is read-only apart from the status —
rewriting a title or a description is an agent's job, and "Ask agent to edit" starts one on the
issue. The status picker offers three of bd's eleven statuses (Ready, Pinned, Done); the rest belong
to agents, so the one the issue actually holds is appended as a fourth option when it falls outside
those three, because a picker with no matching option would render its first entry and claim the
issue was Ready. Delete is `bd delete -f` — irreversible, and `-f` is not about skipping a prompt:
without it bd prints a preview, deletes nothing and exits zero.

Which directory that is comes from `src-tauri/src/project.rs` — the vocabulary the tracker and the
settings share: `has_tracker`, `nearest_tracked_ancestor` (a folder inside a tracked repository
resolves to its root, so the list, the settings key and the worker all name the same directory) and
`default_project` for the very first run. Picking a folder is the `tauri-plugin-dialog` open dialog,
allowed by `dialog:allow-open` in `capabilities/default.json`; the picked path is normalized once,
by the `project_root` command, before it reaches the list.

Health (`ok`, `no-project`, `not-a-beads-repo`, `bd-version-mismatch`, `error`) is both an event and a command:
the event fires microseconds after start, before the webview can subscribe, so the worker also
answers `tracker_health`. `DesktopApp.vue` renders it where the board would be — quietly, since the
loud budget belongs to the card that needs a human.

`src/stores/tracker.js`, `src/stores/settings.js`, `src/stores/projects.js`, `src/stores/files.js`,
`src/stores/terminals.js`, `src/stores/git.js`, `src/stores/runs.js` and `src/stores/attachments.js`
are the **only** files in `src/` that know Tauri exists — components see reactive stores and nothing
else. `mockBackend.js` below is the ninth and the exception that proves it: it imports Tauri in order
to stand in for the absence of one. Several of those files open by counting themselves into this list
(`runs.js` says it is the seventh, `attachments.js` says the same of itself), which is a habit worth
knowing about before trusting one: an ordinal is written once and the list keeps growing under it.
The list here is the one to check against the tree. `tracker.js` also
owns the two translations: bd's statuses to the design system's (`open → ready`, `in_progress →
running`, `closed → done`; everything else, including custom statuses, passes through to
`normalizeStatus` and gets a hash colour with a 2-letter code), and Rust's diagnostics to short
English messages, with the raw text left in the console. `projects.js` owns the list of open
projects, which one is active, and moving between them — the front end holds the list's truth, bd
holds the board's, so a switch reads the new project's layout with `settings_load` (only the layout:
the list on disk is already the past by then) before it asks the tracker to point at the new
directory — plus offering `bd init` in a folder that has none yet.

In a browser there is no back end, so `src/stores/mockBackend.js` installs the official `mockIPC`
with the old fixtures: read commands answer, and writes to the tracker reject loudly — a "write"
that looked like it worked would be worse than none. `settings_save` is the one exception: it is
accepted and dropped, because a browser has nowhere to keep it and failing every debounce tick
would only fill the console. That is what keeps `npm run dev` and `?view=gallery` working with no
branching in components.

### The bd sidecar

bd ships inside the bundle (`bundle.externalBin`), so the app is self-contained and the version is
fixed. The binary is 128 MB and is **not** committed: `scripts/fetch-bd.mjs` downloads the pinned
release, verifies it against the sha256 digests committed next to `BD_VERSION` (the release's own
`checksums.txt` is only a cross-check), and lays it out as `src-tauri/binaries/bd-<target-triple>`.

`postinstall` runs it with `--optional` and only warns on failure — a contributor who wants the
front end alone should not need a Rust toolchain and a 43 MB download. `npm run fetch-bd` and CI
fail hard instead. `EXPECTED_BD_VERSION` in `service.rs` must stay in step with `BD_VERSION` in the
script; a mismatch surfaces as `bd-version-mismatch` in health, not as a crash.

The app is not the only thing that runs bd any more: an agent files a task by running `bd create`
itself, and it reaches the same binary because `terminal/pty.rs` puts the sidecar's directory on the
front of the `PATH` the agent inherits. See the terminal section for why in front and not behind.

### Files: the tree and the editor

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

`src/stores/tabs.js` owns the centre's tabs — order, which one is temporary, which is active, the
buffers and their dirtiness — and knows nothing about Tauri; the disk is `files.js`. The split is by
lifetime: the list of open tabs survives a restart and therefore lives in settings, the buffers do
not and therefore live here. The mechanics are VS Code's: a single click opens a preview tab that
the next single click replaces in place, a double click makes it permanent, and so does the first
edit — which is what makes "a preview tab is never dirty" true. A buffer is
`{ text, original, mtime, error, saveError, stale, loading }`, and three of those exist to keep a
person's text safe: `loading` refuses edits and writes until the first read comes back (otherwise a
character typed into a not-yet-read buffer would become the whole file on the next save), `stale`
asks instead of choosing when the file moved under a dirty tab, and `error` locks the field without
throwing away anything already typed.

The field itself is CodeMirror 6, assembled by hand under `src/components/files/editor/`: `theme.js`
(chrome and syntax highlighting, entirely on tokens — see the styling exception above), `extensions.js`
(an explicit extension list instead of `basic-setup`, which would have pulled in autocomplete, a
linter and code folding), `languages.js` (a map from file extension to a dynamic `import()`, one
chunk per language, loaded the first time a file of that type opens and cached after) and `states.js`
(a non-reactive `Map` from path to `{ state, scrollTop }`, so a tab keeps its caret, selection, undo
history and scroll position across being switched away from and back).

The theme is one theme for both app themes and both densities. Every value in it is a token
reference, so the browser repaints it on its own when `data-theme` changes — the editor is never
rebuilt. `EditorView.theme()`'s `{ dark: true }` flag is deliberately not passed: it
would raise the `EditorView.darkTheme` facet, which the base themes bundled with the search panel
and special-character rendering watch for, and they would start substituting their own hardcoded
colours through `&light`/`&dark` selectors. `theme.js` is written to be exhaustive instead —
everything a base theme would otherwise paint is repainted with a token, so nothing is left for
`darkTheme` to contribute. Bracket matching is repainted too, but not for that reason: its base
theme in `@codemirror/language` is a flat, unconditional colour that never watches the `darkTheme`
facet at all — it would need overriding whether or not the flag were passed.

Three more decisions in that area look like they could be tidied away and are load-bearing, each
paid for with a real defect during this work. The `tabList` watcher that prunes abandoned states
(`DesktopApp.vue`) runs with `flush: 'post'`, not the default `pre`: with `pre` the cleanup runs
before `FileEditor` has reacted to the outgoing tab's path change and saved its state via `putState`,
so the save re-inserts the entry the cleanup had just removed. The closed file then reopens carrying
the caret, scroll position and undo history of its previous life while looking perfectly fresh — the
first Cmd+Z is what gives it away. The compartments (`editor/compartments.js`) live at module scope,
not inside `FileEditor.vue`: a compartment is a key, not a value — the value lives in the
`EditorState`, which outlives the component instance that created it — so per-instance compartments
would mean a state restored by a later instance carries keys that instance never registered, and
`reconfigure` against them would silently do nothing. And `replaceDoc`'s
`Transaction.addToHistory.of(false)`: content arriving from disk is not a person's edit, and skipping
this would let it enter the undo history, so one Cmd+Z on a freshly opened file would empty the
document, the emptiness would land in the buffer, and the next save would write it to disk. This
deliberately makes Reload after `stale` non-undoable too — the choice between a person's text and the
file's is offered up front by the Keep mine button, not recoverable afterward by undo.

`adoptState` in `FileEditor.vue` is the single place a cached state is installed, and it earned that
by being two places first: `onMounted` (a file tab returning after the board or terminal unmounted it)
and the watcher's path-change branch (a live editor switching to a different open tab) each decided
independently what "adopt" meant. Both times they disagreed, a person's edits went missing: once
because neither re-pointed the update listener at the live instance, once because only one of the
two did. Anything a saved state closes over that belongs to a component instance — today the update listener
that turns CodeMirror transactions into `emit('update:modelValue')`, itself kept in a compartment for
exactly this reason — has to be re-pointed at the live instance on adoption, and doing that in one
function is what stops the two call sites from drifting apart again.

An unknown file extension and a language chunk that fails to load (offline, a broken deploy) are both
ordinary outcomes in `languages.js`, not errors: the file opens as plain text either way, because
losing syntax highlighting is not a reason to break the editor.

### The branch in the scope bar

The bar over everything names the active project and the branch it is on. The branch comes from
`src-tauri/src/git.rs` — one file, the same no-worker shape as `files/` and for a stronger version of
the same reason: `git rev-parse` would spawn a process for one line git already keeps in plain form
on disk. So `.git/HEAD` is read directly, and a `.git` that is a file rather than a directory is
followed to the linked worktree's own HEAD, which is where a worktree's branch actually lives.

Nothing in that file is an error. A folder outside git, an unreadable `.git`, a HEAD in an
unrecognised shape all mean the same thing to the bar — no branch to show, drawn as `—`. A detached
HEAD is not silently dressed up as a branch: `Head` keeps `branch` and `detached` apart, and
`DesktopApp.vue` labels the short hash as detached. Freshness is window focus and switching projects,
the same answer the file tree gives; `src/stores/git.js` guards against its own stale response the
way `terminals.js` does, so the bar cannot name one project's branch under another project's name.

The same file answers a second question, for the run dialog's branch field: `git_branches`, which
holds the no-spawn rule at the one place it is genuinely inconvenient. A branch list is not one line
the way `HEAD` is — it is `refs/heads` walked for loose refs, `packed-refs` for the ones git has
folded away, and the two reconciled — so this is three reads where `head` is one, and it is still
cheaper than a process. `by_recency` then orders them by each branch's own reflog under
`logs/refs/heads`, because the branch somebody wants to merge into is nearly always one they touched
recently and an alphabetical list buries it. Nothing here is an error either: a folder outside git
offers an empty list. The one name that is added whatever the refs say is the current branch — a
repository whose only branch has no commits yet has no ref file at all, and a merge target field
offering nothing would be worse than one offering the single branch that exists.

**Refs are shared and HEAD is per-worktree, and conflating the two is `smetana-5t7`.** A linked
worktree's git directory — whatever its `.git` file points at, `.git/worktrees/<name>` — holds only
the per-checkout half: `HEAD`, `ORIG_HEAD`, the index, `logs/HEAD`. Everything a branch list is made
of lives in the *common* directory instead, named by a `commondir` file sitting next to that git
directory, and `parse_commondir` resolves it — relative (git's usual `../..`, which from
`.git/worktrees/<name>` is `.git`) against the git directory rather than the checkout, absolute taken
as-is, and missing meaning an ordinary clone that *is* its own common directory. So `refs/heads/`,
`packed-refs` and `logs/refs/heads/` are all read from that one resolved place, while `HEAD` stays
where it is, because the branch this checkout is on is exactly the per-worktree fact. Before that,
opening a linked worktree as a project offered exactly one branch in the run dialog — the branch the
worktree was already on, which is the single branch nobody needs to merge into — and the reflog
ordering did not work at all, since the log directory was not there either and every branch fell into
the alphabetical tail. Live-checked against this repository's own linked worktree: the same list as
the main checkout, in the same reflog order, with HEAD still reading per-worktree.

The counters next to it — uncommitted files, running agents — are still fixture.

### The terminal: agent sessions

The centre's `terminal` tab (`chat` before it grew a terminal — `ProjectState::validate` in
`src-tauri/src/settings/model.rs` migrates the old name on load, because files on people's disks
already carry it, and without the substitution that tab would fail the closed-list check and silently
become the board) runs CLI coding agents — Claude Code and, eventually, others — under real PTYs, one
per session, listed in the sidebar's Agents view (`src/components/agent/AgentList.vue`) and started
from its "+ New agent" row — or from the task inspector's "Ask agent to edit", which starts one on a
particular issue. The reason the subsystem exists at all is the second half of that first sentence:
it notices when an agent is waiting on a human, including one in a tab nobody is currently looking
at.

An agent started for a piece of work opens on it. What `terminal_create` takes is not a prompt but an
`Intent` — file this task, edit that issue, or nothing at all — plus the id of the agent to run; the
words are the profile's business (see the agents section below), and `build_command` in `pty.rs` adds
only what every agent alike needs: the working directory, `TERM`, and the bundled `bd` on the front
of `PATH`. Whatever prompt the profile does produce rides as the agent's positional argument. Not as
bytes written after the spawn — the agent takes a moment to come up, and anything sent into an input
that is not reading yet is lost with no acknowledgement to wait for and no way to tell that it went.

That `PATH` line is load-bearing rather than tidy. Filing a task is the agent running `bd create`,
and this app's bd is a sidecar in the bundle: on a machine that never installed one there is nothing
on `PATH` to find, and the flow works in `npm run tauri dev` only because a development machine
happens to have bd of its own. `sidecar_dir` derives the directory the way tauri-plugin-shell does —
`dirname(current_exe())`, which is `smetana.app/Contents/MacOS` in a bundle and
`src-tauri/target/debug` under the dev command — so it is the same directory `app.shell().sidecar("bd")`
resolves to by construction. It goes in front of the inherited value, never behind it: the app pins a
bd version and checks it, and an agent that found some other bd first would be writing to the board
through a version that handshake never verified.

What that directory goes in *front of* is not the `PATH` this process inherited, and `src/shell_env.rs`
is why. A bundled app on macOS is handed launchd's environment: `open smetana.app` gives it whatever
`launchctl getenv PATH` says, which on a stock machine is nothing, so it falls back to
`/usr/bin:/bin:/usr/sbin:/sbin`. Everything a person installs — `~/.local/bin`, `/opt/homebrew/bin`,
nvm's shims — reaches `PATH` from `~/.zshrc` or `~/.zprofile`, which only a shell ever reads. So the
app asks a login shell once (`$SHELL -i -l -c`, the value fenced between markers because an
interactive rc file writes shell-integration escapes into the same stream), and that answer is what
both `agents::pick` and `build_command` work from — finding out whether an agent is installed and the
environment it is started with are the same question, and answering only the first would trade
"no agent is installed" for an agent that cannot find `git` or `node`. `-l` alone is not enough: the
machine this was written on adds cargo and the rest from `~/.zshrc`, which only `-i` reads. Every
failure — no shell, a five-second timeout, unrecognisable output — falls back to the inherited value,
which is where things were before the module existed. The bug is invisible in development, and that
is the whole reason it is a module rather than a line: `npm run tauri dev` starts the binary from a
terminal, so the process already has the full `PATH` and every lookup here is redundant.

| file | what it does |
|---|---|
| `model.rs` | `Session`, `SessionState`, `Question`, `TerminalError` — the vocabulary, and the pure rules for entering and leaving each state (`Session::apply`, `finish`) |
| `ring.rs` | the raw-byte scrollback ring, trimmed on overflow to a line boundary |
| `screen.rs` | a `vt100` grid built from the same bytes — the text a person would actually see |
| `detect.rs` | layer A: bell and silence, a pure function of the screen, the bell flag and the timings |
| `pty.rs` | the only file that touches the OS: spawns, reads, writes, resizes, kills; also assembles the child's environment |
| `service.rs` | the worker: one owner of mutable state, request queue, output and state events |
| `commands.rs` | thin `#[tauri::command]`s, shaped exactly like the tracker's |

`service.rs` is a single tokio task, the same shape as the tracker's worker and for the same reason:
commands, PTY output arriving from per-session reader threads, and a 16 ms flush tick all meet in one
`select!`, so nothing shares mutable state with it. A session starts at a fixed 120×30 before any
view has attached to it; the first `TerminalView.vue` that does replaces that with the pane's real
geometry through `terminal_resize`, which also feeds the new size into `screen.rs` — the app is
obliged to read the screen the size a person actually sees.

**One stream, two models.** Every chunk from a PTY goes into `ring.rs`, a raw byte buffer for the
human — this is exactly what xterm.js repaints itself from on attach — and, separately, into
`screen.rs`, a `vt100` grid for the app. The raw stream is cursor moves and repaints with nothing
findable in it; a `\r` overwriting "thinking..." with "done" is two writes in the ring and one line on
the screen. Detection reads the screen, never the ring. xterm.js in the front end is a third,
independent emulation fed the very same bytes on attach, so the person's picture and the app's
picture agree by construction, not because two implementations were kept in sync by hand.

`seq` plays the part `generation` plays for the tracker. Every flushed output event carries a
monotonic number; `terminal_attach` hands back the ring's snapshot plus the `seq` it should continue
from, and `terminals.js` re-attaches the moment an event arrives out of sequence, the same as the
tracker resyncing on a generation gap. Attaching clears whatever that session had queued to flush:
it is already in the ring snapshot just handed over, and sending it again would show it twice.

Output only flows to the front end for the **active** session — `flush()` drops a background
session's pending bytes on the floor every tick, because nobody is rendering them. **State flows for
every session, active or not** — `reassess()` walks all of them — and that asymmetry is the entire
point: a background agent's row can turn `needs-you` while its bytes never leave the worker.

Detection is two layers that degrade in one direction only. Layer A (`detect.rs`) is agent-independent
— a bell, or three seconds of stillness — and has nothing in it to break. Layer B is `Profile::question`,
so it lives with the agent it reads rather than in this subsystem: `agents/claude.rs` reads
Claude Code's own interface — a question line and numbered options — and a version bump to that CLI can
break it. It did, in exactly that way: the dialog was a box until 2.1, and the frame around it was what
told it apart from any numbered list in the agent's own output. Today it is fenced off by horizontal
rules and its lines are bare, so two other properties carry that weight — the options number themselves
1, 2, 3 … and the **last** such block on the screen is the dialog, since anything merely printed sits
above it; and exactly one option carries the cursor, which prose never does. The question is still the
run of text directly above the options, ending at a blank line or at the rule under a diff preview, and
it still has to end in a question mark. Layer B is also trusted only once the screen has held still
for `SETTLE` (150 ms), so a half-drawn dialog is never read as a truncated question. And `idle` is
deliberately quiet: a finished agent and a waiting agent both simply stop producing output, so layer A
must not guess between them — loudness comes only from the bell or from a layer B match, never from
silence alone.

That last rule is a rule **plus one named exception**, and the shape of the exception is the part
worth keeping. Claude Code's one-off folder-trust dialog — asked the first time it starts somewhere
new, which is a new project's very first agent and the worst possible moment to stall silently — lays
its text out differently: under the heading come the path, the question, a sentence about what the
agent will be able to do, and a link caption. It is the caption, "Security guide", that sits directly
above the options, so the ordinary reading declined it and the agent waited with nothing on screen to
say so (smetana-xh7). The fix opens a second, narrower reading **only after the generic one has
declined**, and only for a dialog that names itself: `const HEADINGS` is a literal table — one entry
today, `"Accessing workspace:"` — of strings such a dialog prints and ordinary output does not. Inside
it the search is fenced between that heading and the options, never above the heading and never in
what the agent itself wrote, and it takes the first paragraph carrying a question mark, cut at the
mark, because the trust dialog's question runs on past it into an aside and a piece of advice.

Both obvious shortcuts were refused, and each would have cost something real. Reaching past a blank
line for the nearest `?` — for the whole reader rather than under a heading — would drag a diff
preview and a title into the permission dialog's question, which is exactly what the blank-line and
rule boundaries exist to prevent. Dropping the question-mark requirement for everyone would leave the
cursor as the only guard against a numbered list in the agent's own prose, and a loud row is budgeted
at one or two a screen. Neither guard is relaxed: tests pin that a numbered list under that very
heading with nobody pointing at it is still refused, that an unheaded dialog still needs its question
mark, and that a question mark in the agent's output *above* the heading is not read as the dialog's.
A wording change on Claude Code's side loses the reading and leaves layer A in place, which is how
the rest of that file already fails.

The fixture behind it is a real trust dialog captured under a PTY off claude 2.1.226 and rendered
through `terminal/screen.rs` — and unlike the two permission fixtures beside it, it is the dialog
alone, because that is the whole of what was captured and an invented surrounding screen would prove
nothing. The live check turned up the one fact that makes the heading gate safe to lean on: the
dialog is drawn on an otherwise empty screen, so the heading is never pushed off the top of the grid
and out of the reader's reach.

**Quiet is measured on the screen, not on the byte stream**, and that is what `Quiet` in `detect.rs`
exists for. An agent that is waiting can still be talking: Claude Code 2.1 repaints an open permission
dialog about every 0.61 s for as long as it stands there, and while quiet meant "no bytes arrived",
every one of those chunks restarted the clock — so a session waiting on a human read as `Running` for
as long as it waited and `IDLE_AFTER` was simply unreachable (`smetana-8h7`). A repaint that draws the
same text changes nothing a person could act on, so what gets timed is the picture they see. The rule
cuts the other way too, deliberately: a session whose screen holds still for `IDLE_AFTER` is called
idle even while bytes pour in, which is the honest reading and cheap to be wrong about.

`Quiet` keeps a hash rather than the screen — this runs for every live session on every detection
tick, and holding the previous screen would mean copying kilobytes per session per tick. **The
fingerprint deliberately covers the plain text of the visible rows and nothing else**: no colour, no
bold or reverse, no cursor, because `Screen::lines` comes from `vt100::Screen::rows`, which writes
characters only. So an attribute-only repaint, or the cursor moving over unchanged text, counts as
stillness. That exclusion is the fix, and **feeding attributes into it would bring the bug straight
back** — an agent waiting on a person redraws its dialog to keep the highlight under the selected
option alive, which is a colour repaint of identical text, and the symptom of getting it wrong is
silence: a session needing a human that reads as busy with nothing anywhere to say so. Getting it
wrong the other way, for an agent whose spinner animates purely in colour, costs a dashed dot instead
of a spinning one.

**Half of `smetana-8h7` is fixed and half is not, and the difference matters when changing this.**
The silence half is closed **for repaints that redraw identical text — which is the mechanism the fix
assumes and not one that has been observed on the dialog it was aimed at**; the caveat below is part
of the claim, not a footnote to it. The bell half is not: Claude Code still rings none on a permission prompt,
so a bell is not the fallback here either. What an unmatched layer B now produces is `Idle` — which
reaches the front end as the `ready` status, whose loudness is `live`, the same as `running`. So the
whole visible cost of a waiting agent that no profile could read is the dot beside its row turning
from the spinning `loader-circle` into `circle-dashed`. **Nothing shouts, nothing dims, and nothing
else in the app acts on the state at all.** Layer A is still not a safety net that says a human is
needed — it never claims one is — and the two states that would cost something to get wrong are
untouched: `NeedsYou` comes only from a bell or from a profile's own match, never from silence of any
kind. One honest caveat on the fix: the premise that the permission dialog repaints *identical* text
is unverified. The live check could not reach a permission dialog without spending model quota, and
the trust dialog is no stand-in — it was measured emitting zero bytes after the first 0.6 s, so it
exercises nothing the old clock would have failed.

An agent that has genuinely finished still reaches `Idle` at about three seconds, but not to the
millisecond it did before, and the drift goes both ways. Earlier, because the last bytes a CLI writes
are often invisible ones — showing the cursor again, resetting the window title — which the old clock
counted and this one does not. Later, because the clock is stamped when the worker next looks rather
than when the screen actually changed, so it can lag by one detection interval (`REASSESS_EVERY` ×
`FLUSH`, ~64 ms today). Anyone lengthening that interval lengthens this error with it, and also
coarsens the resolution at which a change is seen at all.

`terminal_run_capture` — the call an automated flow uses to drive a session and read back its
settled screen — refuses with `busy` when the session is `needs-you`, and also when a bell is still
unrung even if state hasn't caught up yet (state lags the fact by up to `SETTLE` plus a tick; the bell
flag is that same fact arriving sooner). Writing into an open permission dialog would answer, on a
human's behalf, a question the app never read and the human never saw. **What that guard cannot
catch is the other half of `smetana-8h7`**: a dialog whose agent rang no bell and whose profile failed
to read it. Layer A now calls that session `Idle`, which is the truth and not a refusal — an idle
session is exactly what a capture expects to write into, so `Idle` can never join this guard without
breaking the ordinary case. Only the profile can tell the two apart, which is another way of saying
layer B is the whole of the protection here.

The capture's own settle is the one place the stream is still the right thing to measure, and it is
deliberately the opposite of what layer A now does: a capture has just written into the session and is
waiting for an answer to arrive at all, so a screen that happens to look unchanged mid-answer is not a
settled one. Reading a half-finished reply as finished would hand a caller the wrong text with nothing
to say so.

Sessions do not survive a restart, and nothing about them is written to `settings.json` — a session
row with a dead process behind it is worse than an empty list. `RunEvent::Exit` calls
`terminal::service::shutdown`, and the worker ends every session the way closing a terminal window
does: `SIGHUP` to the session's process group — which reaches whatever the agent itself started, as
`SIGKILL` to the direct child would not — then a short wait for them to go, then a kill for whatever
is left. The wait is bounded because the window is already closing. The two seconds `shutdown` itself
waits are a different thing again, and deliberately longer: they are the ceiling on a *wedged worker*,
the same one `settings.js` puts on its close-time flush, and for the same reason — the window always
closes, and a worker that never answers costs the cleanup, not the app. Anything that outruns all
that, or that the app never got a chance to signal, is an orphan in the process list.

`src/stores/terminals.js` is one of those files, and it keeps the
same cost-driven split as the worker: `sessions` and `agentRows` hold every session's state, cheap and
needed for a background row's colour; output bytes go only to the callbacks registered through
`subscribeOutput` — in practice the one live `TerminalView.vue`, and nothing else, because nothing
else renders them. That register is a `Set` and every subscriber gets every chunk: a single field
would tie unsubscribing to who mounted last, which is exactly the ordering the rest of this
subsystem refuses to depend on.

`activeId` looks like it names one thing and actually names two, and conflating them was a real
defect: "which agent the human has selected" has to survive leaving the terminal tab, because
`AgentList.vue` highlights its row from this same field; "which session the worker is currently
streaming output to" has to end the moment that view unmounts, or a background session keeps eating
flush cycles for nobody. While a single field served both, leaving the tab cleared the selection, and
the terminal came back permanently blank. `detach(id)` now takes the id it is leaving: switching
agents is two IPC calls — detach the old, attach the new — with no ordering guarantee at the worker,
so a nameless detach arriving after the new attach would silence the session the human just switched
to, with no error anywhere. `detach` never touches `activeId` — selection is not the transport's to
forget.

A session's row is captioned by the **work** it was started for, never by the process behind it, and
`SessionWork` in `terminal/model.rs` is what an `Intent` reduces to for that purpose — which of its
payload is drawn and which was only a briefing for the agent. `Intent::work()` lives in `agents/mod.rs`
rather than in `terminal::model` because it is knowledge about `Intent`, and the answer moves whenever
a variant does: a `NewTask` carries its prose, type and priority across for the draft panel to draw
and leaves its `images` and its Brainstorming, Spec and Plan switches behind, since those are
instructions to the agent and nothing on screen would show them.

`SessionWork::Run` carries nothing at all, and that absence is honest rather than lazy: **which issues
a batch has taken cannot be known here.** The agent claims one by running `bd update <id> --claim`
itself, which the app hears about only as the tracker changing under the watcher — there is no channel
that says "this session took this issue". So `claimedBy` in `terminals.js` reconstructs it from the
two halves already on the front end: the run knows which session is working, the tracker knows what is
`in_progress`. An explicit report from the agent would be steadier and needs the agent to send one;
until then this is the reconstruction, and it is written down as one. A run that has claimed nothing
yet reads exactly as a bare agent does, which is the truth and not a fallback — it is an agent, and
there is no work to name until it takes some.

`loadSessions` guards against its own stale response the same way `files.js`'s `stale` guards a
buffer: it can be called twice in flight with no ordering guarantee on which `invoke` resolves first,
and without the guard the *last response* would win rather than the *last call* — the list could end
up showing one project's sessions under another project's name, after which the remove button in
`AgentList.vue` would kill the wrong project's agent, silently. A test in `tests/stores/terminals.test.js`
pins this.

`TerminalView.vue`'s pane and its host both carry `minWidth: 0`, and that is not decoration next to
the `minHeight: 0` beside it. A flex item defaults to `min-width: auto` and so refuses to shrink below
its own content — here, xterm.js at whatever width it was last fitted to. Without it, narrowing the
centre column left the pane as wide as the terminal used to be, hanging over the task panel and
painted on top of it, since the pane is positioned and that column is not. It even looked animated,
because it converged: `ResizeObserver` → `fit()` → new cols → xterm redraws a frame later → the floor
drops a little → the observer fires again, with `fit()` measuring a pane sized by its own last answer
instead of by the column. `KanbanBoard` and `FileEditor` never showed it only because `overflow:
auto`/`hidden` zeroes that automatic minimum for them already.

`TerminalView.vue` hosts one `Terminal` instance per view, not per session — switching agents calls
`reset()` and refills from the new ring snapshot, so returning to an agent lands at the end of its
output rather than wherever it was scrolled to. An instance per session, the way `editor/states.js`
keeps one `EditorState` per file, would fix that; it is not built because the lack has not yet been
shown to matter. `AgentList.vue` reads `attentionLevel` the same as the board's status badges, but
draws it with a triangle for `needs-you` against a dot for everything else — colour is never the only
signal here either. That triangle is the *whole* of what the app says about a waiting agent, and
deliberately: `DesktopApp.vue`'s right panel used to draw the selected agent's question with a button
per option above the task card, and it was removed (smetana-s4f). It repeated what the terminal a few
centimetres away already showed, pushed the card the panel exists for down the column, and its option
labels — a permission dialog's are whole sentences — did not fit the panel's width. A person answers
in the terminal. The question still travels: `Session.question` is what layer B fills in and what puts
the session in `needs-you`, and `terminal_run_capture` still refuses to write into one. Nothing draws
it. `answer()` in `terminals.js` went with the block rather than being left as a write path nothing
calls — the same disposal `createIssue`/`tracker_create` got.

In a browser, `mockBackend.js` answers `terminal_list` with one fixture session already sitting in
`needs-you` with a real permission question attached — the only way `?view=gallery` and `npm run dev`
can show that state with no Rust worker behind them — and `terminal_attach` replays a canned
transcript. Every write (`terminal_create`, `terminal_remove`, `terminal_write`, `terminal_run_capture`)
falls through to the same loud rejection the tracker's writes get, for the same reason: a "write"
that looked like it worked would be worse than none. `terminals.js` translates `NoAgent` — the
refusal a machine with no agent installed gets — into its own message naming what was looked for,
rather than the generic "nothing was created": it is the one failure in that list a person can act
on, and since a task is now filed by an agent, it is the difference between a missing convenience and
no way to put a card on the board. The names in it come from the error's own text, because Rust holds
the only copy of that list.

### The agents: one intent, two harnesses

`src-tauri/src/agents/` is what the app knows about the CLI coding agents it runs, one file per
agent, and everything harness-specific lives in it. Claude Code and Codex are supported; which one
runs is the `agent` field in `settings.json`.

The split that makes this a module rather than a `match` in the terminal worker: **what the app wants
done is the same for every agent, and how it reaches one is not.** An `Intent` — `Bare` from the
"+ New agent" row, `NewTask` from the new-task dialog, `EditTask` from the inspector's "Ask agent to
edit", `Setup` from the dialog a person gets when they add a project, and `Run` for one batch of a
run — is where the product decision lives, and it is written once. `Run` is the only one no person
sends: `runs::service` builds it, and it carries the whole of what the run was asked to do rather
than a reference to it, because a session outlives a settings change and a batch that quietly
retargets halfway through is worse than one that is wrong from the start and says so.
`SkillDelivery` is how a skill
library reaches a particular harness, and there is no uniform answer: Claude Code takes
`--plugin-dir` and loads a plugin for one session, installing nothing (`PluginDir`); Codex has no
per-session mechanism at all — its own skills system reads `~/.codex/skills/`, and the only way to
add a root is a JSON-RPC method on the app-server, a different process from the TUI this app spawns —
so its skills ride as text in the prompt (`Inline`). Writing into someone's home directory or
repointing `CODEX_HOME` would reach into their own setup, and neither is done. Nothing about either
harness leaks into the code that decides what we want done: `prompt.rs` takes an `Intent` and a
`SkillDelivery` and is pure, which is where the tests are.

| file | what it does |
|---|---|
| `mod.rs` | `Profile`, `Intent`, `Stage`, `SkillDelivery`, `ImageDelivery`, `TaskDraft`, `Autonomy`, `Launch` — the vocabulary, the registry, `cascade` and `IDS` |
| `library.rs` | where the bundled skills are, whether the person already has their own superpowers, and reading a `SKILL.md` for inlining |
| `prompt.rs` | an intent becomes the text the agent opens on — pure; the skill text, where one is needed, is read by the caller and passed in |
| `claude.rs` | Claude Code: `--plugin-dir`, and layer B, its permission dialog read off the screen |
| `codex.rs` | Codex: `Inline`, `-i` for images, and its own layer B — the approval dialog read off a screen with no frame anywhere on it (smetana-603) |

**Codex has a layer B of its own now, and it is genuinely a different reader — not Claude's with the
glyphs swapped.** The two deliberately share no code, because a glyph one harness happens to use today
is exactly the kind of thing that drifts. Three properties of Codex's interface force the difference,
each measured off fixtures in `src-tauri/tests/fixtures/` captured under a PTY at 60 and 120 columns
from CLI 0.146.0 — the command dialog, the edit dialog, the trust prompt, the update prompt, a session
at work, and a draft left in the composer.

The cursor is `›` (U+203A), not Claude's `❯`, and Codex draws that same glyph in two other places: in
front of the person's own submitted prompt in the transcript, and in front of the placeholder in the
empty composer. So the marker has to be the **first non-blank character of the line** rather than
merely present in it — a label that happened to contain one would otherwise read as the selected
option and point a person at the wrong answer.

There is no frame at all, and **the only structural boundary is indentation**. Codex draws the first
row of every transcript entry hard against column 0 and indents everything continuing it, and
top-level blocks are separated by **two** blank rows where paragraphs inside one block are separated
by one. That single-versus-double distinction is the whole of what separates the second paragraph of
an agent's answer from a dialog drawn underneath it; both are prose indented by two under a bullet at
column 0.

The rule that took three versions to get right is what a block is **refused** for. It is refused for
what it hangs off — a turn of the conversation, meaning `•`, `◦` or `›` specifically — and not for how
closely it sits. Bounding the block by indentation alone held only while the turn above fitted on one
row and failed the moment it wrapped, which is the common case in a narrow centre pane. Refusing a
block only when it was glued to the turn above with no blank between missed whenever that turn's first
paragraph was a single row, since the row beneath is then the blank before its *second* paragraph and
the block looks unanchored. Distinguishing a conversational turn from any old column-0 line is also
what keeps the trust prompt readable, because that prompt hangs off `> You are in …` rather than off a
bullet.

Two other refinements come from real screens. A long label is gathered across the rows it wrapped
onto: the update prompt at 60 columns wraps its first option over three rows and numbers none of them,
and cutting at the pane's wrap column once left that option reading as a truncated fragment of the
shell command behind it. And a paragraph that is a question outright is preferred over one that merely
opens with a question mark, because in the command dialog the preview sits *below* the question — so a
command containing `x ? 1 : 2` would otherwise be handed back as what a person is being asked.

The known gap is recorded rather than papered over: **a scrolled screen with no anchor left on it**.
When the turn that owns the prose has scrolled off the top, the walk upward reaches row 0 having met
nothing, and indented prose ending in a question mark above a numbered draft still reads as a dialog;
a test pins the case by name. Closing it would mean requiring every block to be anchored, which would
then refuse Codex's update prompt — a real dialog drawn from row 0. That is a trade between a false
match in a rare scroll position and a miss in an ordinary one, with no measurement available to settle
it. Every rule here is written to fail closed for the same reason the design budgets loudness: a
session wrongly turned `needs-you` spends one of the one or two loud rows on the screen and makes
`terminal_run_capture` refuse a session with nothing open on it, so a change to that CLI should cost
a miss rather than a false alarm.

Three more methods on `Profile` are the same split one level down, and each one's **default is a
working answer rather than a gap** — that is the shape to keep when a fourth is added. `images` says
how pixels reach a harness: Codex takes `-i/--image`, Claude Code has no flag and simply opens a path
the prompt names, so the default is `InPrompt`, which is the one channel every CLI has. `usage_command`
and `parse_usage` are a pair — how to ask a harness what is left of the subscription's allowance and
how to read its answer — and a profile answering one without the other reads as unaskable, which the
run gate treats as no reason to hold anything up. `autonomy` is the extra arguments and environment
for working with nobody watching; the default is nothing, and a harness with no such switch therefore
stops at its first permission prompt and turns `needs-you`, which is exactly what `Supervised` already
is. A harness that cannot be autonomous is a fact about that harness, and the app says so by behaving
like the supervised mode instead of pretending otherwise.

`agents::IDS` is the single copy of the agent-id list. `settings/model.rs` validates against it
rather than repeating it, for the reason recorded above about the side-tab set being written out
twice: a value that survives the session and silently comes back as something else. The front end
never learns the names either — `settings.js` holds whatever string is in the file and passes it to
`terminal_create`, and Rust resolves it. A configured agent that is not on `PATH` falls back to the
first one that is, and `Session.agent` carries the name of what actually started. Nothing on screen
reads it: the agents panel captions a row by the work, and the one place the process name was drawn
— `claude-1 asks`, over the question block — went with that block (smetana-s4f), so today the
substitution happens silently and the only way to see it is the terminal itself. When nothing at all
is installed the session fails with `NoAgent` — a write failing loudly, which is the rule everywhere
else here.

Two directories under `src-tauri/resources/` are the library itself, both bundle resources.
`smetana/` is ours — seven skills now (`filing-a-task`, `provisioning`, `running-tasks`, `reviewing`,
`merging`, `live-checking`, `project-setup`), laid out as a plugin in its own right
(`.claude-plugin/plugin.json`, `skills/<name>/SKILL.md`) because that is what `--plugin-dir` accepts
and what makes them answer to `smetana:filing-a-task` and the rest. Three intents name one apiece —
filing names `filing-a-task`, setting a project up names `project-setup`, a run's batch names
`running-tasks` — and that last one is the process the remaining four hang off: an agent carrying out
a batch reaches `provisioning`, `reviewing`, `merging` and `live-checking` because `running-tasks`
sends it to them, not because the prompt lists them. Which is the point of a skill library over a
longer prompt — the prompt names an entry point and the library carries the depth.
`superpowers/` is a committed copy of that
plugin, 668 K of markdown under MIT, with its own `LICENSE` and a `SUPERPOWERS_VERSION` beside it
recording the version and commit sha, the way `BD_VERSION` does for the sidecar. Committing rather
than downloading is the opposite of the `bd` decision and for the reason that made that one go the
other way: 668 K of text is not 128 MB of binary, and committing makes the build hermetic.

The vendored copy is stripped of its `hooks/` directory, and that is the one exclusion that changes
behaviour rather than size. Superpowers ships a `SessionStart` hook that injects "you MUST invoke"
into every session the plugin is loaded into. Through `--plugin-dir` that would impose the process on
"+ New agent" and on editing an issue — the two intents this design deliberately leaves alone — and
would make the Brainstorming switch a lie in its Off position. A person who installed superpowers
themselves keeps their own hook: that is their choice, and our copy is never loaded for them.
`library.rs` decides that from `~/.claude/plugins/installed_plugins.json`, where a key is
`<plugin>@<marketplace>` and its value is the list of scoped installs — both halves matter, since a
key with an empty list is a plugin uninstalled everywhere. Anything unreadable answers "no", because
handing a second copy to someone who has one costs a duplicate line in a list while withholding it
from someone who has none removes the feature with nothing on screen to say so. When it is handed
over it keeps its own name rather than being folded into ours, which is what lets the prompt say
`superpowers:brainstorming` in both cases.

**Filing a task is an agent session, not a write.** `NewTaskModal` no longer emits an issue: its
fields become a `TaskDraft` inside a `NewTask` intent, and `DesktopApp.vue` switches to the agents
side tab and the terminal centre tab and calls `createSession`, exactly as "Ask agent to edit" does.
The agent runs `bd create` itself and the watcher puts the card on the board, so there is no plumbing
between the two — and `createIssue`, `tracker_create`, `NewIssue` and `create_args` are deleted
rather than left unused, because a live write path into the tracker that nothing calls is the kind of
thing that gets called again in six months.

The dialog collects one piece of prose, not a title and a description: the person writes what needs
doing in a single `Textarea`, and the title bd wants is written by the agent, which is the only party
that has read the text. Five `Dropdown`s sit under it in two rows, and every one of them defaults to
**Auto** — type, priority and Brainstorming, then Spec and Plan. For the first two, Auto travels as
`null`, never as the word, so `TaskDraft`'s
`Option<String>`/`Option<u8>` cannot carry a type bd would reject; `prompt.rs` then names the pinned
fields as settled and hands the rest to the agent *by name* ("Decide the priority yourself"), because
an agent simply told nothing about a field would have to invent one anyway and would not know that
inventing it was its job rather than a gap in the briefing.

Brainstorming's three positions: `Off` files it now; `On` requires a discussion first; `Auto` states
the test the agent applies and leaves the judgement to it, deliberately, since nothing in the app has
read the text of the task and a heuristic on its length would misfire in both directions. How to
file one *properly* is not
part of that question — an agent that files without discussing still has to file it well — so the
filing skill reaches the agent in all three positions, by name for `PluginDir` and as text for
`Inline`. `Auto` differs from `On` only in what it hands over for the brainstorming process: a name
for `PluginDir`, which is already loaded and costs one index line, and the absolute path to the
vendored `SKILL.md` for `Inline`, so a one-line change does not pay for 10 KB it will not use.

**Spec and Plan hang off it, and they cascade rather than sitting beside it.** They are the two
stages the filing session used to stop short of: writing down the design the discussion produced, and
writing the implementation plan (`superpowers:writing-plans`). Spec is a person's to choose only
while Brainstorming is `On`, and Plan only while Spec is — there is nothing for a design document to
record when no discussion happened, and nothing for a plan to plan when no design was written. A
stage nobody may touch **reads as its parent rather than as a placeholder**: under an `Off` it shows
Off, under an `Auto` it shows Auto, so the screen states exactly what will be sent. The rule is
`components/kanban/taskStages.js`, another of the `branchChoice.js` family, and `agents::cascade`
applies the same rule again on the far side of the wire — not a duplicate to tidy away, since what
arrives there is a payload and a payload can carry a spec chosen under a discussion since switched
off. `prompt.rs` normalises before it writes any prose, so such a spec produces no words about a
spec at all.

The output is files, and the task is filed **last**: the design goes to
`.smetana/docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md` and the plan to
`.smetana/docs/plans/YYYY-MM-DD-<topic>.md` — superpowers' own layout moved under the folder
`runs/gitignore.rs` keeps out of the repository, so nothing is committed and the prompt says so.
Filing last means an interrupted session leaves no card promising documents nobody wrote. The paths
copied into the issue are **absolute**, for the reason `IMAGES` already gives plus one of its own:
an ignored file does not travel into the worktree `provisioning` cuts, so a relative path resolves
from nowhere an implementer actually stands — and the issue still has to say in prose what was
decided, because the files are on one machine. Spec needs no skill text of its own (the design
document is part of the brainstorming process, which is already named or already pasted whenever
Spec is reachable at all); Plan is its own skill and follows exactly the trade Brainstorming's own
`Auto` makes — the name for `PluginDir`, the body for `Inline` on `On`, the path alone on `Auto`.

One `Stage` covers all three switches, matching `STAGES` on the front end, and the collapse was the
point rather than a tidy-up. While Brainstorming had an enum of its own, only one of the two
directions was loud: a fourth position added to `Brainstorm` broke the `From` impl's match at compile
time, but the same position added to `Stage` alone compiled perfectly and left the discussion switch
a position short of its children.

**What a filed task owes is set by the far end of the app, not by the dialog.** `provisioning` says
the description *is* the spec and a description that never says what "done" looks like is not
something to start on — so a thin task is not a smaller task, it is a supervised run stopping
overnight on a question or an automatic one parking the work. The two ends used to disagree, since
the filing skill asked for a paragraph of prose and nothing more. They are held together now by
`bd create --validate`, which refuses a description missing the sections its type requires
(`## Acceptance Criteria`, plus `## Steps to Reproduce` on a bug, `## Success Criteria` on an epic,
three headings on a decision, and nothing at all on a chore). That flag is the whole mechanical part
of the standard, which is why `STANDARD` in `prompt.rs` names it in the prompt rather than leaving it
to the skill: an `Inline` harness may find no skill text to read, and prose can be skimmed where a
refusal cannot. **It is a floor and not the standard**, and the skill says so in as many words —
measured against the pinned sidecar, it matches the wording of a heading and nothing else, so an
empty section, a `###` and lower case all pass. It converts "no acceptance criteria" from an
invisible default into something somebody has to do on purpose; judging whether the criteria are
real is `provisioning`'s job at the other end. `running-tasks` holds its own filing — the depth-budget one — to the same skill and
adds the test that follows from it: a finding nobody can state acceptance criteria for is a digest
line, not a task.

The other half is what the discussion produces. Brainstorming on `On` buys half an hour of narrowing
down what somebody meant, and none of it is anywhere but that conversation — the session ends, and the
agent that picks the task up months later has the person's original four sentences and nothing else.
So `DISCUSS` requires the outcome, rejected options included, to be written into the issue itself.

### Attachments: pictures on a task nobody has filed yet

A screenshot is the fastest way to say what is wrong, so the new-task dialog takes images:
`src-tauri/src/attachments.rs`, `src/stores/attachments.js` and
`components/kanban/AttachmentStrip.vue`. The Rust side is the same no-worker shape as `files/` and
`git.rs`, for the same reason — writing a couple of megabytes guards no state — and it is two
commands over pure functions that carry the tests.

Three gestures put a picture in the list and they arrive as only two kinds of thing. A file already
on disk arrives as a path and Rust copies it (`attachment_import`); the clipboard exists inside the
page and nowhere this process can reach it, so a paste arrives as bytes and travels down
(`attachment_write`). Both answer with the same record, which is what leaves the strip one shape to
draw. The list lives in the store rather than in the dialog because a drop is not the dialog's event
to hear: Tauri intercepts file drops before the webview sees them and reports them against the
*window*, so while the dialog is open the whole window is the drop target.

**The bytes are copied, never pointed at.** They go into `app_data_dir()` and the path that reaches
the agent is absolute. The case this exists for is a screenshot in `~/Downloads` that a person throws
away in a week, and the link in the issue is obliged to outlive that. Writing into the repository
instead — `.smetana/attachments/` and a relative path — was considered and refused: it would work in
every clone and every worktree, but only for files somebody committed, and committing binaries into
another person's tree is not this app's decision. The price is plain and worth knowing: in somebody
else's clone, and in CI, the pictures are not there.

There is no `resolve_within` here, and its absence is the design rather than an oversight. `files/fs.rs`
confines every path to the project root because everything it touches belongs to the project; nothing
here does. The *source* is whatever a person picked in the OS's own dialog or dragged off their
desktop, and a folder outside the project is the ordinary case rather than the attack. What is
confined is the *destination*: always `app_data_dir()/attachments`, under a name that is not the one
that arrived. `stored_name` builds it from a timestamp and a `slug` keeping ASCII letters and digits
and nothing else, so no incoming name can climb a directory, hide behind a dot or need quoting — that
string ends up in a prompt, in a shell argument and in an issue description, and the one that
eventually gets pasted without quotes is the reason none of it survives. The extension comes from
`sniff`ing the bytes, not from the name, so a JPEG somebody renamed `.png` reaches the agent labelled
with what it is.

Two numbers are deliberately not shared. `MAX_IMAGE_BYTES` is 8 MiB and is **not**
`files::model::MAX_FILE_BYTES`: that one is 2 MiB and answers how much text a textarea will open
without freezing the window, while this one answers how big a screenshot is, and a full-screen retina
PNG — the very gesture this feature exists for — routinely lands between the two. A test asserts they
are still different, so that wiring them together is a thing somebody has to do on purpose. The other
is the copy of that ceiling in `attachments.js`, which exists only so a file certain to be refused is
not first read into an ArrayBuffer and encoded a third larger again. Drift there is not symmetrical:
above Rust's is harmless, below Rust's makes every file between the two impossible to attach at all
by a refusal Rust would never have sent. The front end's copy must never be the smaller.

Nothing here ever deletes. Taking a thumbnail out of the dialog forgets the path and leaves the file;
tidying the store is deliberately outside this work, so the directory grows.

### Runs: a batch of the board, carried out by sessions

A *run* is the app driving itself — read the board, start an agent session on a batch of it, wait for
that session to end, read the board again — and it is `src-tauri/src/runs/` plus `src/stores/runs.js`
plus `src/components/run/`. It sits on top of the other two workers rather than beside them: it owns
no board and no PTY, and `lib.rs` hands it clones of both handles so it queues behind them like every
other caller.

| file | what it does |
|---|---|
| `model.rs` | `Run`, `RunSettings`, `RunScope`, `RunMode`, `RunState`, `StopReason`, `RunError` — the vocabulary, and the settings rules that are not the dialog's to keep |
| `config.rs` | `.smetana/project.toml`: the shape of the project a run works in |
| `survey.rs` | what a project looks like from outside, before anyone has configured it |
| `gitignore.rs` | keeping `.smetana/` out of the repository |
| `preflight.rs` | bringing the project up before the first batch — declared commands, then declared health checks |
| `usage.rs` | what the subscription has left, and whether to run at full size, a smaller one, or not yet |
| `browser.rs` | whether there is anything on this machine to drive a browser with — pure over file contents and directory listings, and where those tests are |
| `queue.rs` | what is left to do and whether to run another batch — pure, and where the tests are |
| `service.rs` | the worker: the loop, one run per scope per project |
| `commands.rs` | thin `#[tauri::command]`s, shaped exactly like the tracker's |

`service.rs` is the same single-tokio-task shape as the other two workers. The deciding is
`queue.rs` and that is pure; the map's own lifecycle — `absorb`, `permit`, `admit` and the
browser-candidate list — is pure too, and unlike the other workers this file carries a test module
of its own at the bottom for exactly that part, because both ways of getting the lifecycle wrong
are silent. A project holds several runs at once, and the map is keyed by each run's own
`token` (smetana-5hf): what is refused is a second run over the **same scope** — two runs both told
to take the whole queue are two leads racing for the same tasks, and the refusal names the scope it
found in the way — while a queue run beside a task run, or two runs over different epics, divide the
board between them. Which tasks each may touch is not this worker's question: bd's atomic claim
under per-session actors (smetana-4fh) is the exclusivity, and a second mechanism here could only
disagree with it. A run in another project is none of this one's business, since different projects
are different folders, boards and target branches. The one thing all runs share is a subscription
limit, and a run does not reserve one (smetana-tra). The loop runs on a task of its own so the
worker stays answerable while a batch runs for an hour, and it reports whole `Run` values back
through a channel — the worker is the only thing that ever writes one out. The `token` is on the
`Run` itself and does the job `generation` does for the tracker: a stop names one run by it, every
`run:state` event carries it, and a late report from an ended run finds no entry rather than the run
that started after it.

**Stopping is cooperative, and that is a decision with a cost attached.** `request_stop` sets a flag
and the loop reads it between batches; the batch in flight is allowed to finish. A run interrupted
between a merge and a close is exactly the state the recovery phase exists to clean up, and killing a
session mid-merge is how you get there deliberately. A run with nothing in flight stops at once,
which is also what lets the stop button reach a paused one. `StopReason` keeps `Cancelled` and
`SessionRemoved` apart for the same kind of reason: both are somebody's doing and neither is a crash,
but pressing stop let the batch finish while removing the session killed it where it stood, and the
person reading the bar is deciding whether to go and look at what got left behind.

**A map entry outlives the run it holds, and that is what makes "one run per scope" true**
(smetana-0kb). It leaves in exactly one place — `Report::Ended`, sent by a `Drop` guard when the loop
task is gone however it went — so "there is an entry" and "a loop task is alive" are one fact rather
than two that agree most of the time. Removing the entry the moment a stop declared the run over
looked equivalent and was not: the loop was still between reading the board and spawning, so it put
a batch out that nothing could then stop, and the scope was free to start a second run beside it.
The spawn itself is **asked for rather than checked**: `may_spawn` puts the question on the channel
the worker's own `select!` already drains, so the same single task decides it and handles
`Request::Stop`. That is the whole guarantee, and it is not a FIFO one — the two arrive on different
channels and a `select!` with both ready picks at random. What it buys is that the two can never
interleave, so **both orderings are safe and each has its own honest outcome**: stop first and the
spawn is refused, spawn first and the stop that follows finds a batch in flight and waits for it.
Yes records that batch as in flight (`Active.starting`, the fact `Run.session` cannot carry yet). A
second read of the stop channel just before spawning would only have narrowed the window, since
nothing orders a stop in another task against the microseconds after a check.

A stop leaves a gap between the run reading `Stopped` and its entry leaving, and the **refusal in
that gap has its own reason**, `RunError::WindingDown`. Reusing `AlreadyRunning` there put two
contradictory things on screen at once — a bar saying the run is stopped and a message saying one is
going — and a person reads that as the stop not having taken. The gap is not always brief: the loop
may be inside a board read or a 60s usage probe, and it holds its scope for the whole of it — only
its scope, since the rest of the project's runs were never this one's to hold.

**The preflight is the one phase where a stop is not cooperative** (smetana-16w), and that exception
is the reason the gap is no longer measured in minutes. `bring_up` read the stop channel nowhere at
all, so a stop pressed during it waited out every declared command at 600s apiece and every health
check at 120s — on this project the first declared command is `npm install`. It now watches that
channel: the command in flight is killed where it stands, and a check is given up between looks
rather than during one, since a look is bounded by seconds of its own (`curl --max-time 5`, a
two-second connect) where a command has nothing bounding it but the ceiling. Killing is safe here for
exactly the reason it is refused between batches: a declared command brings infrastructure up and is
run again from the top next time, where a session interrupted between a merge and a close leaves work
for the recovery phase. The signal goes to the process group, the way `terminal/pty.rs` sends its
`SIGHUP`, because the child is a shell and the work is what it started — `npm install` is node and
everything node forks. Nothing about the ending changes: the run reaches `Stopped { Cancelled }` and
its entry leaves the map the one way every ending does, when the loop task is gone.

Two smaller rules hold that up, and both were found by driving the race rather than by reading it.
`may_start_batch` refuses a run that is merely `stopping`, not only one already over — "the batch in
flight finishes" has always meant that one and no more, and a stop landing just after the loop's own
check would otherwise start a whole further round, board read and all. And a report from the loop is
**adopted, not assigned** (`Run::adopt`): stop is asked for on the worker's side and never travels to
the loop task, so the loop's copy says `stopping: false` for the rest of its life, and taking it
wholesale unasked the stop a moment before the check that reads it.

`queue.rs` is a port of `holiday-curb`'s `loop-state.mjs` with one substitution that changes its cost
and not its logic: the source shelled out to `bd ready` and `bd list` between every batch, about four
seconds each, while this reads the snapshot the tracker worker already keeps current from its
watcher. It tracks `unfinished` — `in_progress` and `ready_to_merge` — separately from `ready`,
because `bd ready` hides both and a run watching only the ready set would leave a killed batch's
orphans on the board forever. A dependency counts as blocking only when it is bd's `blocks` kind;
`parent-child`, `related` and `discovered-from` do not mean "wait". And `LastBatch` has three answers
rather than "did it crash", because a batch stopped by a spent allowance moved the board no more than
a crashed one did — reading either as a stuck queue would end a run over nothing — while a harness
that keeps falling over needs a person and an exhausted allowance needs only time.

`usage.rs` is the piece the runs design deliberately left out and then took back, and the reason is
worth keeping. Reading `claude -p "/usage"` is a parse of somebody else's prose that can break
silently, which is why it was refused; what did not survive contact was the trade. A run that
exhausts its allowance overnight spends five sessions and a minute of backoff discovering it and then
stops with `Crashed` — which says the harness kept failing, when nothing failed and the work was
never stuck. So the parse is back with its failure mode named rather than assumed: **an unreadable
answer never blocks a run.** It reads as `Normal`, the batch goes at full size, and that is exactly
where things were before the module existed — the same shape layer B keeps in `agents/claude.rs`.
The gate runs *before* each batch, which is the whole of why it is worth having: an allowance is
checked before it is spent, so the exhausted case costs no session at all. `service.rs` asks the same
question a second time after a session exits non-zero, and there it is not a gate but a
classification — telling a spent limit apart from a harness that fell over, from the one source of
truth, with no second mechanism to keep in step.

`browser.rs` answers the question the config could not: `[live_check].mode = "browser"` says what the
*project* wants and nothing about the machine the run rides on, so a run with the live check on
started happily where there was nothing to drive a browser with and found out inside the check, as
INFRA (smetana-29s). Either tool is enough — Playwright, which is two facts and not one (an MCP entry
in `~/.claude.json`, the project's `.mcp.json` or `~/.codex/config.toml`, **and** the browsers
actually downloaded under `ms-playwright`), or the Claude in Chrome extension, found by its id in a
Chrome profile. Every path and id in it is fragile by nature and that is accepted rather than hidden:
an extension writes itself into no agent's configuration, so from outside the unpacked directory is
the only evidence there is. Hence the rule the whole file is built on — **anything unobservable reads
as "no", loudly**, the toggle goes off and the tooltip names what was not found, rather than the
toggle staying live on a guess. Matching an MCP entry goes the other way on purpose (its name *or*
what it runs, either alone) because the two mistakes are not the same size: a false "present" leaves
things exactly where they were before the module existed, while a false "absent" takes a working
feature away under a tooltip claiming a tool is missing that is sitting right there.

Busy-ness is the second reason and it is deliberately only half a question. `Request::BrowserBusy`
answers which projects have a live run that asked for a live check — counted per run, and the asking
project among them, since a live-check run in this very project is exactly what holds Playwright's
one profile against a second run beside it now that a project holds several — and `browser_tools`
then reads each candidate's config, because the worker knows a run wanted a check and not whether
that project's check opens a browser — naming a `command` check as the reason this toggle is blocked
would be an invention. **The extension's busy-ness is out of reach entirely, and so is a browser a person is
driving themselves**: neither is visible from this process, and that gap is written down rather than
papered over. The sentence a person reads is composed on the front end
(`components/run/browserTools.js`, pure and tested, one of the `branchChoice.js` family),
since it is UI copy; the scope is `browser` and nothing else, because a `command` check needs no
browser and `none` is `liveCheckAvailable`'s own reason with its own words under the switch.

Busy-ness may block **only where Playwright is the tool that would be used**, which means the
extension is absent. It is a Playwright fact and nothing else: the app sees its own runs, and a
Playwright run in another project genuinely holds the one persistent profile, while a Chrome window
holding the extension is not something this process can observe at all. Letting the busy branch fire
whenever *either* tool was present disabled the toggle on an extension-only machine over a tool
nobody had shown to be held — guessing about precisely the half the module has already said it
cannot know.

A pause is a `RunState`, not a `sleep` inside the loop, and that is load-bearing twice over: a run
that had simply gone quiet for three hours is indistinguishable from one that hung, and the bar is
where somebody looks to tell those apart — and being a state is what lets the stop button reach it,
since a paused run has no session in flight and therefore stops the moment it is asked. `resets` is
the harness's own sentence about when the allowance clears ("Aug 11 at 5:59pm (Europe/Moscow)"),
passed through untouched and never turned into a moment in time: that would be a second parse of the
same prose, and its failure would be a run that woke at the wrong hour.

`config.rs` refuses to load a damaged file, which is the **opposite** of `settings/model.rs`, and
opposite for the right reason. There, a broken section loses itself and the app carries on, and the
cost of that leniency is a forgotten panel width. Here the cost would be a run whose gates quietly
went missing and whose green merges therefore proved nothing — hence `deny_unknown_fields`
throughout: a typo has to be louder than a silence. `runs::service` is the first and only place a
damaged config is ever shown to anybody; everywhere else in the app it reads as "no configuration",
which is right for a marker on a row and wrong for starting a run. The file is declarative where the
work is mechanical and prose where it needs judgement — `hazards` stays as text the lead reads,
because two branches emitting the same migration number off one base is not a pattern, it is a thing
to look for.

`gitignore.rs` keeps `.smetana/` out of the repository, and it is code rather than a line in the
setup skill on purpose: an instruction in prose can be followed, argued with or quietly skipped, and
this one was all three. An agent reading a `.gitignore` whose neighbouring lines hide the tracker and
the docs will reasonably conclude the folder belongs there too, or reasonably conclude the opposite,
and the answer then differs from project to project. The app decides once, in code. `amend` is pure
and carries the tests; it treats `.smetana`, `.smetana/`, `/.smetana` and even the negation
`!.smetana` as already covered, that last one because it can only have been typed on purpose.

On the front end, `runs.js` is deliberately small — a file read with no worker behind it, freshness
from switching projects and from a setup session finishing. It keeps the back end's `config` and
`Run` objects **whole** rather than unpacking them into flags, which is the same instinct
`tracker.js` follows with statuses: a state this front end has not heard of must not silently read as
one it has. The runs ride as a set keyed by `token` — events and stop answers land by `upsert`, so a
late word about one run can never write over another. It is guarded against its own stale response
exactly as `git.js` and `terminals.js` are, and the `run:state` listener carries that guard in its
other form — an event is not a response to anything, so nothing orders it against a project switch,
and a batch ending just as somebody moves project would otherwise post its run under the new
project's name. `RunBar` draws one segment per run in the scope bar, each stop button naming its own
token, and keeps a stopped run there until the project changes or a run of the same scope replaces
it: the reason it stopped is what somebody came back to read, an unknown reason is an ordinary
outcome rather than a crash (this front end may be older than the worker), and the endings differ by
glyph as well as by colour, the rule the status palette keeps everywhere else. The scope rule itself
— what "the same run" means, and the words a greyed play carries — is `components/run/runScopes.js`,
one of the `branchChoice.js` family and shared with the worker's `admit` by vocabulary rather than
by code.

### Panel widths

Either side column is dragged by the `Resizer` between it and the board, and the rules for how wide
it may get live in `src/views/panelWidths.js` — pure, no Vue and no DOM, which is what makes them the
one part of this that a test can reach at all. A panel takes at most a third of the window and never
so much that the board drops below `CENTER_MIN`; the neighbour is part of that sum, costing its own
width open and a rail collapsed.

The stored width and the drawn width are different numbers, and conflating them would be the defect
here. What `settings.json` keeps is what a person dragged to; what `leftStyle` draws is that number
clamped against the window it is in now. Only a drag writes back — narrowing the window squeezes the
panel and widening it restores what was asked for, because a resized window must not silently rewrite
a preference. Every delta a `Resizer` emits is likewise measured from a width snapshotted at
`dragstart`, not from the previous frame: clamping against the last frame would make each clamped
move the new origin and the panel would drift away from the pointer.

Dragging a panel past `COLLAPSE_SLACK` below its minimum folds it into the same 32px rail the header
button gives, keeping the stored width so it comes back where it left; pulling out of the rail past
`EXPAND_PULL` reopens it. Double click resets to the shipped 252/340. When the window is too narrow to
honour both a panel's minimum and the board's floor, the panel keeps its minimum and the board takes
the squeeze — the board's content scrolls, a file tree at 90px does not.

`Resizer` itself diverges from the design system in behaviour, not in styling — pointer capture so a
release outside the window still ends the drag, `user-select: none` on the body for the duration, and
arrow keys, which its `role="separator"` and `tabindex` had been promising with nothing behind them.
Those belong back upstream.

### The column order

`components/kanban/columnOrder.js` is the same split one screen over, and it says so: bd owns which
columns exist, the settings own only the sequence, and this file is the reconciliation between them —
pure, no Vue and no DOM, which is again what makes it the one part of the reordering a test can reach.
The stored order is per project, because the set of statuses is: bd carries custom ones and one
repository's status has no meaning in another's order.

A stored order is a **hint, never the truth**. A status bd no longer has cannot be conjured onto the
board by a line in a settings file, and a status bd grew since the last visit has to appear even
though nothing stored names it. So columns the stored order knows are drawn in its sequence and the
rest go after them in bd's own order — appended rather than dropped, because a column nobody has
arranged yet still holds issues, and appended rather than slotted back into bd's position, because
there is no honest position to slot it into once the neighbours have been moved by hand. Names in the
stored order matching nothing are passed over rather than pruned, so a custom status deleted and
recreated, or a project reopened, finds the place it was left in.

`moveColumn` returns the very array it was given, by reference, when nothing moved — an out-of-range
index, or a move to where the column already is. The caller leans on that identity to tell "nothing
happened" from "something did" without comparing contents.

`components/run/branchChoice.js` is the next of that family and was pulled out for the same reason:
a `.vue` file is the one thing no test in this repository can reach, so the whole of the rule filling
the run dialog's branch field lives outside the component. `pickBranch` is three steps in one order —
what this project was left at last time, then its own `[defaults].target_branch`, then whatever the
list puts first, which is the most recently worked-on branch because `git_branches` orders by reflog.
A remembered name that is no longer in the list is skipped in silence rather than offered, since a
branch deleted since it was remembered would sit in the field as an option that fails on the first
merge.

The defect it was written for was not the rule being wrong but the rule running **once**, against a
list that had not arrived yet (smetana-6gs, smetana-o8r): the dialog is shown first and the branches
are fetched afterwards, so the fill on opening ran against nothing and the field opened on "Pick a
branch" with Run disabled, which left the remembered branch, the config default and the fall-back to
the most recent branch all dead at once. A watcher now refills when the list lands — **but only while
nobody has chosen**. That is what `branchChosen` guards, and it is why the control is deliberately not
on `v-model`: through `v-model` a fill and a person's pick are the same assignment and nothing
downstream could tell them apart, so a late answer would overwrite a choice somebody had already made.

The other half of that fix is in `git.js`, and it is a trade taken with its eyes open. `loadBranches`
clears the list when it belongs to *another* project — offering the branches of a repository somebody
has already left is worse than offering none — but leaves **this** project's list in place while it
reads it again, because clearing unconditionally emptied the field under the dialog that had just
opened. The cost is that for the length of one call the field can be filled from a list one read out
of date: a branch deleted since the last open is still on offer in that window, and somebody picking
it inside that window has the choice frozen by `branchChosen`, so the run goes out against a branch
that is not there. Clearing first made that impossible — by keeping Run disabled every time, for
everybody. The window is a single call, the field self-corrects when the list lands as long as nobody
has chosen, and the bad case costs a run that fails at the merge.

The family has more members than this section names, and the place to see them all is `tests/` — for
the reason the note under Commands gives, a list written out here is wrong by the time somebody
trusts it. What they have in common is the shape rather than the count: each is the whole of one
rule, pure, with no Vue and no DOM in it, and each lives under the directory of the part of the
interface it is a rule about.

`src/paths.js` is the one that breaks the second half of that, and it is worth saying why, because
its location is the one thing about it a reader cannot work out for themselves. `basename` — what a
path is called, for a project row, a file tab, a dialog's sentence — is not a rule about any one part
of the interface: two stores and a component module want it at once, so there is no "under" to put it
and it sits at the top of `src/` instead. It had been written out three times over, and the three
disagreed — the newest answered `''` for a root path where the other two answer the path itself,
which would have drawn an empty gap in the middle of a tooltip's sentence. That one was caught in
review rather than on screen, which is luck and not a process. Borrowing a store's copy instead of
lifting it out would have pulled Vue and Tauri into a family defined by having neither.

### Settings

What the app remembers between runs lives in one JSON file in `app_config_dir()`
(`~/Library/Application Support/com.invisor.smetana/settings.json` on macOS).
`src-tauri/src/settings/` owns it: `model.rs` is the schema, the validation and the merge — pure,
and where the tests are; `file.rs` is the disk (atomic write through a per-call temp file that is
`sync_all`ed and renamed, a `.bak` copy of anything unparseable or too new); `commands.rs` is two
thin commands.

The file keeps appearance, panel layout — collapsed state and width for each side — and `agent`, the
id of the CLI agent to start, at the root;
below that, `openProjects` is the list of
projects the window has open, `lastProject` is the one active when it last closed, and `projects` is
a map from each project's absolute path to its content state (side tab, active tab, selected task,
selected path, expanded folders, `openTabs`, `previewTab`, `columnOrder`, `runSettings`, `usedAt`).
The last two are per project for the same reason the rest are: a status has no meaning in another
repository's column order and a branch name has none in another repository. `runSettings` is what the
run dialog opens on next time and is a mirror of `runs::model::RunSettings` **minus the scope** —
deliberately its own type rather than a reuse, since this one lives in a file people edit by hand and
has to tolerate anything while the other crosses the IPC boundary and must not, and deliberately
without the scope, since that comes from whichever play button was pressed and remembering it would
open the dialog claiming to run something nobody clicked. The open tabs are paths
relative to the project root — the key already carries the absolute part, and a moved folder does
not turn the list into rubbish. The map never crosses the IPC boundary: `settings_load` returns the
resolved view for one project (`{ appearance, layout, agent, project, openProjects, activeProject }`) and
`settings_save` puts it back, stamps `usedAt` on the active project and trims `projects` toward the
20 most recently used — but never evicts the current project or anything still in `openProjects`, so
the cap only bites entries from past visits that were closed, not projects a person still has open.

The front end owns the truth here — the opposite of the tracker, where bd owns it.
`src/stores/settings.js` holds a reactive object and writes it back with a 400 ms debounce, one
write in flight at a time; components read and write plain fields. Closing the window does not wait
for the debounce: the store holds the close through `onCloseRequested`, flushes with a two-second
ceiling and then destroys the window itself — the window always closes, a slow back end costs the
last edit rather than the app.

There is no settings screen and no theme switch: appearance and layout are only ever changed by
using the app, and `agent` is not changed by using it at all — until there is a screen, switching to
Codex is a hand edit of the file, which is fine and changes the shape of nothing. The one part of
`settings.json` the interface does edit directly is the project list —
adding, switching and removing rows is what writes `openProjects` and `lastProject`.
`?theme=` and `?density=` still override both for one run and are deliberately **not**
written back — one visit to the dev server must not repaint the app forever. `?view=gallery`
neither reads nor writes.

A missing file is the first run, not an error. A broken or too-new file is copied to
`settings.json.bak` and the app starts from defaults, and saving over it afterwards is fine. One
that cannot be read at all — wrong permissions, a directory in its place — has nothing to copy, so
it is logged *and* `settings_save` refuses: overwriting a file nobody could read would destroy it
sight unseen. Damage is contained field by field where it can be: a single field whose *value* is
outside its allowed set loses that field, while a section whose *type* is wrong
(`{"layout": {"leftCollapsed": "yes"}}`) fails to deserialize and loses the whole section to its
defaults — the same holds for one project entry among many. A file written before the list existed
carries `lastProject` and no `openProjects`; reading it makes the list that one project, so an update
does not open to an empty panel. That leniency lives on the file-reading side only — an empty list
coming from the front end means the last project was closed on purpose and stays empty.

The side-tab set is a closed list written out twice, in `model.rs` and in `views/DesktopApp.vue`.
Changing one without the other is silent: the value survives the session and comes back as Files.

Window size and position are not in this file: `tauri-plugin-window-state` handles them, and
`src-tauri/src/window.rs` is the one thing added on top. The plugin keeps geometry in memory and
writes it to disk in exactly one place — `RunEvent::Exit` — so any run that does not reach a clean
exit leaves the last run's geometry behind: a crash, a force quit, and in development every Rust
rebuild, which kills the process outright. The symptom is a window that opens at the configured
1440×900 no matter what size it was left at, and it is invisible from the front end, since
`settings.json` keeps saving on its own debounce the whole time. So `persist_geometry` subscribes to
`Resized`/`Moved` and saves 500 ms after the last one. The debounce is not only about disk traffic:
it also settles the question of handler order, because the plugin's own listener has long since
updated the cache by the time the write runs.

### Tests

`tests/` mirrors `src/`, and `vitest.config.js` merges the app's Vite config so the alias and the
Vue plugin come along. Two decisions are load-bearing, and both are explained where they live.

The mock boundary is the IPC transport, not the Tauri modules: `listen` and `emit` are themselves
`invoke('plugin:event|…')` calls, so a delta in a test is delivered by a real `emit` through the
same `initTracker` the app runs — and `plugin:dialog|open` needs no separate mock.
`tests/support/ipc.js` also calls `mockWindows`, without which `getCurrentWindow()` throws,
`settings.js` reads that as "we are in a browser" and the window-close path silently never
registers.

Stores are module singletons holding more state than they export — `timer`, `chain`, `watching`,
`moving` — so `tests/support/stores.js` rebuilds the whole graph per test with `vi.resetModules()`.
It hands back `nextTick` from that same fresh graph: `resetModules` recreates `vue` too, and
another instance's `nextTick` drives another scheduler, so a test awaiting it would wait for a tick
that never comes.

Not covered, deliberately: `.vue` files and the CodeMirror wiring (`editor/theme.js`,
`extensions.js`, `compartments.js`), and, for the same reason, `TerminalView.vue` and
`terminal/theme.js` — all of it is DOM, and it is checked by eye through `?view=gallery`.

### Styling: inline style objects, never CSS classes

Components carry no scoped CSS and no utility classes. Every visual value is a computed style object
bound with `:style`, and every value in it is a `var(--token)` reference (see `core/Button.vue`,
`status/StatusBadge.vue`). Two consequences:

- A new component follows the same shape — `computed(() => ({ ... }))` of token references. Do not
  introduce a `<style>` block or a class-based approach.
- Never hardcode a colour, radius, spacing or font value. If a token does not exist for what you
  need, that is a design-system question, not a licence to write `#hex` or `8px`.

Two exceptions, and exactly two. The first is `components/files/editor/theme.js`. CodeMirror renders
its own DOM, and the only way to reach it is CSS rules, so this one file is allowed to produce them,
through `EditorView.theme()`. The rule is narrowed, not lifted — every value inside is still a
`var(--token)` reference, and no `#hex`, no `px` and no gradient belongs there. `@codemirror/search`'s
own theme paints a `linear-gradient` onto its panel buttons; `theme.js` suppresses it explicitly
(`backgroundImage: 'none'`), because gradients are forbidden everywhere in this system, including
inside a third-party stylesheet that ships its own opinion.

The second is `components/terminal/theme.js`. xterm.js renders its own DOM too, but its API differs
from CodeMirror's in a way that matters: `EditorView.theme()` takes CSS, so `var(--token)` works there
and the browser repaints on its own; xterm.js takes an `ITheme` of **resolved colour strings**, so
tokens have to be read with `getComputedStyle` and handed over as values. The consequence has no
parallel in the editor — flipping `data-theme` does **not** repaint the terminal for free, which is
why `TerminalView.vue` carries a `MutationObserver` on the root's attributes. The rule is narrowed the
same way: every value still comes from a token, and no `#hex`, `px` or font literal belongs in that
file.

`styles/styles.css` is an `@import` list only; the tokens live in `styles/tokens/`. `tokens/base.css`
holds element defaults (focus ring, selection, scrollbar) and the only three global classes in the
system (`.sm-mono`, `.sm-hatch-blocked`, `.sm-scroll-hidden`).

The first line of `base.css` is `box-sizing: border-box` on everything, and the whole system rests on
it. Components declare a size as a token and add padding and a border on top — `width:100%` with
`padding:0 var(--space-4)`, `height:var(--control-h)` with a border, `width:8px` with a 1.5px ring —
which only comes out right under border-box. The React design system gets it from its own reset; the
port did not carry it over at first, and the cost was `Input` overflowing `Modal` by exactly
`2×--space-4 + 2×--border-w` and `StatusDot` drawing its single `size` prop as three different glyph
sizes depending on whether that silhouette happened to have a border. Both vanished with the one
line. Do not remove it, and do not "fix" a component by subtracting its own padding from its width.

### Theme and density live on the document root

Both are attributes on `document.documentElement` (`data-theme`, `data-density`), set by a
`watchEffect` in each view. Every token is defined against them: `tokens/color-*.css` redefine
colours under `[data-theme="dark"]`, and `tokens/space.css` redefines *only* the space scale and
row/control heights under `[data-density="compact"]` — density never changes colour, radius or type.

### `status/status.js` owns colour and loudness

The single source of truth for what a status looks like and how loud it is:

- `RESERVED` = `blocked, ready, running, needs-you, done, failed`. These get fixed
  `--status-<name>-*` tokens and a distinct glyph from `STATUS_GLYPH`.
- Anything else is user-defined: `normalizeStatus` → FNV-1a hash → one of 12 generated slots
  (`--status-gen-<0-11>-*`), hues chosen to stay outside a guard band around every reserved hue.
  Generated statuses also render a 2-letter `statusCode` — **status is never colour alone**.
- `attentionLevel(status)` returns `loud` / `live` / `quiet`; components set `data-attention` and
  dim `quiet` to `--attn-quiet-opacity`.

Two rules that break the product if ignored: `loud` (needs you) is budgeted at **1–2 per screen** —
if everything shouts, the design failed — and there is no fixed column set, so never hardcode one.

`core/interactive.js` (`useInteractive`) is the shared hover/press tracker: interaction is a surface
step up, never a colour change and never a transform, so controls in a dense list cannot jump.

### `kanban/issueType.js` — what a card is, beside what it is doing

A card does not draw its status: it is already the column it sits in, and saying it twice spends the
one badge a card has on nothing. It draws bd's **type** instead, through `TypeBadge` — the status
prop stays because it still decides the card's loudness, the border, the flash and the dimming of
anything done.

Three of bd's six types carry a hue (`bug`, `feature`, `epic` — `tokens/color-type.css`); `task`,
`chore`, `decision` and every custom type share the neutral `--type-plain-*` set. That split is the
whole design: `task` is bd's default, so a board where every type is coloured is a board where
nothing stands out. A type this file has never heard of is an ordinary outcome, not an error — it
renders with its own name, the generic `tag` glyph and the neutral colours.

The type hues sit *inside* the reserved status bands, which the generated status palette carefully
avoids, and that is deliberate rather than sloppy: there is no free hue space left, so the two
vocabularies are separated by silhouette instead. A type badge is **sentence-case sans with no
border**; a status badge is **uppercase mono with one**. Both sit side by side in the task
inspector's header, which is where the difference is easiest to check. Keep it — a red `Bug` drawn
in the status idiom is a red `failed` pill to anyone scanning quickly.

### Icons

`core/icons.js` is the only file that names Lucide, and it registers glyphs explicitly so the build
tree-shakes to the ~58 actually used. Adding a glyph to the UI
means adding it there first; `Icon` warns in dev for an unregistered name. Swapping icon sets means
replacing that one file. Note `message-circle-question-mark` is kept as the design-system key and
mapped to lucide 0.469's `MessageCircleQuestion`.

### Adding a component

Create it under the matching `src/components/<group>/`, export it from `src/components/index.js`
(the library's public surface), and add it to `views/Gallery.vue` so it stays checkable. Product
code imports from `index.js`; components import their siblings by relative path — the `@` → `src`
alias exists in `vite.config.js` but is currently unused, so prefer relative paths for consistency.

## Constraints

- **No gradients, images, glass, blur or emoji.** Partly taste, partly the WebKitGTK constraint.
- Sentence case everywhere; identifiers in mono (`--font-mono`), prose in sans.
- The primary button is ink on paper with no brand hue — the entire saturated range belongs to
  status.
- The build target (`es2021`, `chrome100`, `safari15`) is set for the system webviews Tauri runs in
  (WebKitGTK / WKWebView / WebView2). Do not raise it, and do not reach for APIs newer than that.
- `tokens/fonts.css` `@import`s IBM Plex Mono from Google Fonts; an offline Tauri build needs the
  latin subset vendored locally instead.
