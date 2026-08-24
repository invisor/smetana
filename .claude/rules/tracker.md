---
paths:
  - "src-tauri/src/tracker/**"
  - "src-tauri/src/project.rs"
  - "src/stores/tracker.js"
  - "src/stores/projects.js"
  - "src/components/shell/ProjectRail.vue"
  - "src/components/shell/ProjectTile.vue"
  - "src/components/shell/monogram.js"
  - "src/components/shell/projectState.js"
  - "src/components/shell/projectMenu.js"
---

# The tracker bridge

The board shows the **bd** issue tracker of the active project's directory — chosen from the project
list, and remembered between runs — and follows it as it changes, no matter who changed it: this
window, an agent, or a person in a terminal. bd has no daemon and no API — its CLI is the API, and
one call costs about two seconds. Hence the shape of `src-tauri/src/tracker/`:

| file | what it does |
|---|---|
| `model.rs` | `Issue`, `ColumnDef`, `Delta`, `Health`, `Repair`, `Failure`, `TrackerError` — the vocabulary the front end sees |
| `bd.rs` | the only file that knows bd's CLI: arguments, spawning, parsing |
| `backup.rs` | the copy taken before a migration: its name, and a recursive copy in `std::fs` |
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

That panel's body is read-only — rewriting a title or a description is an agent's job, and "Ask agent
to edit" starts one on the issue. What acts on an issue is one menu, `components/kanban/taskMenu.js`,
and it has two triggers: the card's own on the board, and a three-dot button in the header of the
Task & details panel itself (`views/DesktopApp.vue`), built from the same card so that the two cannot
come to offer different things. Its "Move to…" submenu offers three of bd's eleven statuses (Ready,
Pinned, Done); the rest belong to agents, so the one the issue actually holds is appended as a fourth
option when it falls outside those three — a list with nothing checked in it reads as an issue
holding no status at all. That rule and its `STATUSES` live in `taskMenu.js` rather than in either
trigger, because two copies would have drifted the first time bd grew a status. Delete is
`bd delete -f` — irreversible, and `-f` is not about skipping a prompt: without it bd prints a
preview, deletes nothing and exits zero.

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

### Repair: fixing without diagnosing

Under `error` that empty state carries **what bd itself said** — the last non-empty line of
`health.message`, in the `detail` slot `EmptyState` grew for it — and two buttons. "See the console"
used to stand there instead, which was an instruction for whoever wrote the app addressed to whoever
uses it, while the app held bd's own words and threw them away. `TrackerError::Command` now carries
the argument list as well as the code, so that line names which of the six calls failed.

**Repair tracker** copies `.beads` to `.beads.backup-<UTC>` beside it, runs `bd migrate` and then
`bd migrate schema`, and reopens the folder — which is what brings the board back with nothing for
the front end to ask for. Failing to take the copy **stops the repair**: the copy is the entire
reason there is no confirmation dialog in front of the button, and `TrackerError::Backup` says in its
own sentence that nothing was migrated. Nothing ever removes a copy — a migration is the one
irreversible thing this app does to somebody's tracker.

It is offered for **any** tracker failure, and that is a decision rather than laziness. bd offers no
structured verdict about its own database: `bd doctor` answers "not yet supported in embedded mode",
and `bd migrate --json`, `bd migrate schema --json` and `bd migrate --inspect` all ignore the flag
and answer in prose with tick marks in it — measured on the pinned 1.1.2. So a health state meaning
"this tracker is too old" could only be entered by grepping that prose, and a recognizer that quietly
stops matching on the next bd release is exactly how the caught bug reached a person. Both migrations
are idempotent by bd's own documentation, so running them against a tracker broken some other way
costs about four seconds and changes nothing. Do not re-open this without new measurements; the
rejected alternatives — a state of its own, `bd backup`, `bd export`, a `--dry-run` confirmation
dialog — are recorded on smetana-j7o.

**Ask an agent** is the second button, for the failure the migrations did not fix. It reads
`tracker_failure` — the folder, `EXPECTED_BD_VERSION`, the bd command line that last failed and its
stderr, as **one** answer — and starts an `Intent::RepairTracker` carrying all four. One call rather
than four reads, and complete at the moment it is sent, because the tracker is what is broken: the
agent cannot ask bd anything afterwards. That is the `ResolveConflict` shape and deliberately not the
`ResolveTask` one.

`tracker.js` also owns the two translations: bd's statuses to the design system's (`open → ready`,
`in_progress → running`, `closed → done`; everything else, including custom statuses, passes through
to `normalizeStatus` and gets a hash colour with a 2-letter code), and Rust's diagnostics to short
English messages — the raw text is in the console, and under `error` it is on the screen as well. `projects.js` owns the list of open
projects, which one is active, and moving between them — the front end holds the list's truth, bd
holds the board's, so a switch reads the new project's layout with `settings_load` (only the layout:
the list on disk is already the past by then) before it asks the tracker to point at the new
directory — plus offering `bd init` in a folder that has none yet.

## The project rail

Projects are a 44px strip of 28×28 monogram tiles down the far left
(`ProjectRail.vue` over `ProjectTile.vue`), not a list of rows. Two letters come
from `monogram.js` — the first letter of each of the first two segments, or the
first two characters of a name with only one — and collisions are tolerated
rather than resolved: two projects called `smetana` under different parents both
draw `sm`, and mangling the second one's letters produces a label that means
nothing to anybody. The full name is in the tooltip, where somebody who is
unsure looks.

The dot in a tile's corner is the whole reason the rail exists: which project is
waiting on you, seen without switching to it. It reads `projectStates` from
`stores/terminals.js`, where loud wins over live, and it is the one place in this
system where two hues on an 8px circle carry a difference — `prefers-reduced-
motion` takes the live dot's pulse away and leaves nothing but colour. That is
paid for in words rather than left standing: the tooltip reads
`<name> · <branch> · <state in prose>`, and the prose is `projectState.js`'s,
which is also what the panel header's summary line is built from, so the two
cannot drift.

Those words name **no noun** — "1 waiting on you", not "1 agent waiting on you".
That began as a constraint: `SessionMark` carried no work kind, so
`projectStates` counted a person's own shell exactly like an agent, and a shell
that rang the bell lit the tile loud while the scope bar's counter, filtering
through `isShellSession`, read 0 a few pixels away. The mark carries a `kind`
now (smetana-low) and the map drops shells by it, so the two counts are about
one population again and the sentence *could* name agents. It does not: the copy
is unchanged until somebody decides otherwise, and the map still says nothing
about which agent. `projectState.js`'s header carries that as the standing note.

Nothing else fits on a tile. The three verbs a project row used to carry — set
up for runs, new agent, remove from list — are in the tile's secondary-click
menu, `projectMenu.js`'s items unchanged, and the menu is now the only door to
them. The three warning marks moved the other way, into the left panel's header
beside the project's name, where there is room for a glyph and a sentence; they
are drawn for the selected project only. The one of them that is true of an
unselected project — no bd tracker — is the tooltip's fourth segment, which is
why `projectRows` carries `tracked` for every row and not just the active one.
