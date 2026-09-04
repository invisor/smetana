---
paths:
  - "src-tauri/src/tracker/**"
  - "src-tauri/src/project.rs"
  - "src/stores/tracker.js"
  - "src/views/folderAccess.js"
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
| `access.rs` | whether the folder can be read at all, and the one repair for when it cannot |
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

**The worker holds one project, and two of its requests name a folder anyway.** `Request::BoardAt`
and `Request::UpdateAt` carry a `dir`, and they exist for the one caller that is not the app window:
a run lives for hours against the folder it was started in, so every read and write it made was
answered about whatever project somebody had switched to meanwhile (smetana-ynyc, and
`.claude/rules/runs.md` carries the night it cost). Where `dir` is the folder the worker already
holds — `access::same_dir`, which is the literal spelling and then `resolved`, so a symlink is not a
second project — they are the ordinary paths: the live store, `full_sync` behind `fresh`, and a
write through the worker's own `Bd` and `finish` so the delta goes out and the board redraws.
Where it is any other folder the answer is a one-off `Bd::new(app, dir)` and the store is not
touched, there being nothing on screen showing that board — and health is not touched either, since
it belongs to the project the person has open. `BoardAt` says which half answered
(`BoardSource::Cache` or `Direct`), which is what the run's journal writes as `via=`. This is
deliberately **not** a worker per project: the watcher, the cache, `close_merged` and health all
stay bound to the selected folder.

Health (`ok`, `no-project`, `not-a-beads-repo`, `bd-version-mismatch`, `folder-refused`, `error`) is
both an event and a command:
the event fires microseconds after start, before the webview can subscribe, so the worker also
answers `tracker_health`. `DesktopApp.vue` renders it where the board would be — quietly, since the
loud budget belongs to the card that needs a human.

### A branch somebody merged by hand closes its task

On the same sixty-second tick, right after the full sweep, `service::close_merged` closes every task
in **`ready_to_merge`** whose branch is already in the project's target branch. A task merged through
the app is closed by the agent that merged it, as `merging`'s last step; a person who merges the
branch themselves closes nothing, and the task then sits on the board for ever while the work is in
the target branch — the case this was written for had it deployed to a staging environment
(holiday-curb-a769). bd cannot see it either: `bd orphans` looks for the id in commit messages, and
the merge was a **fast-forward**, so no commit anywhere names the task.

Hence the predicate: **the tip of the task's branch is an ancestor of the target's**
(`vcs::merged::is_ancestor`, `git merge-base --is-ancestor`), never the presence of a merge commit.
Ancestry sees the ordinary merge, the squash landed as one commit and the fast-forward alike. The
branch itself is `git::task_work`'s — the id in the last segment of the name — which is refs read off
the disk with no process, so a task nobody cut a branch for costs no spawn. In a project of several
repositories a task closes only when its branch is merged in **every** repository that has it, and a
repository without the branch does not hold the closure up: merged in the backend and outstanding in
the frontend is half-finished work. The whole of that rule is `vcs::merged::merged_in_all`, pure and
tested; `merged.rs` exists at all because `git.rs`, which finds the branch, is forbidden a process.

Three narrownesses, and each is a way of being wrong that costs the work rather than a minute.
**Only `ready_to_merge`** — an `open` or `in_progress` task may have a branch with the same slug, half
merged or cut for another attempt. **Only local refs** — nothing asks a remote, so no timer ever
carries a network call, and a branch merged only on somebody's server is not merged on this machine.
**Only a target branch this project has actually named**: what the run dialog was last left on here
(`settings.json`, per project), then `[defaults] target_branch` in `.smetana/project.toml`. Those are
`branchChoice.js`'s first two terms and deliberately not its third — falling back to "the branch most
recently worked on" is a fair guess in a field somebody is looking at, and a bad one behind a sweep
nobody sees.

The close carries a reason naming the branch, the short sha of its tip and the branch it reached, so
a closure nobody performed says where it came from. A failed close is a log line and never health:
nobody asked for this write, and the task stays where it is for the next tick to try again. This does
not replace `merging`'s own closing step — that still closes a task the moment it merges it, and this
picks up what went past the app.

### A refused folder is not a broken tracker

`folder-refused` exists because the two used to arrive as one state (smetana-8lq). A build opened on
`error` — "bd is failing, most often the tracker was made by an older bd", with a button that runs a
database migration — while the file tree beside it said the truth: no permission to read the folder.
The cause was macOS TCC, the project sat under `~/Desktop`, and no prompt could appear because a
stored refusal is the one thing macOS does not ask about twice. bd failed for exactly that reason and
had no way to say so.

`tracker/access.rs` is where the two are told apart, and it asks **the filesystem**, never bd's
prose: `ErrorKind::PermissionDenied` on the project directory or its `.beads` is the fact, and bd's
wording is bd's and moves between releases. `refusal` is checked in `open` before `has_tracker` —
`has_tracker` is an `is_dir` and macOS lets a `stat` through while refusing the `read_dir`, so
without that order the notice would offer `bd init` over a `.beads` nobody may open — and again in
`HealthReporter::failed`, which is why that method takes the folder the call was made in.

The repair is `tccutil reset <service> <identifier>` and a restart, and it is offered **only where
macOS will actually ask again**. TCC prompts per protected place — Desktop, Documents, Downloads, a
mounted volume — and everything else on the disk is governed by Full Disk Access, which it never
prompts for at all. So `tcc_service` answers `Option`, `None` for anywhere outside those four, and
nothing ever passes `SystemPolicyAllFiles` to `tccutil`: pressing there would clear a grant the
person did give, with no dialog left to ask for it back, and a folder outside the four is as likely
refused by an ordinary unix mode as by TCC.

**A folder has two spellings and both are asked.** A path can be a symlink out of a protected folder
or into one, and both are ordinary: with iCloud "Desktop & Documents Folders" on, `~/Desktop` is
itself a link into `~/Library/Mobile Documents/…`, while a checked-out project can be a link the
other way. So `service_for` asks the literal path first and the canonicalized one second — resolving
`home` alongside `dir`, so both sides of every `starts_with` are spelled alike — and takes the first
promptable service either names. Choosing one spelling cannot cover both cases, because they are
symmetric; asking both is safe because *every* service either spelling can name is one macOS will
prompt for again, so a wrong hit costs one dialog and nothing irreversible.

`service_for` is the **only** place a folder is resolved, and that is the point of it: the read that
draws the notice and the write that runs `tccutil` have to agree. They did not once, and both
directions of the disagreement were this task's own defect in miniature — a button that refuses
itself under a sentence promising a prompt, and a sentence sending somebody to grant Full Disk Access
for a folder a Desktop dialog would have covered.
`repair_for_agrees_with_what_reset_would_do` is that invariant as a test.

`tracker_access_repair` is the read the front end asks before drawing anything, and it answers per
**folder** rather than per build — `reset`, `full-disk-access`, `unavailable` — so the store asks it
again on every project switch. The three sentences are `views/folderAccess.js`, tested there; only
the first has a button. The identifier is read from `app.config().identifier` rather than written a
third time beside `tauri.conf.json` and `runs/awake.rs`.

Two things the reset shares with `updates_install`, and it is the same reasoning in both places
(`.claude/rules/updates.md`). It is **gated on live runs anywhere**, refusing while any project holds
one and refusing again if the run worker cannot be reached — or does not answer inside five seconds,
since a worker that is alive and wedged is a third silence and silence is not permission — because a
restart kills every PTY child
and under a run those are the agents — and the run this would end is always somebody else's, since
the project on screen cannot be read at all. And it calls `request_restart`, never `restart`, so the
exit event fires and those children are killed rather than orphaned. Rust restarts the app itself: a
reset only takes effect for a process that has not already been refused in this launch, so the
restart is the second half of the repair rather than a courtesy — and the button says so, since
there is no confirmation dialog.

There is a second way into this state, and smetana-fkt is what stopped it happening again: a grant is
bound to the code requirement, and under an ad-hoc signature that is a cdhash which changes with
every build, so an in-place update silently invalidated whatever was granted. Signed with a Developer
ID the requirement is the team and survives the update. The reset stays, because the releases before
it were ad-hoc signed and a grant already lost that way is not repaired by the signature — and
because the first way in, somebody once answering "Don't Allow", is untouched by any of this.
`.claude/rules/updates.md` is where the signing itself is written down, and `RELEASING.md` names the
last release that went out without it.

### Repair: fixing without diagnosing

Under `error` that empty state carries **what bd itself said** — the last non-empty line of
`health.message`, in the `detail` slot `EmptyState` grew for it — and two buttons. "See the console"
used to stand there instead, which was an instruction for whoever wrote the app addressed to whoever
uses it, while the app held bd's own words and threw them away. `TrackerError::Command` now carries
the argument list as well as the code, so that line names which call failed — the methods on `Bd`
are the list, and it is not the length it was when this was written.

**Repair tracker** copies `.beads` to `.beads.backup-<UTC>` beside it, runs `bd migrate` and then
`bd migrate schema`, and reopens the folder — which is what brings the board back with nothing for
the front end to ask for. Failing to take the copy **stops the repair**: the copy is the entire
reason there is no confirmation dialog in front of the button, and `TrackerError::Backup` says in
its own sentence that nothing was migrated, and that refusal goes through health like the
migration's own, so the line under the board says why rather than quoting the last `bd list`.
Nothing ever removes a copy — a migration is the one irreversible thing this app does to somebody's
tracker. The copy earns its name by a rename from `<name>.partial`, and symbolic links inside
`.beads` are followed, bounded by a canonicalised ancestor chain rather than skipped: a copy
silently missing whatever a person parked behind a link, under a toast naming the backup, is the
failure this whole screen exists to stop.

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
English messages — the raw text is in the console, and under `error` and under `folder-refused` it
is on the screen as well (there it is the path that could not be read, and bd said nothing about
it).
`projects.js` owns the list of open projects, which one is active, and moving between them — the
front end holds the list's truth, bd holds the board's, so a switch reads the new project's layout
with `settings_load` (only the layout: the list on disk is already the past by then) before it asks
the tracker to point at the new directory — plus offering `bd init` in a folder that has none yet.

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
that rang the bell lit the tile loud while the footer's counter, filtering
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
