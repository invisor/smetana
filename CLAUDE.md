# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

It is tracked, and that is a decision. A blanket `CLAUDE.md` line in `.gitignore` kept this document
out of every worktree cut for a task, so nothing working in one could correct it when the code moved
underneath, and it drifted until it named six stores against a tree holding eight. A document that is
wrong is a thing to fix in a diff, and an untracked file has no diff.

`AGENTS.md` beside it is the other half, for agents rather than for this architecture: bd's quick
reference, and the rule that `cp`, `mv` and `rm` may be aliased to `-i` on a person's machine, so
every such call needs `-f`/`-rf` or it hangs for good on a prompt nobody can answer.

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

One file, or one test, rather than the lot:

```sh
npm test -- tests/stores/tracker.test.js   # positional argument: a regex over the file paths
npm test -- -t "stale response"            # by the text of an it()/describe()
cd src-tauri && cargo test parse_commondir # by test name
cd src-tauri && cargo test settings::model # by module path
```

Two test runners: `npm test` covers the front end's pure logic — the plain modules and the stores —
and `cargo test` covers the Rust side. That used to say "the nine plain modules" and had been wrong
for some time before anybody noticed; `tests/` mirrors `src/`, so the directory is the count and it
cannot drift the way a number written once does — name where a list lives, not how long it was on the
day somebody looked. Neither runner covers components: there is no component test runner and no
linter or formatter, so do not invent one, and do not claim a change is "tested" on the basis of a
build succeeding.

Front-end tests live in `tests/`, never next to the source. They mock exactly one thing — the IPC
transport — through the official `mockIPC`, and rebuild the store module graph per test;
`tests/support/stores.js` explains why, and `tests/support/setup.js` is the `setupFiles` every one of
them passes through.

A component change is still verified by eye, in the dev server, with the query parameters the app
reads (`src/App.vue`):

| parameter | values | default |
|---|---|---|
| `theme` | `dark`, `light` | `dark` |
| `density` | `comfortable`, `compact` | `comfortable` |
| `view` | `gallery`, `settings` | the app |

`?view=gallery` renders every exported component once (`src/views/Gallery.vue`) — the harness for
catching a broken component. Check any component change in all four theme × density combinations,
since both switches only swap CSS custom properties and a component that hardcodes a value will
look correct in exactly one of them.

`?view=settings` is the settings window (`src/views/SettingsWindow.vue`), which in the app is a
second OS window loading that same query string. It paints itself from what the app window tells it,
and in a browser from `settings_load` — which is what makes the settings screen checkable without
Tauri; the one thing it cannot do there is change anything for good. `?theme=`, `?density=` and
`?tab=` are passed in as props rather than read there, so `App.vue` stays the one place that knows
about the query string, and without that this window's own chrome could not be seen in compact or in
the other theme at all.

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

That right column draws one of four things, and which one is **derived rather than stored**
(`rightPanel` in `DesktopApp.vue`). `DraftInspector` shows a task still being filed — the person's
own words, read-only, with no issue behind them. `ClaimedTasks` shows what a run has taken, with the
card for whichever of them is picked, the list staying above the card because the choice between them
is the point. Otherwise it is `TaskInspector` on the selected issue, and with nothing picked an
`EmptyState` saying so — the slot used to hold a fixture issue instead, so a newly added project
opened by announcing that somebody had filed a task needing a human (smetana-agh). That empty state
is deliberately not drawn under a run's claimed list, where the list is the content. Deriving the
choice is what stops the halves drifting: `selectedTask` is remembered per project in
`settings.json`, so a panel choice that wrote to it would turn a glance at an agent into an edit of a
preference, and a stored version had the run case highlighting a card the inspector refused to draw.

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
invisible there with nothing to say it went missing. Three fields broke that and the cost was exactly
what the rule predicts (smetana-dbr): bd emits `notes`, `design` and `acceptance_criteria`, the
struct had none, and `notes` is where `running-tasks` writes the reason a run parked a task — so the
one sentence explaining why the night left a task alone was readable only through `bd show`. All
three are back, drawn as prose under the description in spec-then-log order. The one thing still
deliberately dropped is on the edges: `Dependency` keeps only the ids and the kind, because the panel
draws an edge as a "Blocked by" id and bookkeeping about the edge has nothing to be drawn as.

That panel is read-only apart from the status — rewriting a title or a description is an agent's job,
and "Ask agent to edit" starts one on the issue. The status picker offers three of bd's eleven
statuses (Ready, Pinned, Done); the rest belong to agents, so the one the issue actually holds is
appended as a fourth option when it falls outside those three, because a picker with no matching
option would render its first entry and claim the issue was Ready. That rule and its `STATUSES` live
in `components/kanban/taskMenu.js`, not in the inspector: the card's overflow menu offers the same
three, and two copies would have drifted the first time bd grew a status. Delete is `bd delete -f` —
irreversible, and `-f` is not about skipping a prompt: without it bd prints a preview, deletes
nothing and exits zero.

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
`src/stores/terminals.js`, `src/stores/git.js`, `src/stores/runs.js`, `src/stores/attachments.js`
and `src/stores/app.js` are the **only** files in `src/` that know Tauri exists — components see
reactive stores and nothing else. `mockBackend.js` below is the last and the exception that proves
it: it imports Tauri in order to stand in for the absence of one. `app.js` is the odd one, and it is
a store for exactly this reason rather than for holding state: it has none. It is what the app knows
about itself and asks the desktop for — open the settings window, read this build's version, open a
link in the person's own browser — and every one of those would otherwise be an `@tauri-apps/api`
import inside a component. Several of those files open by counting themselves into this list, which
is a habit worth knowing about before trusting one: an ordinal is written once and the list keeps
growing under it. The list here is the one to check against the tree.

`tracker.js` also owns the two translations: bd's statuses to the design system's (`open → ready`,
`in_progress → running`, `closed → done`; everything else, including custom statuses, passes through
to `normalizeStatus` and gets a hash colour with a 2-letter code), and Rust's diagnostics to short
English messages, with the raw text left in the console. `projects.js` owns the list of open
projects, which one is active, and moving between them — the front end holds the list's truth, bd
holds the board's, so a switch reads the new project's layout with `settings_load` (only the layout:
the list on disk is already the past by then) before it asks the tracker to point at the new
directory — plus offering `bd init` in a folder that has none yet.

In a browser there is no back end, so `src/stores/mockBackend.js` installs the official `mockIPC`
with the old fixtures: read commands answer, and writes to the tracker reject loudly — a "write"
that looked like it worked would be worse than none. `settings_save` is the one exception, accepted
and dropped, because a browser has nowhere to keep it and failing every debounce tick would only
fill the console. That is what keeps `npm run dev` and `?view=gallery` working with no branching in
components.

### The bd sidecar

bd ships inside the bundle (`bundle.externalBin`), so the app is self-contained and the version is
fixed. The binary is 128 MB and is **not** committed: `scripts/fetch-bd.mjs` downloads the pinned
release, verifies it against the sha256 digests committed next to `BD_VERSION` (the release's own
`checksums.txt` is only a cross-check), and lays it out as `src-tauri/binaries/bd-<target-triple>`.

`postinstall` runs it with `--optional` and only warns on failure — a contributor who wants the
front end alone should not need a Rust toolchain and a 43 MB download. `npm run fetch-bd` and CI
fail hard instead. `EXPECTED_BD_VERSION` in `service.rs` must stay in step with `BD_VERSION` in the
script; a mismatch surfaces as `bd-version-mismatch` in health, not as a crash.

An agent files a task by running `bd create` itself, and reaches the same binary because
`terminal/pty.rs` puts the sidecar's directory on the front of the `PATH` the agent inherits — see
the terminal section for why in front and not behind.

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
(chrome and syntax highlighting, entirely on tokens — see the styling exception above), `extensions.js`
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

The same file holds the no-spawn rule at the one place it is genuinely inconvenient — a branch list
is not one line the way `HEAD` is, it is `refs/heads` walked for loose refs, `packed-refs` for the
ones git has folded away, and each branch's own reflog under `logs/refs/heads`: three reads, still
cheaper than a process, all from the common directory so a linked worktree offers its whole list. The
reflog is what orders the result rather than the alphabet, because the branch somebody merges into
every day is nowhere in particular alphabetically; a branch with no reflog anywhere falls outside the
recency group entirely, into the alphabetical tail a fresh clone leaves nearly everything in. Nothing
in that reading is an error either: a folder outside git offers an empty list rather than a failure,
and a repository whose only branch has no commits yet has no ref file at all — a merge-target field
offering nothing would be worse than one offering the single branch that exists.

A run's dialog reaches those same three sources through `branches_with_recency`, which sorts and
dedups the names before stamping each with its own reflog time — the ordering itself is left undone,
deliberately, because it is `by_recency`'s rule and not a second one written here. `combine` is the
pure function that applies it: it folds several repositories' lists into one, splits complete from
partial, and calls `by_recency` once on each group. Its one new judgement is where a branch's
freshness comes from across repositories — `develop` opened an hour ago in `backend` and a month ago
in `admin` is an hour old, because it is one branch to the person merging into it, and taking the
first repository's answer, or the least of them, would bury the branch somebody is actually in.
`BranchOption { name, missing_in }` is what a folded list is made of: a name, and the repositories
from `[project].repos` that do not have it, in the order those repositories were given. An empty
`missing_in` means every one of them has it.

**`git.rs` no longer answers the dialog.** `runs::commands::target_branches` does, because "what may
this run merge into" is a question about a run rather than about one directory: it reads
`.smetana/project.toml` itself, through `config::load`, and walks `[project].repos`, calling
`branches_with_recency` once per repository and folding the results through `combine`. `git.rs` keeps
its shape — a leaf, no worker, no spawn — and no code in it reads project configuration: `combine`
takes a list of `(name, branches)` pairs and never learns where they came from. The config is read
inside that one command rather than taken from the front end, and that is the design rather than a
shortcut: `runs.js` holds its own copy of the config, filled by its own `project_config` call, and
the run dialog is shown before that call has landed — the whole of `smetana-6gs` and `smetana-o8r`,
where the branch-filling rule ran once against a list that was not there yet. A repository list
threaded down from the front end would be the same race wearing a different name; reading both facts
inside the one command leaves no order between them to get wrong.

What the field draws from that is two groups, headed "Everywhere" and "Not everywhere", and no
captions at all when nothing is partial — which is every single-repository project, and therefore the
common case. A name in `[project].repos` that resolves to nothing readable, a missing folder or one
with no `.git`, is left out of the coverage question entirely rather than counted as missing every
branch: the alternative reads worse in exactly the case that matters, since one typo in the config
would make every branch partial, empty the field's top group, and bury the real question behind a
fault that has nothing to do with it. This is what closed a defect with no issue behind it, and the
shape of it is worth keeping: a project of four repositories living under one folder had the dialog
asking not any of the four but the fifth repository that folder itself happened to be, so `develop` —
present in all four — read as a branch nobody had, and the run went out telling the agent to cut
`develop` from the current branch in every one of those four repositories, though each already had it
with its own history.

**Refs are shared and HEAD is per-worktree, and conflating the two is `smetana-5t7`.** A linked
worktree's git directory — whatever its `.git` file points at, `.git/worktrees/<name>` — holds only
the per-checkout half: `HEAD`, `ORIG_HEAD`, the index, `logs/HEAD`. Everything a branch list is made
of lives in the *common* directory instead, named by a `commondir` file sitting next to that git
directory, and `parse_commondir` resolves it — relative (git's usual `../..`) against the git
directory rather than the checkout, absolute taken as-is, and missing meaning an ordinary clone that
*is* its own common directory. So `refs/heads/`, `packed-refs` and `logs/refs/heads/` are all read
from that one resolved place, while `HEAD` stays where it is. Before that, opening a linked worktree
as a project offered exactly one branch in the run dialog — the branch the worktree was already on,
which is the single branch nobody needs to merge into — and the reflog ordering did not work at all,
since the log directory was not there either. Live-checked against this repository's own linked
worktree: the same list as the main checkout, in the same reflog order, with HEAD still reading
per-worktree.

The counters next to it — uncommitted files, running agents — are still fixture.

### The terminal: agent sessions

The centre's `terminal` tab (`chat` before it grew a terminal — `ProjectState::validate` migrates the
old name on load, since files on people's disks carry it and without the substitution that tab would
fail the closed-list check and silently become the board) runs CLI coding agents under real PTYs, one
per session, listed in the sidebar's Agents view (`components/agent/AgentList.vue`) and started from
its "+ New agent" row, or from the task inspector's "Ask agent to edit". The reason the subsystem
exists at all is the second half of that sentence: it notices when an agent is waiting on a human,
including one in a tab nobody is looking at.

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
happens to have bd of its own. `sidecar_dir` derives the directory as tauri-plugin-shell does,
`dirname(current_exe())`, so it is the same directory `app.shell().sidecar("bd")` resolves to by
construction. It goes in front of the inherited value, never behind: the app pins a bd version and
checks it, and an agent that found some other bd first would be writing to the board through a
version that handshake never verified.

What that directory goes in *front of* is not the `PATH` this process inherited, and
`src/shell_env.rs` is why. A bundled app on macOS is handed launchd's environment: `open smetana.app`
gives it whatever `launchctl getenv PATH` says, which on a stock machine is nothing, so it falls back
to `/usr/bin:/bin:/usr/sbin:/sbin`. Everything a person installs — `~/.local/bin`,
`/opt/homebrew/bin`, nvm's shims — reaches `PATH` from `~/.zshrc` or `~/.zprofile`, which only a
shell ever reads. So the app asks a login shell once (`$SHELL -i -l -c`, the value fenced between
markers because an interactive rc file writes shell-integration escapes into the same stream), and
that answer is what everything that starts a program works from — `agents::pick` and `build_command`
here, `runs/usage.rs` and `runs/preflight.rs` in the run worker. Whether an agent is installed and
the environment it is started with are one question, and answering only the first would trade "no
agent is installed" for an agent that cannot find `git` or `node`. `-l` alone is not enough: the
machine this was written on adds cargo and the rest from `~/.zshrc`, which only `-i` reads. Every
failure — no shell, a five-second timeout, unrecognisable output — falls back to the inherited value.
The bug is invisible in development, which is why it is a module rather than a line: `npm run tauri
dev` starts the binary from a terminal, so the process already has the full `PATH`.

| file | what it does |
|---|---|
| `model.rs` | `Session`, `SessionState`, `Question`, `TerminalError` — the vocabulary, and the pure rules for entering and leaving each state (`Session::apply`, `finish`) |
| `transcript.rs` | a batch's machine-format output cut into lines and handed to the profile's own rendering, before anything downstream sees a byte of it |
| `ring.rs` | the raw-byte scrollback ring, trimmed on overflow to a line boundary |
| `screen.rs` | a `vt100` grid built from the same bytes — the text a person would actually see |
| `detect.rs` | layer A: bell and silence, a pure function of the screen, the bell flag and the timings |
| `pty.rs` | the only file that touches the OS: spawns, reads, writes, resizes, kills; also assembles the child's environment |
| `service.rs` | the worker: one owner of mutable state, request queue, output and state events |
| `commands.rs` | thin `#[tauri::command]`s, shaped exactly like the tracker's |

`service.rs` is a single tokio task, the same shape as the tracker's worker and for the same reason:
commands, PTY output arriving from per-session reader threads, and a 16 ms flush tick all meet in one
`select!`. A session starts at a fixed 120×30 before any view has attached to it; the first
`TerminalView.vue` that does replaces that with the pane's real geometry through `terminal_resize`,
which also feeds the new size into `screen.rs` — the app is obliged to read the screen at the size a
person actually sees.

**One stream, two models.** Every chunk from a PTY goes into `ring.rs`, a raw byte buffer for the
human — exactly what xterm.js repaints itself from on attach — and, separately, into `screen.rs`, a
`vt100` grid for the app. The raw stream is cursor moves and repaints with nothing findable in it; a
`\r` overwriting "thinking..." with "done" is two writes in the ring and one line on the screen.
Detection reads the screen, never the ring. xterm.js is a third, independent emulation fed the very
same bytes, so the person's picture and the app's agree by construction rather than by hand. A
batch's chunk is translated before any of the three has seen it, inside `absorb` itself, and that
position is the whole of why it is safe: one translation ahead of the fork leaves all three reading
one identical stream, where translating for the pane alone would set them arguing.

`seq` plays the part `generation` plays for the tracker: every flushed output event carries a
monotonic number, `terminal_attach` hands back the ring's snapshot plus the `seq` to continue from,
and `terminals.js` re-attaches on an out-of-sequence event. Attaching clears whatever that session
had queued to flush — it is already in the snapshot just handed over.

Output only flows to the front end for the **active** session — `flush()` drops a background
session's pending bytes on the floor every tick, because nobody is rendering them. **State flows for
every session, active or not** — `reassess()` walks all of them — and that asymmetry is the entire
point: a background agent's row can turn `needs-you` while its bytes never leave the worker.

Detection is two layers that degrade in one direction only. Layer A (`detect.rs`) is
agent-independent — a bell, or three seconds of stillness — and has nothing in it to break. Layer B
is `Profile::question`, so it lives with the agent it reads rather than in this subsystem:
`agents/claude.rs` reads Claude Code's own interface, and a version bump to that CLI can break it. It
did: the dialog was a box until 2.1, and the frame was what told it apart from any numbered list in
the agent's own output. Today it is fenced by horizontal rules with bare lines, so two other
properties carry that weight — the options number themselves 1, 2, 3 … and the **last** such block on
the screen is the dialog, since anything merely printed sits above it; and exactly one option carries
the cursor, which prose never does. The question is the run of text directly above the options,
ending at a blank line or at the rule under a diff preview, and must end in a question mark. Layer B
is trusted only once the screen has held still for `SETTLE` (150 ms). And `idle` is deliberately
quiet: a finished agent and a waiting agent both simply stop producing output, so loudness comes only
from the bell or from a layer B match, never from silence alone.

That last rule is a rule **plus one named exception**: Claude Code's one-off folder-trust dialog,
whose question is not the paragraph above the options — a link caption is (smetana-xh7). The second,
narrower reading opens **only after the generic one has declined** and only under a heading from
`const HEADINGS`, a literal table of strings such a dialog prints and ordinary output does not.
Neither guard is relaxed for it: the search stays fenced between heading and options, and the
question mark is still required, because dropping either would let a diff preview or a numbered list
in the agent's own prose turn a session `needs-you` against a budget of one or two loud rows a
screen. `claude.rs` carries the rules and the refusals it was measured against, over fixtures
captured under a PTY off claude 2.1.226. A wording change on Claude Code's side loses the reading and
leaves layer A in place, which is how the rest of that file already fails.

**Quiet is measured on the screen, not on the byte stream**, and that is what `Quiet` in `detect.rs`
exists for. An agent that is waiting can still be talking: Claude Code 2.1 repaints an open
permission dialog about every 0.61 s for as long as it stands there, and while quiet meant "no bytes
arrived", every one of those chunks restarted the clock — so a session waiting on a human read as
`Running` for as long as it waited and `IDLE_AFTER` was unreachable (`smetana-8h7`). A repaint that
draws the same text changes nothing a person could act on, so what gets timed is the picture they
see. The rule cuts the other way too, deliberately: a session whose screen holds still for
`IDLE_AFTER` is called idle even while bytes pour in, which is the honest reading and cheap to be
wrong about.

`Quiet` keeps a hash rather than the screen — this runs for every live session on every detection
tick, and holding the previous screen would mean copying kilobytes per session per tick. **The
fingerprint deliberately covers the plain text of the visible rows and nothing else**: no colour, no
bold or reverse, no cursor. So an attribute-only repaint, or the cursor moving over unchanged text,
counts as stillness — and **feeding attributes into it would bring the bug straight back**, since an
agent waiting on a person redraws its dialog to keep the highlight under the selected option alive,
which is a colour repaint of identical text. Getting it wrong that way is silent: a session needing a
human reads as busy. Getting it wrong the other way, for a spinner that animates purely in colour,
costs a dashed dot instead of a spinning one.

**Half of `smetana-8h7` is fixed and half is not, and the difference matters when changing this.**
The silence half is closed **for repaints that redraw identical text — the mechanism the fix assumes,
and not one that has been observed on the dialog it was aimed at**; the live check could not reach a
permission dialog without spending model quota, and the trust dialog is no stand-in, having been
measured emitting zero bytes after the first 0.6 s. The bell half is not closed: Claude Code still
rings none on a permission prompt. What an unmatched layer B produces is `Idle`, which reaches the
front end as `ready`, whose loudness is `live` — so the whole visible cost of a waiting agent no
profile could read is a dashed dot instead of a spinning one. **Nothing shouts, nothing dims, and
nothing else in the app acts on the state at all**, and `NeedsYou` comes only from a bell or from a
profile's own match.

An agent that has genuinely finished still reaches `Idle` at about three seconds, but not to the
millisecond, and the drift goes both ways: earlier, because the last bytes a CLI writes are often
invisible ones the old clock counted and this one does not; later, because the clock is stamped when
the worker next looks rather than when the screen changed, so it lags by up to one detection interval
(`REASSESS_EVERY` × `FLUSH`, ~64 ms today). Lengthening that interval lengthens this error with it.

`terminal_run_capture` — the call an automated flow uses to drive a session and read back its settled
screen — refuses with `busy` when the session is `needs-you`, and also when a bell is still unrung
even if state hasn't caught up yet (state lags the fact by up to `SETTLE` plus a tick; the bell flag
is that same fact arriving sooner). Writing into an open permission dialog would answer, on a human's
behalf, a question the app never read and the human never saw. **What that guard cannot catch is the
other half of `smetana-8h7`**: a dialog whose agent rang no bell and whose profile failed to read it.
Layer A calls that session `Idle`, which is the truth and not a refusal — an idle session is exactly
what a capture expects to write into, so `Idle` can never join this guard without breaking the
ordinary case, and layer B is therefore the whole of the protection here. The capture's own settle is
the one place the stream is still the right thing to measure, and deliberately the opposite of what
layer A does: a capture has just written into the session and is waiting for an answer to arrive at
all, so a screen that happens to look unchanged mid-answer is not a settled one, and reading a
half-finished reply as finished would hand a caller the wrong text with nothing to say so.

Sessions do not survive a restart, and nothing about them is written to `settings.json` — a session
row with a dead process behind it is worse than an empty list. `RunEvent::Exit` calls
`terminal::service::shutdown`, and the worker ends every session the way closing a terminal window
does: `SIGHUP` to the session's process group — which reaches whatever the agent itself started, as
`SIGKILL` to the direct child would not — then a short wait, then a kill for what is left. The two
seconds `shutdown` itself waits are the ceiling on a *wedged worker*, the same one `settings.js` puts
on its close-time flush: the window always closes, and a worker that never answers costs the cleanup,
not the app. Anything that outruns that, or that the app never got a chance to signal, is an orphan;
for the sessions a *run* started, the next launch finds them again through the registry under Runs
below, the one place a session's pid is written down.

`src/stores/terminals.js` keeps the same cost-driven split as the worker: `sessions` and `agentRows`
hold every session's state, cheap and needed for a background row's colour; output bytes go only to
the callbacks registered through `subscribeOutput` — in practice the one live `TerminalView.vue`.
That register is a `Set` and every subscriber gets every chunk: a single field would tie
unsubscribing to who mounted last, exactly the ordering the rest of this subsystem refuses to depend
on.

`activeId` looks like it names one thing and actually names two, and conflating them was a real
defect: "which agent the human has selected" has to survive leaving the terminal tab, because
`AgentList.vue` highlights its row from this same field, while "which session the worker is streaming
output to" has to end the moment that view unmounts. While a single field served both, leaving the
tab cleared the selection and the terminal came back permanently blank. `detach(id)` takes the id it
is leaving: switching agents is two IPC calls with no ordering guarantee at the worker, so a nameless
detach arriving after the new attach would silence the session the human just switched to, with no
error anywhere. `detach` never touches `activeId` — selection is not the transport's to forget.

A session's row is captioned by the **work** it was started for, never by the process behind it, and
`SessionWork` in `terminal/model.rs` is what an `Intent` reduces to for that purpose — which of its
payload is drawn and which was only a briefing for the agent. `Intent::work()` lives in
`agents/mod.rs` rather than in `terminal::model` because it is knowledge about `Intent`, and the
answer moves whenever a variant does: a `NewTask` carries its prose, type and priority across for the
draft panel to draw and leaves its `images` and its Brainstorming, Spec and Plan switches behind,
since those are instructions to the agent and nothing on screen would show them.

`SessionWork::Run` carries nothing at all, and that absence is honest rather than lazy: **which
issues a batch has taken cannot be known here.** The agent claims one by running `bd update <id>
--claim` itself, which the app hears about only as the tracker changing under the watcher — there is
no channel that says "this session took this issue". So `claimedBy` in `terminals.js` reconstructs it
from the two halves already on the front end: the run knows which session is working, the tracker
knows what is `in_progress`. An explicit report from the agent would be steadier and needs the agent
to send one; until then this is the reconstruction, written down as one.

`loadSessions` guards against its own stale response the same way `files.js`'s `stale` guards a
buffer: called twice in flight, without the guard the *last response* would win rather than the *last
call*, and the list could end up showing one project's sessions under another project's name — after
which the remove button in `AgentList.vue` would kill the wrong project's agent, silently. A test in
`tests/stores/terminals.test.js` pins this.

`TerminalView.vue`'s pane and its host both carry `minWidth: 0`, and that is not decoration next to
the `minHeight: 0` beside it. A flex item defaults to `min-width: auto` and refuses to shrink below
its own content — here, xterm.js at whatever width it was last fitted to — so narrowing the centre
column left the pane painted over the task panel, and converging visibly as `ResizeObserver` →
`fit()` → new cols → redraw fed each other. `KanbanBoard` and `FileEditor` never showed it only
because `overflow: auto`/`hidden` zeroes that automatic minimum for them already.

`TerminalView.vue` hosts one `Terminal` instance per view, not per session — switching agents calls
`reset()` and refills from the new ring snapshot, so returning to an agent lands at the end of its
output rather than wherever it was scrolled to. An instance per session, the way `editor/states.js`
keeps one `EditorState` per file, would fix that; it is not built because the lack has not been shown
to matter. `AgentList.vue` reads `attentionLevel` the same as the board's status badges, but draws it
with a triangle for `needs-you` against a dot for everything else — colour is never the only signal
here either. That triangle is the *whole* of what the app says about a waiting agent, and
deliberately: the right panel used to draw the selected agent's question with a button per option
above the task card, and it was removed (smetana-s4f) because it repeated what the terminal a few
centimetres away already showed, pushed the card the panel exists for down the column, and its option
labels — whole sentences, in a permission dialog — did not fit the panel's width. A person answers in
the terminal. The question still travels: `Session.question` is what layer B fills in and what puts
the session in `needs-you`, and `terminal_run_capture` still refuses to write into one; nothing draws
it, and `answer()` in `terminals.js` went with the block rather than being left as a write path
nothing calls.

In a browser, `mockBackend.js` answers `terminal_list` with one fixture session already sitting in
`needs-you` with a real permission question attached — the only way `?view=gallery` and `npm run dev`
can show that state with no Rust worker behind them — and `terminal_attach` replays a canned
transcript. Every write falls through to the same loud rejection the tracker's writes get.
`terminals.js` translates `NoAgent` into its own message naming what was looked for, rather than the
generic "nothing was created": it is the one failure in that list a person can act on, and since a
task is now filed by an agent, it is the difference between a missing convenience and no way to put a
card on the board. The names in it come from the error's own text, because Rust holds the only copy.

### The agents: one intent, two harnesses

`src-tauri/src/agents/` is what the app knows about the CLI coding agents it runs, one file per
agent, and everything harness-specific lives in it. Claude Code and Codex are supported; which one
runs is the `agent` field in `settings.json`.

The split that makes this a module rather than a `match` in the terminal worker: **what the app wants
done is the same for every agent, and how it reaches one is not.** An `Intent` — `Bare` from the
"+ New agent" row, `NewTask` from the new-task dialog, `EditTask` from a card's "Edit", `ResolveTask`
from a parked card's "Answer questions", `Setup` from the dialog a person gets when they add a
project, and `Run` for one batch of a run — is where the product decision lives, written once. `Run`
is the only one no person sends: `runs::service` builds it, carrying the whole of what the run was
asked to do rather than a reference to it, because a session outlives a settings change and a batch
that quietly retargets halfway through is worse than one wrong from the start that says so.
`SkillDelivery` is how a skill library reaches a particular harness, and there is no uniform answer:
Claude Code takes `--plugin-dir` and loads a plugin for one session, installing nothing
(`PluginDir`); Codex has no per-session mechanism at all — its skills system reads `~/.codex/skills/`
and the only way to add a root is a JSON-RPC method on the app-server, a different process from the
TUI this app spawns — so its skills ride as text in the prompt (`Inline`), since writing into
someone's home directory or repointing `CODEX_HOME` would reach into their own setup. Nothing about
either harness leaks into the code deciding what we want done: `prompt.rs` takes an `Intent` and a
`SkillDelivery` and is pure, which is where the tests are.

**Every prompt is a whole instruction, and a test pins it.** A prompt rides as the agent's positional
argument, and both harnesses submit that argument as the session's first message rather than leaving
it in the composer — so there is no such thing as a prompt somebody finishes by hand. `EditTask`'s
stopped mid-sentence at a colon on the theory that the person would type the second half; they never
got the chance, and the agent's first move was to ask whether the message had been truncated. It is
finished now by **asking** rather than by guessing, since an agent that decides for itself rewrites
an issue nobody asked it to touch. `no_prompt_stops_mid_sentence` walks every intent and both
deliveries and refuses a prompt ending in dangling punctuation.

| file | what it does |
|---|---|
| `mod.rs` | `Profile`, `Intent`, `Stage`, `SkillDelivery`, `ImageDelivery`, `TaskDraft`, `Autonomy`, `Launch` — the vocabulary, the registry, `cascade` and `IDS` |
| `library.rs` | where the bundled skills are, whether the person already has their own superpowers, and reading a `SKILL.md` for inlining |
| `prompt.rs` | an intent becomes the text the agent opens on — pure; the skill text, where one is needed, is read by the caller and passed in |
| `claude.rs` | Claude Code: `--plugin-dir`, and layer B, its permission dialog read off the screen |
| `codex.rs` | Codex: `Inline`, `-i` for images, and its own layer B — the approval dialog read off a screen with no frame anywhere on it (smetana-603) |

**Codex's layer B is genuinely a different reader, not Claude's with the glyphs swapped**, and the
two deliberately share no code, because a glyph one harness happens to use today is exactly what
drifts. Its rules are measured off fixtures in `src-tauri/tests/fixtures/`, captured under a PTY at
60 and 120 columns from CLI 0.146.0, and `codex.rs` carries each rule with the screen that forced it.
Three properties of that interface are why it cannot be shared: the cursor `›` (U+203A) is also drawn
in front of the person's own submitted prompt and the empty composer, so it counts only as the first
non-blank character of a line; there is no frame anywhere, so **the only structural boundary is
indentation**, with two blank rows between top-level blocks against one between paragraphs inside
them; and a block is **refused for what it hangs off** — a conversational turn, `•`, `◦` or `›` — not
for how closely it sits, which is what survives a turn wrapping over several rows in a narrow pane.

One known gap is recorded rather than papered over: **a scrolled screen with no anchor left on it**,
where the walk upward reaches row 0 having met nothing and indented prose above a numbered draft
still reads as a dialog. A test pins it by name. Closing it would mean requiring every block to be
anchored, which would refuse Codex's update prompt — a real dialog drawn from row 0 — so it is a
false match in a rare scroll position against a miss in an ordinary one, with no measurement to
settle it. Every rule here fails closed for the reason the design budgets loudness: a session wrongly
turned `needs-you` spends one of the one or two loud rows on the screen and makes
`terminal_run_capture` refuse a session with nothing open on it, so a change to that CLI should cost
a miss rather than a false alarm.

Five more methods on `Profile` are the same split one level down, and each one's **default is a
working answer rather than a gap** — the shape to keep when the next one is added. `images` says how
pixels reach a harness: Codex takes `-i/--image`, Claude Code simply opens a path the prompt names,
so the default is `InPrompt`, the one channel every CLI has. `usage_command` and `parse_usage` are a
pair, and a profile answering one without the other reads as unaskable, which the run gate treats as
no reason to hold anything up. `autonomy` is the extra arguments and environment for working with
nobody watching; the default is nothing, so a harness with no such switch stops at its first
permission prompt and turns `needs-you` — exactly what `Supervised` already is, which is the app
saying a harness cannot be autonomous by behaving like it rather than pretending otherwise.
`batch_args` and `transcript` are the last pair and hang off one predicate, `agents::is_batch`: an
interactive session finishes its work and sits at its prompt, so a run's loop — which comes round
only when the batch's process is gone — never comes round at all, and the non-interactive form that
fixes that is also the one printing a machine format nobody reads. So the first says how this
harness is told to carry one batch out and **exit**, in front of everything else on the line, and
the second says how a line of what it then prints becomes a line in the pane. Their defaults are
nothing and no translator, working answers again: a harness given neither runs exactly as every
harness ran before they existed — which is Codex today, deliberately and with its own task behind
it.

`agents::IDS` is the single copy of the agent-id list, and `settings/model.rs` validates against it
rather than repeating it — the side-tab hazard again: a value that survives the session and silently
comes back as something else. The front end never learns the names either: `settings.js` holds
whatever string is in the file and passes it to `terminal_create`, and Rust resolves it. A configured
agent that is not on `PATH` falls back to the first one that is, and `Session.agent` carries what
actually started; nothing on screen reads it, so the substitution is silent and the terminal is the
only way to see it. When nothing at all is installed the session fails with `NoAgent`.

`agents::LANGUAGES` is the same idea one field over: the twelve languages a person may choose, as
BCP-47 ids **with the English name of each**, and the only copy of that list — `settings/model.rs`
validates `agentLanguage` and `taskLanguage` against it exactly as it validates `agent` against
`IDS`. The name is carried beside the id because the name is what goes into the prompt: `zh-Hans` is
a tag out of a settings file, "Chinese (Simplified)" is a sentence. Both default to `en` rather than
to an Auto position, which would have meant "say nothing about language" — today's behaviour exactly,
so an update changes nothing until somebody chooses. The price is deliberate: `Intent::Bare` no
longer opens on nothing, since it carries the one sentence naming the conversation language, and the
alternative was that the session where a person talks to the agent most is the one the setting cannot
reach.

Neither language crosses the IPC. `settings::languages(app)` reads the file where
`settings::agent(app)` already does, and `terminal::service`'s `Create` arm calls it while building
the `Launch` — the one place every session in the app is built, so a person's session and a run's
batch get the same answer by construction. From the `Launch` the two ids reach `prompt::build`, which
stays pure. Two costs come with reading it there and both are accepted: a session started in the same
fraction of a second as a language change reads the previous language (the front end writes on a
400 ms debounce, the lag `settings::agent(app)` already lives with), and a run reads the languages
**per batch** rather than snapshotting them, so a language changed at 2am reaches the next batch and
one run's issues can end up in two languages. Putting them on `Intent::Run` instead would be a second
road into a session, which is what reading them in one place exists to prevent.

What each moves is not the same. The conversation language goes into **every** intent; the task
language goes only where the agent writes into bd — `NewTask`, `EditTask`, `ResolveTask` and `Run` —
and it carries a caveat that is not optional, because what the setting must never move is a string
some other piece of software matches on. The `##` section headings, since `bd create --validate`
matches the wording of a heading and nothing else, so a translated `## Acceptance Criteria` is bd
refusing the issue. And the markers a note begins with: `parked:` and `resolved:` are matched as
literals by `components/kanban/parked.js`, so a translated one empties `openQuestions` and the parked
card's dialog says nothing is open while the Ready warning goes quiet — silent, and landing on
somebody trying to answer a parked task. What the setting moves is the title, the body of the
description, the criteria themselves and what follows the colon in a note. Specifications and plans
are English whatever either setting says (`IN_ENGLISH` in `prompt.rs`): they are read by whoever
picks the work up months later and by every agent after them. A setting for the language of *code
comments* was asked for and refused — it would either do nothing in a repository with a convention,
or produce exactly the regression the Language section names.

Two directories under `src-tauri/resources/` are the library itself, both bundle resources.
`smetana/` is ours — the directory is the list, for the reason the test-count note under Commands
gives — laid out as a plugin in its own right (`.claude-plugin/plugin.json`, `skills/<name>/SKILL.md`)
because that is what `--plugin-dir` accepts and what makes them answer to `smetana:filing-a-task` and
the rest. Four intents name one apiece: filing names `filing-a-task`, a parked task's questions name
`resolving-questions`, setting a project up names `project-setup`, a run's batch names
`running-tasks` — and that last one is the process the rest hang off, since an agent carrying out a
batch reaches `provisioning`, `reviewing`, `merging` and `live-checking` because `running-tasks`
sends it to them, not because the prompt lists them. That is the point of a library over a longer
prompt: the prompt names an entry point and the library carries the depth. `superpowers/` is a
committed copy of that plugin, 668 K of markdown under MIT, with its own `LICENSE` and a
`SUPERPOWERS_VERSION` recording version and commit sha, the way `BD_VERSION` does for the sidecar —
committed rather than downloaded because 668 K of text is not 128 MB of binary, and committing makes
the build hermetic.

The vendored copy is stripped of its `hooks/` directory, the one exclusion that changes behaviour
rather than size. Superpowers ships a `SessionStart` hook injecting "you MUST invoke" into every
session the plugin is loaded into; through `--plugin-dir` that would impose the process on "+ New
agent" and on editing an issue — the two intents this design deliberately leaves alone — and would
make the Brainstorming switch a lie in its Off position. A person who installed superpowers
themselves keeps their own hook, and our copy is never loaded for them. `library.rs` decides that
from `~/.claude/plugins/installed_plugins.json`, where a key is `<plugin>@<marketplace>` and its
value is the list of scoped installs — both halves matter, since a key with an empty list is a plugin
uninstalled everywhere. Anything unreadable answers "no": a second copy costs a duplicate line in a
list, while withholding it removes the feature with nothing on screen to say so. When it is handed
over it keeps its own name, which lets the prompt say `superpowers:brainstorming` in both cases.

**Filing a task is an agent session, not a write.** `NewTaskModal` no longer emits an issue: its
fields become a `TaskDraft` inside a `NewTask` intent, and `DesktopApp.vue` switches to the agents
side tab and the terminal centre tab and calls `createSession`, exactly as "Ask agent to edit" does.
The agent runs `bd create` itself and the watcher puts the card on the board — and `createIssue`,
`tracker_create`, `NewIssue` and `create_args` are deleted rather than left unused, because a live
write path into the tracker that nothing calls is the kind of thing that gets called again in six
months.

The dialog collects one piece of prose, not a title and a description: the person writes what needs
doing in a single `Textarea`, and the title bd wants is written by the agent, the only party that has
read the text. Five `Dropdown`s sit under it in two rows, and every one defaults to **Auto** — type,
priority and Brainstorming, then Spec and Plan. For the first two, Auto travels as `null`, never as
the word, so `TaskDraft`'s `Option<String>`/`Option<u8>` cannot carry a type bd would reject;
`prompt.rs` then names the pinned fields as settled and hands the rest to the agent *by name*
("Decide the priority yourself"), because an agent told nothing about a field would have to invent
one anyway and would not know that inventing it was its job rather than a gap in the briefing.

Brainstorming's three positions: `Off` files it now; `On` requires a discussion first; `Auto` states
the test the agent applies and leaves the judgement to it, since nothing in the app has read the text
of the task and a heuristic on its length would misfire in both directions. How to file one
*properly* is not part of that question — an agent that files without discussing still has to file it
well — so the filing skill reaches the agent in all three positions, by name for `PluginDir` and as
text for `Inline`. `Auto` differs from `On` only in what it hands over for the brainstorming process:
a name for `PluginDir`, already loaded and costing one index line, against the absolute path to the
vendored `SKILL.md` for `Inline`, so a one-line change does not pay for 10 KB it will not use.

**Spec and Plan hang off it, and they cascade rather than sitting beside it.** They are the two
stages the filing session used to stop short of: writing down the design the discussion produced, and
writing the implementation plan (`superpowers:writing-plans`). Spec is a person's to choose only
while Brainstorming is `On`, and Plan only while Spec is — nothing for a design document to record
when no discussion happened, nothing for a plan to plan when no design was written. A stage nobody
may touch **reads as its parent rather than as a placeholder**, so the screen states exactly what
will be sent. The rule is `components/kanban/taskStages.js`, another of the `branchChoice.js` family,
and `agents::cascade` applies it again on the far side of the wire — not a duplicate to tidy away,
since what arrives there is a payload and a payload can carry a spec chosen under a discussion since
switched off. `prompt.rs` normalises before it writes any prose, so such a spec produces no words
about a spec at all. One `Stage` covers all three switches, matching `STAGES` on the front end, and
the collapse was the point: while Brainstorming had an enum of its own, a fourth position added to
`Stage` alone compiled perfectly and left the discussion switch a position short of its children.

The output is files, and the task is filed **last**: the design goes to
`.smetana/docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md` and the plan to
`.smetana/docs/plans/YYYY-MM-DD-<topic>.md` — superpowers' own layout moved under the folder
`runs/gitignore.rs` keeps out of the repository, so nothing is committed. Filing last means an
interrupted session leaves no card promising documents nobody wrote. The paths copied into the issue
are **absolute**, since an ignored file does not travel into the worktree `provisioning` cuts — and
the issue still has to say in prose what was decided, because the files are on one machine. Spec
needs no skill text of its own; Plan is its own skill and follows the trade Brainstorming's `Auto`
makes.

**What a filed task owes is set by the far end of the app, not by the dialog.** `provisioning` says
the description *is* the spec, and a description that never says what "done" looks like is not
something to start on — a thin task is not a smaller task, it is a supervised run stopping overnight
on a question or an automatic one parking the work. The two ends are held together by
`bd create --validate`, which refuses a description missing the sections its type requires
(`## Acceptance Criteria`, plus `## Steps to Reproduce` on a bug, `## Success Criteria` on an epic,
three headings on a decision, nothing at all on a chore). That flag is the whole mechanical part of
the standard, which is why `STANDARD` in `prompt.rs` names it in the prompt rather than leaving it to
the skill: an `Inline` harness may find no skill text to read. **It is a floor and not the standard**
— measured against the pinned sidecar it matches the wording of a heading and nothing else, so an
empty section, a `###` and lower case all pass. It converts "no acceptance criteria" from an
invisible default into something somebody has to do on purpose; judging whether the criteria are real
is `provisioning`'s job. `running-tasks` holds its own filing to the same skill and adds the test
that follows: a finding nobody can state acceptance criteria for is a digest line, not a task.

The other half is what the discussion produces. Brainstorming on `On` buys half an hour of narrowing
down what somebody meant, and none of it is anywhere but that conversation — the agent that picks the
task up months later has the person's original four sentences and nothing else. So `DISCUSS` requires
the outcome, rejected options included, to be written into the issue itself.

### Answering what a run could not

A task an automatic run could not settle is `parked` with the question written into its notes as a
`parked:` line — `runs::queue::parking_note` on the app's side, `running-tasks` when a lead does it
by hand. `ResolveTask` is the way back: an agent session that puts those questions to the person at
the terminal, writes what they answer into the issue, and unparks it.

The rules are `smetana:resolving-questions`, and three of them are in `prompt.rs` as well, for the
reason `STANDARD` is: an `Inline` harness may find no skill text at all. Ask one at a time and
**answer none of them yourself** — the task is parked precisely because guessing was not good enough
for the agent that stopped. The answers go into the **description**, because that is the spec
`provisioning` reads and a decision recorded only in the notes is one the implementer never sees, and
a `resolved:` line goes into the notes beside each `parked:` one. And the status is the **last**
write, the same rule filing keeps: a session interrupted halfway leaves the task parked rather than
back in the queue with the answer written nowhere.

The front end's half is `components/kanban/parked.js`, another of the `branchChoice.js` family, and
it holds the pairing rule the notes are read by: **everything below the last `resolved:` line is
still open.** Not a question matched to its own answer, which would need the two written in step —
a person settling three questions in one sentence writes one `resolved:`, and a positional pairing
would call two of them unanswered. The sequence is what is true instead, because a resolving session
answers everything open at that moment and only then unparks.

Three places act on it and they have to agree, which is why the rule is one pure file rather than
three conditions. A parked card's menu offers "Answer questions" first, above the play. The play
itself is dead there (`runnableTask` in `DesktopApp.vue`), and that is not tidiness: without it the
play is the way around the dialog, one row above it in the same menu. And moving a parked card to
Ready asks first, quoting the open questions verbatim — three ways out, `Move anyway` writing the
status exactly as the menu always did, with no note invented on the person's behalf. Only Ready
asks: Done decides the question no longer matters and Pinned takes the task off the queue, while
Ready is the one that hands it to an agent with the question still open.

### Attachments: pictures on a task nobody has filed yet

A screenshot is the fastest way to say what is wrong, so the new-task dialog takes images:
`src-tauri/src/attachments/`, `src/stores/attachments.js` and
`components/kanban/AttachmentStrip.vue`, plus the Storage tab of the settings window
(`components/settings/StorageSettings.vue` over the pure `settings/storage.js`). The Rust side is the
same no-worker shape as `files/` and `git.rs`, for the same reason — writing a couple of megabytes
guards no state — and it is four commands over pure functions that carry the tests: `mod.rs` is the
disk and the vocabulary, `cleanup.rs` is the whole of the deleting rule with no filesystem in it.

Three gestures put a picture in the list and they arrive as only two kinds of thing. A file already
on disk arrives as a path and Rust copies it (`attachment_import`); the clipboard exists inside the
page and nowhere this process can reach, so a paste arrives as bytes (`attachment_write`). Both
answer with the same record, which leaves the strip one shape to draw. The list lives in the store
rather than in the dialog because a drop is not the dialog's event to hear: Tauri intercepts file
drops before the webview sees them and reports them against the *window*.

**The bytes are copied, never pointed at.** They go into `app_data_dir()` and the path that reaches
the agent is absolute, because the case this exists for is a screenshot in `~/Downloads` that a
person throws away in a week and the link in the issue has to outlive that. Writing into the
repository instead would work in every clone and worktree, but only for files somebody committed, and
committing binaries into another person's tree is not this app's decision. The price is plain: in
somebody else's clone, and in CI, the pictures are not there.

There is no `resolve_within` here, and its absence is the design rather than an oversight.
`files/fs.rs` confines every path to the project root because everything it touches belongs to the
project; nothing here does — the *source* is whatever a person picked in the OS's own dialog or
dragged off their desktop. What is confined is the *destination*: always `app_data_dir()/attachments`,
under a name that is not the one that arrived. `stored_name` builds it from a timestamp and a `slug`
keeping ASCII letters and digits and nothing else, so no incoming name can climb a directory, hide
behind a dot or need quoting — that string ends up in a prompt, in a shell argument and in an issue
description. The extension comes from `sniff`ing the bytes, not from the name, so a JPEG somebody
renamed `.png` reaches the agent labelled with what it is.

Two numbers are deliberately not shared. `MAX_IMAGE_BYTES` is 8 MiB and is **not**
`files::model::MAX_FILE_BYTES`: that one is 2 MiB and answers how much text a textarea will open
without freezing the window, while this one answers how big a screenshot is, and a full-screen retina
PNG routinely lands between the two. A test asserts they are still different. The other is the copy
of that ceiling in `attachments.js`, which exists only so a file certain to be refused is not first
read into an ArrayBuffer and encoded a third larger again; drift there is not symmetrical, since
above Rust's is harmless while below Rust's makes every file between the two impossible to attach at
all. The front end's copy must never be smaller.

**The store is laid out by project, and that layout is the boundary the one deleting thing in this
app works inside.** A picture goes into `attachments/<key>/`, where the key is `cleanup::project_key`:
the folder's own name through the same `slug`, and the FNV-1a hash of the whole absolute path after
it. Three properties are wanted at once — derivable from the path alone, since nothing written down
anywhere can be lost; the same on every run, which is why the hash is written out here rather than
taken from `DefaultHasher`, documented as free to move between Rust releases and so able to strand
every picture under the old name; and safe as a single path segment, since this string is joined onto
the store's root and everything deleted is found by walking the result. The name half is for a person
opening the directory in Finder; the hash is what tells two projects called `app` apart.

**Nothing deletes on its own, at any moment.** Not on start, not on a schedule, not when the new-task
dialog closes on images nobody filed — taking a thumbnail out forgets the path and leaves the file.
The one thing that deletes is `attachments_clean`, at the end of a person's press on the Storage tab,
after `attachments_survey` has told them how many files and how many bytes it is about to take.

**What survives is what an unfinished task still names.** `cleanup::removable` is the rule, pure,
over a list of files and a snapshot of the board: a file whose absolute path appears in any of an
issue's four prose fields — description, acceptance criteria, design, notes — stays if that issue is
anything but `closed`; a file only closed issues name goes; a file nothing names at all goes, and
that third case is most of the rubbish and the reason the directory stops growing. The four fields
are deliberately more than the prompt asks for, because the agent decides where the link lands and a
field too many costs a file kept for nothing while one too few costs somebody's screenshot. There is
no record of which task a picture belongs to and there cannot be one: nothing comes back from
`bd create` saying which id it wrote — the same missing channel `claimedBy` reconstructs around.

**An empty board and an unreadable board are the same `Snapshot` and opposite facts**, and keeping
them apart is `cleanup::refusal` — the guard both commands ask before anything is listed. `open()`
resets the store and then ignores whether the first sync worked, so a worker that cannot reach bd
sits with a project open and an empty snapshot; `removable` reads that as "no task refers to any of
these files" and the sweep takes every attachment of every live task in the project. The ways in are
ordinary rather than exotic — no bd on the machine, a version mismatch, a damaged `.beads`, or a
folder with no tracker at all, which the app deliberately keeps open so `bd init` can be offered. So
`Request::Current` carries `Health` beside the snapshot, in the same message as the emptiness it
explains, and anything but `Ok` refuses with `NoBoard` — the rule `runs/browser.rs` sets for the whole
repository: anything unobservable reads as "no", loudly. The survey counts zero in that state rather
than counting everything as rubbish, because a number offering the whole folder under a button that
refuses to press is the same lie told quietly; the front end's `canClear` holds the button on the
same field, and treats a health word it has never heard of, or a missing one, as unread.

**The button reaches one project's folder and physically cannot reach another's.** The directory is
`store_root()/project_key(dir)` where `dir` comes from the tracker worker — `Request::Current`, which
answers with the folder being watched *and* the board it holds in one message, so the two cannot name
different projects across a switch. Everything deleted is that directory joined with a name out of
its own `read_dir`, checked once more by `plain_name`; no subdirectory is entered and no string from
the front end reaches the sweep. Reading every open project's tracker instead was refused: a project
closed in the list would still go unread, and its live tasks would lose their pictures while looking
like nobody's. The files in the root of `attachments/` from before the split are out of reach for
that reason and stay for good — they belong to no project, so there is no board to ask about them,
and they are finite. An attachment made while no project is open also lands in that root: the honest
place for it rather than a fallback, since the root is the part of the store nothing sweeps.

### The bell: what the app has to say right now

The bell in the scope bar opens a panel of notifications, and the badge counts what is in it —
`components/notifications/` (the pure `notifications.js`, `NotificationPanel.vue`,
`NotificationCard.vue`) over `src/stores/notifications.js`. There are two sources — the attachment
store growing, and a run that is over — and the badge counts one card per stopped run beside the one
the storage source is ever allowed.

**The list is derived, not an inbox.** A notification is computed from the state of its source and
thrown away when that state goes away; nothing accumulates on disk — no history, no message log, no
read/unread ledger, and the bell's own label says "1 notification" rather than "unread" for that
reason. A durable inbox was considered and dropped: everything this app has any use for announcing is
a statement about something it can look at right now, so a stored copy is a second source of truth
that goes stale the moment the first one moves. The cost is named rather than discovered: there is
nothing to say about the past, and a source that genuinely needs history brings its own storage. A
card stands until it is answered or stops being true, and every measurement under it rewrites its
prose from the size just read.

The one thing that survives a restart is a number per project — `storageWarnedMib` in
`settings.json`. **A threshold is announced once and arms itself again when the size falls back below
it**: after *every* measurement the remembered number becomes the highest threshold the folder still
reaches, so crossing 10 MiB says so once and stays quiet for the next 40, while cleaning down to
3 MiB clears the memory and the next crossing of 10 speaks again. Dismissing is the same write, which
is why there is no dismissed flag: there is nothing a second one could express that this does not.

The ladder is 10, 50 and 100 MiB, weighed against **the active project's subdirectory** of the
attachment store rather than the whole of it, and that follows from the Storage tab: the clean-up
button reaches this project's folder and nothing else, so a warning summing in a neighbouring project
and the unreachable files in the store's root would name a number a person cannot bring down.
Announcing every project's folder was dropped as well, though it is honest: the only action for
somebody else's folder is "switch project first", and it needs the stable project key mapped back to
a path.

The size comes from `attachments_survey` — **the same command the Storage tab reads**, never a second
one, because two commands measuring one folder eventually disagree and the screen a person is sent to
would argue with the card that sent them. `projectBytes` in `settings/storage.js` is the reading of
it: `kept` plus `removable`, and `null` — not zero — whenever the board could not be read, since a
zero taken as a size would announce nothing about a folder that may be full, re-arm the ladder off a
number nobody measured, and offer a Clean up button Rust refuses. So an unreadable board changes
nothing: no card made, none taken away, the remembered threshold left where it is. Freshness is the answer the file tree and
the branch already give — no watcher: at start once the project is resolved, on a project switch (in
`projects.js`, after the new layout has landed *and* after `tracker_set_project`, since the survey is
answered against the worker's idea of the active project), on window focus, and after an attachment
is saved.

Clean up opens the settings window **on the Storage section**: `settings_window_open` takes a `tab`,
a window being built gets it as `?tab=storage` on the URL it already loads, and one already open —
focused rather than rebuilt — is told by the `settings:show` event. That event lives in
`stores/app.js` rather than in `settings.js`'s three-event contract because nothing about it reaches
`settings.json`: the main window is still the only writer.

**The second source is a run that has stopped, and it is what tells anybody the night is over.**
`runNotification` beside `storageNotification`, one card per stopped run keyed `run:<token>`, derived
from `runsState.runs` and gone when the run leaves that list — a project switch, or a run of the same
scope replacing it. `syncRunCards` is called from the three functions in `runs.js` that cover every
assignment of the list, one of them transitively (`startRun`'s filter of a replaced scope's stopped
run, through the `upsert` on the line after it), which is the one place an edit landing between the
two would leave a card up for a run no longer in the list. It rewrites that source's half of `items`
and leaves every other source's cards alone. Which source sits above which is a property of the list
rather than of who spoke last: `SOURCES` declares the order and both writers hand their result to
`arrange`, runs above storage, because a night that has ended is what somebody came back to read
while a folder that has grown will still be there tomorrow.

The import between the two stores is circular by construction — `notifications.js` reads `runsState`,
`runs.js` calls a hoisted function declaration — and **nothing in `notifications.js` may read
`runsState` at evaluation time**, only inside `syncRunCards`. That is not a style rule: the bundler
emits `notifications.js` first, before `runs.js` has evaluated and before the `const` exists, so the
natural-looking improvement — a module-scope `watch(() => runsState.runs)` replacing the explicit
calls — would throw on the built app's first line and leave a white window, while working perfectly
in `npm run dev`, where the browser's module order is the other way round.

**Nothing about it reaches disk**: dismissing adds the token to an in-memory `Set`, and a run no more
survives a restart than a session does, so this source needs no equivalent of `storageWarnedMib`. The
token is issued once per app process and never reused, which is what makes one set safe across
projects.

The card is short on purpose — the ending, `N closed · M parked`, the duration, and one button — and
everything else is in the report the button opens; a card that restated the document would be the
right panel's question block all over again (smetana-s4f). The ending's sentence and glyph come from
`components/run/stopReason.js`, the same table the bar draws, and they go into the **body** rather
than into the title, since several of those sentences carry an em dash of their own and folding one
into a title after a second dash reads as two sentences run together. Every entry in `REASONS` names
its own glyph, and so does the answer for an ending this build has never heard of — while they did
not, the bar and this card each kept their own `?? 'square'`, drifting invisibly.

**An unread board is never a zero** — `summary.tasks` of `null` says the board could not be read
instead of "0 closed, 0 parked", and no `Show details` is offered when there is no document.
**A run carrying no summary at all is a third case, not that one**: nothing has failed to read
anything, nothing has looked yet. `request_stop` ends a run with nothing in flight at once and the
account arrives seconds later through `Run::take_summary_from`, so every press of Stop between
batches passes through this state on its way to the real counts — a card announcing a board failure
there would state a failure that did not happen. It says the ending and nothing else, and is still
announced at all because this front end may also simply be older than the worker.

`Show details` opens the report as an ordinary centre tab, through the very `openFile` the file tree
calls. The one translation is `reportTabPath` in `reportTab.js`: the summary's path is absolute
because a worker that knows nothing of tabs has to name a file on disk, `openTabs` is
project-relative, and separators are normalised on both sides since `files.js` uses `/` while Rust
wrote the platform's. It answers `null` rather than guessing for anything not squarely inside this
project's reports folder.

There are no toasts. The bell is the whole surface: a folder that has grown is not a person waiting
on an answer, a run that has finished is not one either, and the loud budget on that screen is one or
two rows.

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
| `registry.rs` | `.smetana/runs.json`: what a live run leaves on disk, and the rules for reading it — pure, and where those tests are |
| `procs.rs` | the process table and the two signals: the only `unsafe` in `runs/` |
| `recovery.rs` | the disk half of the registry, and the start-up sweep for what an unclean exit left running |
| `preflight.rs` | bringing the project up before the first batch — declared commands, then declared health checks |
| `usage.rs` | what the subscription has left, and whether to run at full size, a smaller one, or not yet |
| `browser.rs` | whether there is anything on this machine to drive a browser with — pure over file contents and directory listings, and where those tests are |
| `queue.rs` | what is left to do and whether to run another batch — pure, and where the tests are |
| `summary.rs` | what the run did, as a diff of the board between its first read and its last — pure, and where those tests are |
| `report.rs` | that summary and the batches' own accounts, rendered into a self-contained HTML document — pure, and where those tests are |
| `service.rs` | the worker: the loop, one run per scope per project |
| `commands.rs` | thin `#[tauri::command]`s, shaped exactly like the tracker's |

`service.rs` is the same single-tokio-task shape as the other two workers. The deciding is `queue.rs`
and that is pure; the map's own lifecycle — `absorb`, `permit`, `admit` and the browser-candidate
list — is pure too, and unlike the other workers this file carries a test module of its own for that
part, because both ways of getting the lifecycle wrong are silent. A project holds several runs at
once and the map is keyed by each run's `token` (smetana-5hf): what is refused is a second run over
the **same scope**, since two runs told to take the whole queue are two leads racing for the same
tasks, while a queue run beside a task run, or two runs over different epics, divide the board
between them. Which tasks each may touch is not this worker's question — bd's atomic claim under
per-session actors (smetana-4fh) is the exclusivity. A run in another project is none of this one's
business. The one thing all runs share is a subscription limit, and a run does not reserve one
(smetana-tra). The loop runs on a task of its own so the worker stays answerable while a batch runs
for an hour, and reports whole `Run` values back through a channel — the worker is the only thing
that ever writes one out. The `token` does the job `generation` does for the tracker: a stop names one
run by it, every `run:state` event carries it, and a late report from an ended run finds no entry
rather than the run that started after it.

**Stopping is cooperative, and that is a decision with a cost attached.** `request_stop` sets a flag
and the loop reads it between batches; the batch in flight is allowed to finish, because a run
interrupted between a merge and a close is exactly the state the recovery phase exists to clean up. A
run with nothing in flight stops at once, which is what lets the stop button reach a paused one.
`StopReason` keeps `Cancelled` and `SessionRemoved` apart: both are somebody's doing and neither is a
crash, but pressing stop let the batch finish while removing the session killed it where it stood,
and the person reading the bar is deciding whether to go and look at what got left behind.

**A map entry outlives the run it holds, and that is what makes "one run per scope" true**
(smetana-0kb). It leaves in exactly one place — `Report::Ended`, sent by a `Drop` guard when the loop
task is gone however it went — so "there is an entry" and "a loop task is alive" are one fact rather
than two that agree most of the time. Removing the entry the moment a stop declared the run over
looked equivalent and was not: the loop was still between reading the board and spawning, so it put a
batch out that nothing could then stop. The spawn itself is **asked for rather than checked**:
`may_spawn` puts the question on the channel the worker's own `select!` already drains, so the same
single task decides it and handles `Request::Stop`. That is not a FIFO guarantee, but the two can
never interleave, so **both orderings are safe**: stop first and the spawn is refused, spawn first
and the stop that follows finds a batch in flight and waits for it. Yes records that batch as in
flight (`Active.starting`, the fact `Run.session` cannot carry yet).

A stop leaves a gap between the run reading `Stopped` and its entry leaving, and the **refusal in
that gap has its own reason**, `RunError::WindingDown`. Reusing `AlreadyRunning` put two
contradictory things on screen at once — a bar saying the run is stopped and a message saying one is
going — which a person reads as the stop not having taken. The gap is not always brief: the loop may
be inside a board read or a 60s usage probe, and it holds its scope for the whole of it — only its
scope, since the rest of the project's runs were never this one's to hold.

Every declared command and every health probe the preflight starts is given the **login shell's**
`PATH`, from the same `shell_env` the terminal uses and for the same reason: a bundled app inherits
launchd's, which holds nothing a person installed, so `docker compose up -d` exited 127 against
infrastructure that was up and answering — and the one phase whose whole job is to name the missing
piece named the wrong one. `shell_env::path` falls back to the inherited value, so this is never a
narrowing.

**The preflight is the one phase where a stop is not cooperative** (smetana-16w). `bring_up` read the
stop channel nowhere at all, so a stop pressed during it waited out every declared command at 600s
apiece and every health check at 120s — on this project the first declared command is `npm install`.
It now watches that channel: the command in flight is killed where it stands, and a check is given up
between looks rather than during one, since a look is bounded by seconds of its own where a command
has nothing bounding it but the ceiling. Killing is safe here for the reason it is refused between
batches: a declared command brings infrastructure up and is run again from the top next time. The
signal goes to the process group, because the child is a shell and the work is what it started. The
ending is unchanged. Two smaller rules hold that up, both found by driving the race rather than by
reading it: `may_start_batch` refuses a run that is merely `stopping`, not only one already over,
since "the batch in flight finishes" has always meant that one and no more; and a report from the
loop is **adopted, not assigned** (`Run::adopt`), because stop is asked for on the worker's side and
never travels to the loop task, so taking the loop's copy wholesale unasked the stop a moment before
the check that reads it.

`queue.rs` is a port of `holiday-curb`'s `loop-state.mjs` with one substitution that changes its cost
and not its logic: the source shelled out to `bd ready` and `bd list` between every batch, about four
seconds each, while this reads the snapshot the tracker worker already keeps current. It tracks
`unfinished` — `in_progress` and `ready_to_merge` — separately from `ready`, because `bd ready` hides
both and a run watching only the ready set would leave a killed batch's orphans on the board forever.
A dependency counts as blocking only when it is bd's `blocks` kind. And `LastBatch` has three answers
rather than "did it crash", because a batch stopped by a spent allowance moved the board no more than
a crashed one did — reading either as a stuck queue would end a run over nothing — while a harness
that keeps falling over needs a person and an exhausted allowance needs only time.

`usage.rs` is the piece the runs design deliberately left out and then took back. Reading
`claude -p "/usage"` is a parse of somebody else's prose that can break silently, which is why it was
refused; what did not survive contact was the trade, since a run that exhausts its allowance
overnight spends five sessions and a minute of backoff discovering it and then stops with `Crashed`,
which says the harness kept failing when nothing failed. So the parse is back with its failure mode
named rather than assumed: **an unreadable answer never blocks a run** — it reads as `Normal`, the
batch goes at full size, which is where things were before the module existed. The gate runs *before*
each batch, so the exhausted case costs no session at all. `service.rs` asks the same question again
after a session exits non-zero, and there it is not a gate but a classification: a spent limit told
apart from a harness that fell over, from the one source of truth.

`browser.rs` answers the question the config could not: `[live_check].mode = "browser"` says what the
*project* wants and nothing about the machine the run rides on, so a run with the live check on
started happily where there was nothing to drive a browser with and found out inside the check, as
INFRA (smetana-29s). Either tool is enough — Playwright, which is two facts and not one (an MCP entry
in `~/.claude.json`, the project's `.mcp.json` or `~/.codex/config.toml`, **and** the browsers
actually downloaded under `ms-playwright`), or the Claude in Chrome extension, found by its id in a
Chrome profile. Every path and id in it is fragile by nature, and that is accepted rather than
hidden: an extension writes itself into no agent's configuration, so the unpacked directory is the
only evidence there is. Hence the rule the whole file is built on — **anything unobservable reads as
"no", loudly**: the toggle goes off and the tooltip names what was not found, rather than staying
live on a guess. Matching an MCP entry goes the other way on purpose (its name *or* what it runs,
either alone), because a false "present" leaves things where they were before the module existed
while a false "absent" takes a working feature away under a tooltip claiming a tool is missing that
is sitting right there.

Busy-ness is the second reason and deliberately only half a question. `Request::BrowserBusy` answers
which projects have a live run that asked for a live check, counted per run and including the asking
project, since a live-check run in this very project is what holds Playwright's one profile against a
second run beside it; `browser_tools` then reads each candidate's config, because the worker knows a
run wanted a check and not whether that project's check opens a browser, and naming a `command` check
as the reason would be an invention. **The extension's busy-ness is out of reach entirely, and so is
a browser a person is driving themselves.** So busy-ness may block **only where Playwright is the
tool that would be used**, which means the extension is absent — letting the branch fire whenever
*either* tool was present disabled the toggle on an extension-only machine over a tool nobody had
shown to be held. The sentence a person reads is composed on the front end
(`components/run/browserTools.js`, pure and tested, one of the `branchChoice.js` family), since it is
UI copy; the scope is `browser` and nothing else.

A pause is a `RunState`, not a `sleep` inside the loop, and that is load-bearing twice over: a run
that had simply gone quiet for three hours is indistinguishable from one that hung, and the bar is
where somebody looks to tell those apart — and being a state is what lets the stop button reach it,
since a paused run has no session in flight. `resets` is the harness's own sentence about when the
allowance clears ("Aug 11 at 5:59pm (Europe/Moscow)"), passed through untouched and never turned into
a moment in time: that would be a second parse of the same prose, and its failure would be a run that
woke at the wrong hour.

`config.rs` refuses to load a damaged file, the **opposite** of `settings/model.rs` and opposite for
the right reason. There, a broken section loses itself and the cost is a forgotten panel width; here
it would be a run whose gates quietly went missing and whose green merges therefore proved nothing —
hence `deny_unknown_fields` throughout, since a typo has to be louder than a silence. `runs::service`
is the first and only place a damaged config is shown to anybody; everywhere else in the app it reads
as "no configuration", which is right for a marker on a row and wrong for starting a run. The file is
declarative where the work is mechanical and prose where it needs judgement — `hazards` stays as text
the lead reads, because two branches emitting the same migration number off one base is not a
pattern, it is a thing to look for.

`gitignore.rs` keeps `.smetana/` out of the repository, and it is code rather than a line in the
setup skill on purpose: an instruction in prose can be followed, argued with or quietly skipped, and
this one was all three — an agent reading a `.gitignore` whose neighbouring lines hide the tracker
and the docs can reasonably conclude either way, and the answer then differs from project to project.
The app decides once, in code. `amend` is pure and carries the tests; it treats `.smetana`,
`.smetana/`, `/.smetana` and even the negation `!.smetana` as already covered, that last one because
it can only have been typed on purpose.

### The run's own account of itself

A run used to end saying one word — `Queue empty`, `Crashed`, `Cancelled` — and that was the whole of
what the app had to say about however many hours it just spent. `summary.rs` and `report.rs` are the
other half: the app keeping its own record and writing it out as an HTML document under
`.smetana/reports/YYYY-MM-DD-HHMMSS.html`. Timestamped rather than keyed by the run's `token`,
because that counts from zero on every app start and would collide across restarts, and nothing ever
deletes one — they are small text, and deciding when a record of a night's work stops mattering is
not this app's call. One second is not one run, though, since a project holds several at once
(smetana-5hf), so `claim_report` *makes* the file with `create_new` and walks a `-2`, `-3` suffix
rather than checking whether the path exists: two runs are two loop tasks, so the creation itself has
to be the exclusive step.

**Three facts about what the app can know decide the whole shape.** It can see the board and its own
clock, so *which* tasks moved and *how long* the run took are its to work out. It cannot see what was
*done* — nothing comes back from a session but an exit code, the same missing channel `claimedBy`
reconstructs around and `SessionWork::Run` refuses to invent — so the lead is asked for it: one JSON
file per batch at `.smetana/runs/<token>/batch-<n>.json`, named in the `Run` prompt and in
`running-tasks`. And it cannot see per-task time: a batch may hold several tasks with no signal at
either end of one of them, so a task gets a duration of its own **only when its batch held exactly
one**, where the two are the same number and nothing is inferred.

Attribution is a **board diff**, not an actor match: a task is this run's when it is `closed` now and
was not `closed` at the baseline, the first board read inside the loop, after the preflight.
`queue::claimed_by` misses two real cases — an orphan Phase R recovered from a *previous* killed run
carries that dead run's actor, and an epic closed in Phase 3 was never claimed by anybody — so the
diff's own cost is taken instead: a task a person closes by hand in another window during the run is
credited to it. The report's scope is deliberately wider than `queue::in_scope`: an epic run reports
the epic itself, since Phase 3 closes it, and the priority floor is not applied, since it decides
what may be *taken*. The merge lock is excluded through `queue::is_lock` rather than a second copy of
the label.

`RunSummary.tasks` is an `Option`, and that is the point of the type: `None` means the diff could not
be computed — the run died in the preflight so there is no baseline, or the final board read failed —
and it is **never** rendered as "0 closed, 0 parked", the same rule `projectBytes` and
`cleanup::refusal` keep. A batch that left no file, or an unparseable one, is likewise named in the
document as having left no account of itself rather than drawn as an empty row, while its tasks still
appear from the board.

**Every ending the loop task reaches goes through one `finish(...)` in `service.rs`, and that
consolidation is the feature.** A dozen exits into `RunState::Stopped` is how the next ending
somebody adds quietly arrives with no report behind it — so `finish` is the only thing that ever
makes a `RunSummary`, and `advance` clears the field on every transition that is not `Stopped`, which
makes "`None` in every state but `Stopped`" a property of the type rather than a habit. `finish`
reads the board once more through `fresh_board`, for the ~2 s resync the run's own last writes need.
`did` is agent-written text going into a document a person opens and is HTML-escaped without
exception.

**The loop is not the only thing that reaches `Stopped`, though.** `request_stop` ends a run with
nothing in flight *at once* — which is what makes the button immediate and lets it reach a paused one
— so for a stop landing between batches, on a run waiting out a spent allowance overnight, or during
the preflight, the worker's copy is already `Stopped { Cancelled }` by the time the loop looks at the
channel. The loop then runs `finish`, writes the document correctly, and reports a run `absorb`
refuses, because nothing revives a stopped run; left there, the file sat on disk while the `Run` on
the wire said there was none. So `Run::take_summary_from` is `adopt`'s narrow opposite number: from a
report about a run this side has already ended it takes **the summary and nothing else**, once, and
emits the result. The ending deliberately does not travel with it — somebody pressed stop and was
told `Cancelled`, while the loop may have got as far as finding the queue empty a moment later, and
rewriting the reason under them would put a different run's story on the bar. Neither property the
map rests on moves: the stop is still immediate, and an entry still leaves in exactly one place.

### What an unclean exit leaves, and who clears it

Everything a run knows lives in memory, and sessions are deliberately kept out of `settings.json`
because a session row with a dead process behind it is worse than an empty list. The orderly ending
is `RunEvent::Exit`. A crash, a force quit, a `kill -9` and — in development — every Rust rebuild
reach none of it, and what they strand is tasks claimed by a run that no longer exists and agent
processes nobody will signal. This is the same shape as the window-geometry defect `window.rs` was
written for, where the only write happened at `Exit`.

**The app writes a registry and deals with processes; the tracker half stays with
`smetana:running-tasks` Phase R.** The split follows what each half can see: the app can see the
process table and the tracker cannot, and Phase R already recovers claimed tasks correctly with the
worktrees in front of it. So the app never rewrites `in_progress`, never parks anything, and writes
to bd nowhere as part of recovery — doing both would be a second mechanism doing Phase R's job, and
two mechanisms on one fact drift. The registry is `.smetana/runs.json` in the project folder, beside
`project.toml` and outside the repository, so Phase R reads it with an ordinary file read, needing
nothing from the app and no path passed through a prompt — which a file in `app_config_dir()` could
not be, being platform-dependent and findable by a skill only if the app told it.

**A record proves its own liveness, and an actor id alone cannot.** `BEADS_ACTOR` is
`smetana-run-<session-id>` and session ids restart at 1 on every launch, so after a restart a fresh
session takes a dead run's name. Every record therefore carries a `writer` — the app process that
wrote it — as a pid *plus* that process's start time, which is what survives pid reuse; each batch
carries its actor and its process group. Nothing in the file is read as a date to decide liveness:
the one timestamp says when the run began and ages a record out after a week. The stamp is read per
platform in `procs.rs` (macOS `proc_pidinfo`, Linux `/proc/<pid>/stat` against `btime`) rather than
through a crate, since `libc` is already here for `killpg`; a platform that cannot answer keeps no
registry at all, because a record nobody could ever show stale is worse than none.

At start-up the run worker sweeps every project the settings file lists as open, before it serves its
first request — one writer, so the read-modify-write is safe, and no batch can go out beside a sweep
about to hang up a leftover agent in the same worktree. For a record whose writer is provably dead it
signals the recorded process groups exactly as a clean exit does. **Anything the registry does not
name is never touched** — the app cannot show it started it — and neither is a group whose pid has
since been reused, nor anything under a writer that is alive or unreadable. The sweep is silent: the
app is finishing its own interrupted shutdown rather than taking a new decision, and a modal about
housekeeping after every rebuild would be the loudness budget spent on the opposite of a card needing
a human. What was killed goes to the log.

The record itself **outlives the processes on purpose**, for up to `ABANDONED_DAYS`: its actors are
the evidence Phase R reads, and deleting it the moment the processes were dealt with would send that
half of the recovery back to leaving every claim in place.

A record is removed when its run's **loop task ends — however it ended**: `Report::Ended` comes from
the same `Drop` guard the worker's map leans on, so a cancellation, a crash and a failed preflight
all take the record with them with none of them enumerated anywhere. The one condition is the
processes rather than the reason. `runs::service` ends a run with `NeedsAnswer` **without killing the
session** — the person is being sent to that terminal to answer — so `registry::forget_run` keeps a
record that still names something running, trimmed to the batches actually still there; deleting it
would leave a live agent, still claiming under its actor, named nowhere, and a `kill -9` a minute
later orphans exactly the process this file exists to reclaim. Conditioning on the stop reason
instead would have been a `match` somebody has to remember to extend. `smetana:merging`'s 60-minute
lock staleness rule is untouched by all this and cannot be replaced by the registry: the file names
runs this app started on this machine, while the lock can be held by a lead somebody started by hand
in a terminal.

On the front end, `runs.js` is deliberately small — a file read with no worker behind it, freshness
from switching projects, from window focus, and from any of the project's sessions starting or
stopping work. It keeps the back end's `config` and `Run` objects **whole** rather than unpacking
them into flags, the same instinct `tracker.js` follows with statuses: a state this front end has not
heard of must not silently read as one it has. The runs ride as a set keyed by `token`, so a late
word about one run can never write over another. It is guarded against its own stale response exactly
as `git.js` and `terminals.js` are, and the `run:state` listener carries that guard in its other form
— an event is not a response to anything, so a batch ending just as somebody moves project would
otherwise post its run under the new project's name. `RunBar` draws one segment per run in the scope
bar, each stop button naming its own token, and keeps a stopped run there until the project changes
or a run of the same scope replaces it: the reason it stopped is what somebody came back to read, an
unknown reason is an ordinary outcome rather than a crash, and the endings differ by glyph as well as
by colour. The scope rule itself is `components/run/runScopes.js`, one of the `branchChoice.js`
family and shared with the worker's `admit` by vocabulary rather than by code.

That third freshness channel is `components/run/configFreshness.js`, another of that family, and the
only one that fires while somebody sits and watches a setup agent write `.smetana/project.toml` —
they never leave the window, so focus never returns and no project switch happens. `workingKey` is a
value over the set of the project's sessions that are still `starting` or `running`, and a `watch` on
it re-reads the file on **both** edges, so a session going idle, picking up again and then exiting
costs two reads rather than one — the frequency to weigh before touching this channel. The mark
clears on a read that came back `ok`, never on the optimism that a session ended. What it replaces
was a watcher created inside `startSetup` over a single session id, which tore itself down for good
on its first callback for another project or for a session already gone, so a window that never
switched project and never lost focus kept the "Not set up for runs" triangle over a configuration
that existed, and kept the board's play buttons hidden behind the same `configured` (smetana-0ag).
The width is the fix: a key over a set cannot be lost, and it is scoped to one project. That the key
is a **string** is what keeps the two wholesale reassignments of `terminalState.sessions` quiet — an
unchanged set of working sessions produces an unchanged key and no read at all.

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
`EXPAND_PULL` reopens it. Double click resets to the shipped 252/340.

`RAIL` is the one width in the app that does **not** grow with the app-wide font size, and it cannot:
these pure functions do arithmetic with it — a collapsed neighbour's cost, both drag thresholds, the
clamp against the stored width — so a scale-dependent rail would have to be threaded through every
one of them. What sits in it does grow, though, and that was a real defect: the expand button is an
`IconButton size="sm"` at `--control-h-sm`, which reaches 44px at the top of the range and hung over
the board beside it. So the button is capped rather than scaled — `min(var(--control-h-sm),
RAIL_CONTROL_MAX)`, which leaves both densities exactly as they are at the shipped size and stops the
growth at the rail's edge. `Panel.vue` takes both numbers from this file now; it used to write the 32
out a second time. When the window is too narrow to honour both a panel's minimum and the board's
floor, the panel keeps its minimum and the board takes the squeeze — the board's content scrolls, a
file tree at 90px does not.

`Resizer` diverges from the design system in behaviour, not in styling — pointer capture, so a
release outside the window still ends the drag; `user-select: none` on the body for the duration; and
arrow keys, which its `role="separator"` had been promising with nothing behind them. Those belong
back upstream.

### The column order, and what of the board is drawn

`components/kanban/columnOrder.js` says it plainly: bd owns which columns exist, the settings own
only the sequence, and this file is the reconciliation between them — pure, no Vue and no DOM, which
is what makes it the one part of the reordering a test can reach. The stored order is per project,
because the set of statuses is: bd carries custom ones and one repository's status has no meaning in
another's order.

A stored order is a **hint, never the truth**. A status bd no longer has cannot be conjured onto the
board by a line in a settings file, and a status bd grew since the last visit has to appear even
though nothing stored names it. So columns the stored order knows are drawn in its sequence and the
rest go after them in bd's own order — appended rather than dropped, since a column nobody has
arranged yet still holds issues, and appended rather than slotted back into bd's position, since
there is no honest position left once the neighbours have been moved by hand. Names matching nothing
are passed over rather than pruned, so a custom status deleted and recreated finds its old place.

`moveColumn` returns the very array it was given, by reference, when nothing moved — an out-of-range
index, or a move to where the column already is. The caller leans on that identity to tell "nothing
happened" from "something did" without comparing contents.

**Which of those columns are drawn, and which of their cards, is the second question and a separate
file**: `boardView.js`, over the global `kanban` settings. `DesktopApp.vue` composes the two —
`orderColumns` first, then this — and the order is deliberate, since the sequence of the columns is a
property of the whole board and must not depend on which of them happen to be on screen today, or a
column would come back from a hidden spell somewhere it never was. Both settings default to today's
behaviour exactly, and its two closed lists are the doubling against `settings/model.rs` the Settings
section names.

`columnHelp.js` is the third of that family and holds what a column *means* — the sentence a person
gets after holding a column head for two seconds. It is deliberately not a line beside the glyphs in
`status/status.js`: that file is the design-system layer and answers what a status *looks* like,
while "which tasks end up here" is knowledge about this board and this project's way of working —
runs, parking, findings that turned up during a review. Two questions, two files, and nothing in the
tooltip explains bd, because a person hovering a column head is asking about their tasks.

Moving a whole deferred column into the queue is `PromoteColumnModal.vue`, the one bulk write to the
tracker in the app. The count is the entire content of the question and sits in the title, because
there is no undo — putting a task back is one issue at a time in the inspector — and it is a snapshot
taken at the press rather than a live reading, since a number that moved between being read and being
confirmed would describe a set nobody agreed to. Each issue costs about two seconds, so a column of
twenty is most of a minute: the dialog owes progress rather than a spinner, and afterwards how many
landed and how many did not.

`components/run/branchChoice.js` is the next of that family and was pulled out for the same reason:
a `.vue` file is the one thing no test in this repository can reach, so the whole of the rule filling
the run dialog's branch field lives outside the component. `pickBranch` is three steps in one order —
what this project was left at last time, then its own `[defaults].target_branch`, then whatever the
list puts first, which is the most recently worked-on branch because `target_branches` orders by
reflog. A remembered name that is no longer in the list is skipped in silence rather than offered,
since a branch deleted since it was remembered would sit in the field as an option that fails on the
first merge. The list itself holds `{ name, missing_in }` records rather than bare strings:
`needsCutting` is the single rule behind both the field's hint and the run's `create_target`, and
`branchOptions` is what splits the two groups the field draws.

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
of date, and somebody picking a since-deleted branch inside that window has the choice frozen by
`branchChosen`, so the run goes out against a branch that is not there. Clearing first made that
impossible — by keeping Run disabled every time, for everybody.

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
which would have drawn an empty gap in the middle of a tooltip's sentence. Borrowing a store's copy
instead of lifting it out would have pulled Vue and Tauri into a family defined by having neither.

`src/appearance.js` sits beside it for the same reason: what a stored theme means right now, and what
factor a chosen font size comes to, are wanted by two windows at once — the app and the settings
window — so there is no one part of the interface to file them under. It is deliberately small: the
sizes themselves are the stylesheet's, and this file only says by how much. Its DOM half is split off
into `views/useAppearance.js`, which is what keeps the rules themselves reachable by a test.

### Settings

What the app remembers between runs lives in one JSON file in `app_config_dir()`
(`~/Library/Application Support/com.invisor.smetana/settings.json` on macOS).
`src-tauri/src/settings/` owns it: `model.rs` is the schema, the validation and the merge — pure,
and where the tests are; `file.rs` is the disk (atomic write through a per-call temp file that is
`sync_all`ed and renamed, a `.bak` copy of anything unparseable or too new); `commands.rs` is two
thin commands.

At the root the file keeps appearance — theme, density and `uiFontSize` — panel layout (collapsed
state and width for each side), `editor` with its own `fontSize`, `agent`, the id of the CLI agent to
start, `agentLanguage` and `taskLanguage`, the two languages that agent works in, and `kanban`, how
the board is drawn. Below that, `openProjects` is the list of projects the window has open,
`lastProject` is the one active when it last closed, and `projects` is a map from each project's
absolute path to its content state (side tab, active tab, selected task, selected path, expanded
folders, `openTabs`, `previewTab`, `columnOrder`, `runSettings`, `storageWarnedMib`, `usedAt`).

`kanban` is the one that is **global rather than per project**, and deliberately: `columns` (`all` or
`some`) with `alwaysShow`, and `interval` (`all`, `day`, `week`, `month`) with `unlimited`, are a
person's way of reading a board rather than a fact about one repository, and the defaults are today's
behaviour exactly — every column, every task. The rule it feeds is `components/kanban/boardView.js`,
whose two closed lists are written out there and again in `model.rs`: the doubling `SIDE_TABS` and
the storage ladder carry, with the same obligation — what the front end offers must be a subset of
what Rust accepts, or the value loses itself on the next save with nothing on screen to say so.

The per-project three are per project for the reason the rest are: a status has no meaning in another
repository's column order, a branch name has none in another repository, and the attachment folder
the bell weighs is a different folder for every project. The ladder `storageWarnedMib` is validated
against is a closed list written out twice as well, in `model.rs` and in
`components/notifications/notifications.js`; a value off it loses itself and costs one repeated
warning. `runSettings` is what the run dialog opens on next time, a mirror of
`runs::model::RunSettings` **minus the scope** — its own type rather than a reuse, since this one
lives in a file people edit by hand and has to tolerate anything while the other crosses the IPC
boundary and must not, and without the scope, since that comes from whichever play button was pressed
and remembering it would open the dialog claiming to run something nobody clicked. The open tabs are
paths relative to the project root, so a moved folder does not turn the list into rubbish. The map
never crosses the IPC boundary: `settings_load` returns the resolved view for one project and
`settings_save` puts it back, stamps `usedAt` and trims `projects` toward the 20 most recently used —
never evicting the current project or anything still in `openProjects`, so the cap only bites entries
from past visits that were closed.

The front end owns the truth here — the opposite of the tracker, where bd owns it.
`src/stores/settings.js` holds a reactive object and writes it back with a 400 ms debounce, one
write in flight at a time; components read and write plain fields. Closing the window does not wait
for the debounce: the store holds the close through `onCloseRequested`, flushes with a two-second
ceiling and then destroys the window itself — the window always closes, a slow back end costs the
last edit rather than the app.

Most of the file is still only ever changed by *using* the app: a dragged panel, a switched project,
an opened tab. A handful of fields are the exception and they are what the settings window edits —
`appearance.theme`, `appearance.uiFontSize`, `editor.fontSize`, `agent`, the two languages beside it,
and the four `kanban` fields. Density is not among them, deliberately: nothing has asked for it yet,
and a screen full of switches nobody wanted is worse than a short one. `?theme=` and `?density=`
still override the first two for one run and are deliberately **not** written back — one visit to the
dev server must not repaint the app forever. `?view=gallery` neither reads nor writes.

#### The settings window

The gear in the scope bar opens a **second `WebviewWindow`**, not a modal (`window.rs`:
`settings_window_open`), and that is the whole of why this is a window: a modal cannot be dragged
outside the app's own bounds, so it cannot sit beside what it is changing. It loads the same bundle
under `?view=settings`, so there is one front end and one set of tokens; the label `settings` is what
makes a second press focus the window instead of making another, and it is also the name the
capability in `capabilities/default.json` lists beside `main` — a window not named there reaches no
core plugin at all, and the settings UI would come up unable to send an event or read the version.

It is built as a **child of the main window** (`parent`), which keeps it in front of the thing it is
changing: without it, the first click on the board buried the settings behind the app, and the only
way back was the gear that opened it. `parent` says exactly that and nothing wider, in each
platform's own words — an owner window on Windows, transient-for on Linux, a child `NSWindow` on
macOS — where `always_on_top` would float over every other application on the machine, and an app
somebody has switched away from has no business sitting on top of their browser. The price on macOS
is that a child moves when its parent moves; it can still be dragged anywhere, including clear of the
app, which is the whole reason this is a window and not a modal.

**The main window stays the only writer.** `settings_save` writes the whole resolved view — panel
widths, project map, open tabs — so a second window calling it would post its own idea of all of
that, and the later write would win. So the settings window holds no settings store: it asks
(`settings:hello`), it is told (`settings:state`), and it sends one edit at a time
(`settings:apply`), which lands in the main window's reactive object and reaches disk through the
debounce every panel drag already uses. `stores/settings.js` owns all three, and `applyPatch` is
where an event is checked — a field that fails takes its previous value, not the shipped default,
because an event is not a response to anything and a malformed one must cost nothing. The settings
window applies an edit locally *before* sending it, so a dropdown answers in the same frame and the
announcement that follows is the correction when a value was refused. It also follows that this
window cannot outlive the main one, so `close_settings_with_main` takes it down on the main window's
`Destroyed`, which is also what lets the app still exit on its last window.

Appearance reaches the screen through the document root and nothing else (`views/useAppearance.js`).
`theme: system` is not a third palette — it is the absence of a choice, so the word is stored as it
stands, never resolved on the way to disk, and `prefers-color-scheme` is *watched* rather than read
once: a laptop that switches at sunset must not leave the app wrong all evening.

`uiFontSize` is **a factor in the stylesheet, not a set of sizes in JS**, and the difference is the
whole of why this works. `paintRoot` writes exactly two custom properties — `--ui-scale` (the chosen
size ÷ 13) and `--text-code-size` — and `tokens/typography.css` defines each of its eight steps as
`calc(<n> * var(--ui-scale) * 1px)`. Computing the eight sizes in JS and writing them onto the root
works and quietly kills the stylesheet, since an inline custom property beats every rule in a file:
editing a step there would then change nothing on screen with every gate still green. A factor is
also what keeps the hierarchy — moving only the semantic aliases flattens it at every size but the
default. `tokens/space.css` carries the same factor on **the heights and nothing else** (`--row-h`,
the `--control-h` set, `--tab-h`, `--titlebar-h`, `--scope-bar-h`, `--icon-*`, in both densities),
because compact's 22px `--row-h` would clip text that reaches 22px at the top of the range; the
`--space-*` scale deliberately does **not** scale, since padding and gaps are the rhythm of the
interface rather than a container for a glyph. `tests/styles/tokens.test.js` reads both files and
pins all of it.

Three consequences worth knowing. `editor.fontSize` sets `--text-code-size`, the one step the factor
does not reach (chrome and code are two questions) and also what `CodeBlock` and `LogLine` draw with,
so the editor setting moves them too. The terminal was handed a resolved *number* rather than a
token, so it re-reads on the `data-ui-font` attribute `paintRoot` stamps for that purpose — and it
cannot read that number out of `--text-xs`, since the computed value of an unregistered custom
property keeps its `calc()` unevaluated, so `terminal/theme.js` measures a throwaway element whose
`font-size` is the token. And **icons do not scale**: the `--icon-*` tokens are referenced nowhere
and the `Icon` call sites pass numeric literals, so glyphs stay put while their labels grow.

The tabs are `components/settings/` — the directory is the list, for the reason the note under
Commands gives — and each is presentational, handed values and emitting what was picked, so the whole
window renders in `?view=gallery` too. The sections themselves are a closed list in
`SettingsWindow.vue` alone (General, Editor, Agents, Kanban, Storage, About); Rust guards the *shape*
of a `?tab=` name so nothing can smuggle a second parameter into the URL, never its vocabulary, and
an unknown section opens on General.

Every list on them is `Dropdown`, and with that **`Select` is drawn nowhere outside the gallery any
more.** Its bargain — one element, accessible for free — buys a menu the operating system paints, in
its own colours, font and row height, none of them reachable by a token and none following the theme,
the density or the app-wide font size this very window exists to change. `Select` stays in the
library because it is not broken; nothing in the app reaches for it. The switch also turned up the
defect a short list had been hiding in `Dropdown`: its options are flex items in a column, so past
the eighth the `--row-h` height became a starting point and fifteen rows shared the ceiling at 15px
each instead of scrolling at 28. `flexShrink: 0` is the fix, and a list that genuinely scrolls then
needed `reveal`, the cursor's row brought into view on opening and on walking off either end.

Agents is the one place in the front end that ever *names* an agent: the ids are still `agents::IDS`
and Rust still drops one it does not ship, so this is a set of labels for ids Rust already knows. The
two language pickers under it are the same doubling against `agents::LANGUAGES`, accepted for the
same reason — Rust validates the ids, so drift costs a stale label rather than a lost setting — and
all three rows share one control column, wider than the shipped default because `Dropdown`
ellipsises a label that does not fit and "Chinese (Simplified)" is the longest either list holds. The
subscription block under it is a placeholder with dashes and a sentence saying so: invented numbers
under a real setting would claim the app knows something it does not, which is what the fixture log
pane was removed for. Kanban is the same shape one tab over, and the one tab whose lists are not a
closed vocabulary at all — the columns it offers are the active project's own, read from the tracker,
so with no project open or no answer yet it says so rather than drawing an empty list.

**Storage is the one tab that is not a setting**, and it is the exception that keeps the rule
readable. Nothing on it reaches `settings.json`: it asks Rust what the attachment store weighs
(`attachments_survey`) and, on a press, tells Rust to sweep the active project's folder
(`attachments_clean`) — two commands about the app's own data directory, so the main window is still
the only writer of settings. It is read when the tab is opened rather than when the window mounts,
since the answer queues behind the tracker worker, which may be two seconds into a bd call, and the
other tabs have no use for it. The window hands over no path and no project, which is what leaves the
deleting confined in Rust; the component takes the survey whole in Rust's own shape and
`settings/storage.js` — pure, tested, another of the `branchChoice.js` family — turns it into the
sentence somebody reads before an irreversible press.

About's link goes out through `tauri-plugin-opener` (`opener:allow-open-url`, scoped to
`https://github.com/*`): inside this webview it would replace the app with a web page, and there is
no address bar here to come back from. Which branch `openExternal` takes is decided **before** the
call and not by catching its failure, because the two failures are opposites: in the app a rejected
`openUrl` is the ACL doing its job, and falling back to `window.open` would navigate to exactly what
the scope declined. The predicate is "is there a real back end", which is **not**
`window.__TAURI_INTERNALS__` — `mockIPC` sets that property itself, so the obvious test reads true in
the dev server and quietly took the app's branch there, leaving the link opening nothing at all.
`mockBackend.js` publishes what it decided (`usingMockBackend`), the only honest answer.

A missing file is the first run, not an error. A broken or too-new file is copied to
`settings.json.bak` and the app starts from defaults. One that cannot be read at all — wrong
permissions, a directory in its place — has nothing to copy, so it is logged *and* `settings_save`
refuses: overwriting a file nobody could read would destroy it sight unseen. Damage is contained
field by field where it can be: a field whose *value* is outside its allowed set loses that field,
while a section whose *type* is wrong (`{"layout": {"leftCollapsed": "yes"}}`) fails to deserialize
and loses the whole section to its defaults, the same for one project entry among many. A file
written before the list existed carries `lastProject` and no `openProjects`; reading it makes the
list that one project, so an update does not open to an empty panel. That leniency lives on the
file-reading side only — an empty list from the front end means the last project was closed on
purpose and stays empty.

The side-tab set is a closed list written out twice, in `model.rs` and in `views/DesktopApp.vue`.
Changing one without the other is silent: the value survives the session and comes back as Files.

Window size and position are not in this file: `tauri-plugin-window-state` handles them, and
`src-tauri/src/window.rs` is the one thing added on top. The plugin keeps geometry in memory and
writes it to disk in exactly one place — `RunEvent::Exit` — so any run that does not reach a clean
exit leaves the last run's geometry behind, and the symptom (a window opening at the configured
1440×900 whatever size it was left at) is invisible from the front end, since `settings.json` keeps
saving on its own debounce the whole time. So `persist_geometry` subscribes to `Resized`/`Moved` and
saves 500 ms after the last one — a debounce that is not only about disk traffic, since it also
settles handler order: the plugin's own listener has long since updated the cache by then.

### Tests

`tests/` mirrors `src/`, and `vitest.config.js` merges the app's Vite config so the alias and the
Vue plugin come along. Two decisions are load-bearing, and both are explained where they live.

The mock boundary is the IPC transport, not the Tauri modules: `listen` and `emit` are themselves
`invoke('plugin:event|…')` calls, so a delta in a test is delivered by a real `emit` through the same
`initTracker` the app runs. `tests/support/ipc.js` also calls `mockWindows`, without which
`getCurrentWindow()` throws, `settings.js` reads that as "we are in a browser" and the window-close
path silently never registers.

Stores are module singletons holding more state than they export, so `tests/support/stores.js`
rebuilds the whole graph per test with `vi.resetModules()`. It hands back `nextTick` from that same
fresh graph: `resetModules` recreates `vue` too, and another instance's `nextTick` drives another
scheduler, so a test awaiting it would wait for a tick that never comes.

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

Three exceptions, and exactly three. The first is `components/files/editor/theme.js`: CodeMirror
renders its own DOM and the only way to reach it is CSS rules, so this one file is allowed to produce
them through `EditorView.theme()`. The rule is narrowed, not lifted — every value inside is still a
`var(--token)` reference, and no `#hex`, no `px` and no gradient belongs there.
`@codemirror/search`'s own theme paints a `linear-gradient` onto its panel buttons; `theme.js`
suppresses it explicitly (`backgroundImage: 'none'`), because gradients are forbidden everywhere in
this system, including inside a third-party stylesheet that ships its own opinion.

The second is `components/terminal/theme.js`. xterm.js renders its own DOM too, but its API differs
from CodeMirror's in a way that matters: `EditorView.theme()` takes CSS, so `var(--token)` works
there and the browser repaints on its own, while xterm.js takes an `ITheme` of **resolved colour
strings**, so tokens have to be read with `getComputedStyle` and handed over as values. The
consequence has no parallel in the editor — flipping `data-theme` does **not** repaint the terminal
for free, which is why `TerminalView.vue` carries a `MutationObserver` on the root's attributes. The
rule is narrowed the same way: every value still comes from a token.

The third is `src-tauri/src/runs/report.rs`, the widest of the three because its output is not this
app's screen at all: it renders the run report, a self-contained HTML document somebody opens in a
browser with nothing of ours loaded, so there is no stylesheet around it and a token reference would
simply be an unresolved variable. What replaces the rule rather than lifting it: no external
stylesheet, no font off a network, no script and no image — the document reaches nowhere at all,
which is also what makes it safe to hand to a sandboxed frame.

`styles/styles.css` is an `@import` list only; the tokens live in `styles/tokens/`. `tokens/base.css`
holds element defaults (focus ring, selection, scrollbar) and the only three global classes in the
system (`.sm-mono`, `.sm-hatch-blocked`, `.sm-scroll-hidden`).

The first line of `base.css` is `box-sizing: border-box` on everything, and the whole system rests on
it. Components declare a size as a token and add padding and a border on top — `width:100%` with
`padding:0 var(--space-4)`, `height:var(--control-h)` with a border, `width:8px` with a 1.5px ring —
which only comes out right under border-box. The React design system gets it from its own reset; the
port did not carry it over at first, and the cost was `Input` overflowing `Modal` by exactly
`2×--space-4 + 2×--border-w` and `StatusDot` drawing its single `size` prop as three different glyph
sizes. Both vanished with the one line. Do not remove it, and do not "fix" a component by subtracting
its own padding from its width.

### Theme and density live on the document root

Both are attributes on `document.documentElement` (`data-theme`, `data-density`), set by a
`watchEffect` in each view. Every token is defined against them: `tokens/color-*.css` redefine
colours under `[data-theme="dark"]`, and `tokens/space.css` redefines *only* the space scale and
row/control heights under `[data-density="compact"]` — density never changes colour, radius or type.

The root carries one more thing in the same spirit, and it is a value rather than a switch:
`--ui-scale`, the app-wide font size as a factor, which the type scale and the row and control
heights are written in terms of. See the settings window above for why it lives in the stylesheet
rather than in JS.

### `status/status.js` owns colour and loudness

The single source of truth for what a status looks like and how loud it is:

- `RESERVED` = `blocked, ready, running, needs-you, done, failed`. These get fixed
  `--status-<name>-*` tokens and a distinct glyph from `STATUS_GLYPH`.
- Anything else is user-defined: `normalizeStatus` → FNV-1a hash → one of 12 generated slots
  (`--status-gen-<0-11>-*`), hues chosen to stay outside a guard band around every reserved hue.
  Generated statuses also render a 2-letter `statusCode` — **status is never colour alone**.
- `attentionLevel(status)` returns `loud` / `live` / `quiet`; components set `data-attention` and
  dim `quiet` to `--attn-quiet-opacity`.

Between the two sits `EXTRA_GLYPH`, and it is a third thing rather than a hole in the first two: a
status that takes a **generated hue and its 2-letter code, but not the generic tag glyph.** Three of
bd's built-in statuses fall outside `RESERVED` and reach the board as user-defined (`deferred`,
`pinned`, `hooked` — clock, pin, anchor), and beside them are the names a project's own way of
working keeps producing: `parked`, `ready-to-merge`, and `human-check`, the column for work that is
merged and closed and still owes somebody a look. `parked` borrows `needs-you`'s triangle
deliberately, since a parked issue is one somebody has to come back to and the two never share a
board. `human-check` deliberately does **not** borrow it: it draws a person with a tick, because the
column is about the person and that has to be visible, while `needs-you`'s loudness is budgeted at
one or two rows on a screen and this column holds a dozen cards at a time — so it stays an ordinary
`live`. A status nobody has heard of is still an ordinary outcome and draws the generic tag.

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
tree-shakes to the ones actually used — the file is the list, and a number written here would be
wrong within a month. Adding a glyph to the UI means adding it there first; `Icon` warns in dev for
an unregistered name. Swapping icon sets means replacing that one file. Note
`message-circle-question-mark` is kept as the design-system key and mapped to lucide 0.469's
`MessageCircleQuestion`.

### Adding a component

Create it under the matching `src/components/<group>/`, export it from `src/components/index.js`
(the library's public surface), and add it to `views/Gallery.vue` so it stays checkable. Product
code imports from `index.js`; components import their siblings by relative path — the `@` → `src`
alias exists in `vite.config.js` but is currently unused, so prefer relative paths for consistency.

## Constraints

- **No gradients, images, glass, blur or emoji.** Partly taste, partly the WebKitGTK constraint.
  One raster is drawn in the whole interface and it is the app icon itself, on the About tab
  (`src/assets/app-icon.png`): the exception is the artwork rather than the medium, since this
  picture is the app's identity and a version redrawn from tokens would be a second copy to keep in
  step with the first. It carries its own black ground and squircle, which lets one file serve both
  themes with no border and no radius from the component. Anything else wanting a picture is still a
  design-system question. `scripts/make-app-icon.py` builds it and the bundle icons from one source,
  so the two cannot drift; `app-icon.png` at the repository root is that 1024 master.
- Sentence case everywhere; identifiers in mono (`--font-mono`), prose in sans.
- The primary button is ink on paper with no brand hue — the entire saturated range belongs to
  status.
- The build target (`es2021`, `chrome100`, `safari15`) is set for the system webviews Tauri runs in
  (WebKitGTK / WKWebView / WebView2). Do not raise it, and do not reach for APIs newer than that.
- `tokens/fonts.css` `@import`s IBM Plex Mono from Google Fonts; an offline Tauri build needs the
  latin subset vendored locally instead.
