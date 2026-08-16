---
paths:
  - "src-tauri/src/tracker/**"
  - "src-tauri/src/project.rs"
  - "src/stores/tracker.js"
  - "src/stores/projects.js"
  - "src/components/shell/ProjectList.vue"
  - "src/components/shell/projectMenu.js"
---

# The tracker bridge

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

`tracker.js` also owns the two translations: bd's statuses to the design system's (`open → ready`,
`in_progress → running`, `closed → done`; everything else, including custom statuses, passes through
to `normalizeStatus` and gets a hash colour with a 2-letter code), and Rust's diagnostics to short
English messages, with the raw text left in the console. `projects.js` owns the list of open
projects, which one is active, and moving between them — the front end holds the list's truth, bd
holds the board's, so a switch reads the new project's layout with `settings_load` (only the layout:
the list on disk is already the past by then) before it asks the tracker to point at the new
directory — plus offering `bd init` in a folder that has none yet.
