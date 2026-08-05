# Stage 4 — the interface

Spec: `docs/superpowers/specs/2026-08-05-runs-design.md`. Stages 1–3 are on
`feature/runs-project-config`; this continues in the same branch and finishes the feature.

This is the stage that makes stage 3 verifiable: until there is a button, nothing can start
a run, so the end-to-end check belongs here.

## Global constraints

- Comments, test names and UI copy in **English**, sentence case. Commit messages Russian.
- **No scoped CSS, no classes.** Every visual value is a computed style object of
  `var(--token)` references. A new component follows that shape or it is wrong.
- A new glyph is registered in `core/icons.js` before it is used, and every new component
  is exported from `components/index.js` and added to `views/Gallery.vue`.
- Components never import Tauri. `stores/runs.js` and `stores/git.js` are the boundary.
- `.vue` files carry no tests; they are checked by eye through `?view=gallery` in all four
  theme × density combinations. Pure logic and stores are tested.

## What the person asked for, restated

A play button beside the "+" in Ready, and on cards that can be run on their own: a task
with no epic parent, and an epic. Pressing one opens a dialog with the target branch, the
mode, the priority floor, the live check and whether findings may be filed.

## File structure

| file | new? | what it does |
|---|---|---|
| `git.rs` | edit | `git_branches` — the local branch names, read off disk like `head` |
| `stores/git.js` | edit | `loadBranches`, `gitState.branches` |
| `settings/model.rs` | edit | `ProjectState.run_settings` — what the dialog opens with next time |
| `stores/settings.js` | — | nothing: it holds whatever the file has |
| `components/run/RunModal.vue` | new | the dialog |
| `components/run/RunBar.vue` | new | the run's segment in the scope bar |
| `components/kanban/ColumnHeader.vue` | edit | a `runnable` prop and a `run` emit |
| `components/kanban/TaskCard.vue` | edit | the same, on cards that can be run alone |
| `components/kanban/KanbanColumn.vue`, `KanbanBoard.vue` | edit | relaying both |
| `views/DesktopApp.vue` | edit | the scope, the dialog, starting and stopping |
| `views/Gallery.vue` | edit | both new components, four states each |
| `components/index.js`, `core/icons.js` | edit | the exports and the glyphs |

## Task 1 — `git_branches`

`git.rs` reads `.git` directly rather than spawning git, and this keeps to that: local
branches are file names under `.git/refs/heads/` (recursively — `feature/x` is a
directory and a file) plus whatever `packed-refs` holds, which is where git puts a branch
it has packed and where a freshly cloned repository keeps nearly all of them. Reading only
the directory would show a fresh clone one branch, which is the defect this note exists to
prevent.

```rust
pub fn parse_packed_refs(contents: &str) -> Vec<String>   // pure, tested
pub fn branches(project: &Path) -> Vec<String>            // disk
#[tauri::command] pub fn git_branches(project: String) -> Vec<String>
```

Sorted, de-duplicated, `HEAD`'s branch included even when nothing else lists it. A folder
outside git answers with an empty list — the same "nothing here is an error" rule the rest
of the file keeps.

Tests: a packed-refs file with its `# pack-refs` header, an annotated tag's `^` line
ignored, tags not returned as branches, a nested branch name reassembled with its slash.

## Task 2 — remembering what the dialog was set to

`ProjectState` grows `run_settings: Option<RunSettings>` — mode, target branch, priority
floor and the two switches. **Not the scope**: that comes from what was pressed, and
remembering it would open the dialog claiming to run something the person did not click.

Per project rather than at the root, for the reason the column order is: a branch name has
no meaning in another repository.

Tests in `settings/model.rs`: a file with no `run_settings` loads (it is `None`, and every
existing settings file is that file); a `run_settings` whose mode is not one of the three
loses the field rather than the section, the same leniency every other single value gets.

## Task 3 — `RunModal.vue`

Props: `open`, `scope` (`{ kind, id, title }`), `branches`, `defaults` (from the config),
`remembered`, `live-check-available`, `busy`. Emits `close` and `confirm` with the whole
`RunSettings`.

Five fields, in this order:

1. **Merge into** — a `Select` of `branches`. First value: the remembered one if it still
   exists, else `[defaults].target_branch`, else the current branch. A branch that is
   remembered and gone is silently dropped rather than shown as a broken option.
2. **Mode** — Auto / With a lead / On its own. **The third is offered only for a single
   task that is not an epic**, matching `RunSettings::validate`, which refuses it
   otherwise: the dialog and the model agree because the model is what says no.
3. **Take tasks down to** — P0…P4, defaulting to `[defaults].min_priority`.
4. **Live check** — a `Switch`. Off and disabled when the config says `mode = "none"`,
   with one line saying the project declares no way to check — a switch that silently
   does nothing is worse than one that explains itself.
5. **File what it finds** — a `Switch`, with one line saying such tasks go to `deferred`
   and wait for a person. That sentence is the whole reason the setting exists and it
   belongs on screen, not only in the skill.

Below them, one line naming what will run: "12 ready tasks", "this task", "6 children".
The dialog is the last point at which a person can see they aimed at the wrong thing.

## Task 4 — the buttons

`ColumnHeader` takes `runnable` and emits `run`; the play sits before the "+", so the
"+" keeps its position and nothing a person aims at moves. Only the ready column gets it,
the same way `addable` is already decided by the board.

`TaskCard` takes `runnable` and emits `run`. The board decides what is runnable: an issue
with no parent, or one whose type is `epic`. **A child of an epic gets no button** — it
runs as part of its epic, and offering it alone would let somebody merge half an epic
without meaning to.

The button's box is always in the layout and only its visibility changes on hover, the
rule `ProjectList` already keeps: a control that appears on hover must not reflow the row
it appears in.

`play` is registered in `icons.js` already (the log toolbar uses it). `square` for stop
needs adding.

## Task 5 — `RunBar.vue`

The run's segment in the scope bar, drawn only while there is a run. What it says:

- `Preflight` / `Working — batch 3` / `Deciding` / the stop reason.
- `Stopping after this batch` when `stopping` is set and the run is not over. The whole
  point of the cooperative stop is visible here or nowhere.
- A stop button while it runs; nothing once it is over.

A stopped run stays until the project changes or another starts — the reason it stopped
is the thing a person came to read. `queue_empty` reads as "Done", and everything else
says what happened in one line, because the four unhappy endings need different responses
and a single word for all of them would send somebody to the wrong place.

## Task 6 — wiring in `DesktopApp.vue`

The scope comes from what was pressed; the dialog is opened with it; `confirm` calls
`startRun` and switches to the agents side tab and the terminal centre tab, exactly as
filing a task and "Ask agent to edit" already do — a run is agent sessions, and watching
them is the point.

A refusal from `run_start` is shown in the dialog rather than swallowed: a broken config
names the section, which is the other half of smetana-8av.

Branches are loaded when the dialog opens, not on every project switch: it is a directory
read, but it is one nobody needs until they are looking at the field.

## Task 7 — gallery, exports, and the end-to-end check

Both components in `Gallery.vue` in enough states to be worth looking at: the dialog for a
queue and for a single task (the mode list differs), the bar working, stopping, and
stopped for each reason. Then the check by eye in all four combinations, and finally the
one thing no subagent and no test can do — `npm run tauri dev`, press play, watch a batch
start.

## Order

1 and 2 in either order, then 3, then 4 and 5, then 6, then 7.

## Out of scope

Everything already filed: pausing on subscription limits (smetana-bvn), a second
concurrent run (smetana-tra), creating a branch that does not exist (smetana-cfm), pushing
after a run (smetana-5sc), reserved colours for the new statuses (smetana-9ld).
