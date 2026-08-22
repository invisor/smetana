---
paths:
  - "src-tauri/src/settings/**"
  - "src-tauri/src/window.rs"
  - "src-tauri/src/autostart.rs"
  - "src/stores/settings.js"
  - "src/components/settings/**"
  - "src/views/SettingsWindow.vue"
  - "src/appearance.js"
  - "src/sounds.js"
  - "src/views/useAppearance.js"
---

# Settings

What the app remembers between runs lives in one JSON file in `app_config_dir()`
(`~/Library/Application Support/com.invisor.smetana/settings.json` on macOS).
`src-tauri/src/settings/` owns it: `model.rs` is the schema, the validation and the merge — pure,
and where the tests are; `file.rs` is the disk (atomic write through a per-call temp file that is
`sync_all`ed and renamed, a `.bak` copy of anything unparseable or too new); `commands.rs` is two
thin commands.

At the root the file keeps appearance — theme, density and `uiFontSize` — panel layout (collapsed
state and width for each side, `railOpen` for whether the project rail is drawn beside the left
panel, and `gitSections` beside them), `editor` with its own `fontSize`, `agent`, the id of the CLI agent to
start, `agentLanguage` and `taskLanguage`, the two languages that agent works in, `kanban`, how
the board is drawn, `git`, what the app does to a person's repositories without asking each time,
`window`, whether the main window opens where it was left, and `notifications`,
which sound each of the two announcements makes. Below that, `openProjects` is the list of projects the window has open,
`lastProject` is the one active when it last closed, and `projects` is a map from each project's
absolute path to its content state (side tab, active tab, selected task, `recentTasks`, selected
path, `selectedRepo`, expanded folders, `branchFolders`, `openTabs`, `previewTab`, `columnOrder`,
`runSettings`, `storageWarnedMib`, `usedAt`).

`layout.gitSections` is the other thing at the root that could plausibly have gone under a project and
did not: how the Git panel's three sections are folded, and how tall two of them were dragged to. The
argument is `kanban`'s below — a habit of reading rather than a fact about one repository — and it
also keeps five fields out of `ProjectState`, where each would have to be listed in the front end's
`defaults()` or carry the previous project's value across a switch. The two heights are **counts of
rows**, so they survive a change of density or of the app-wide font size, and `null` is a real state
rather than a stand-in for a number: until somebody drags one, a section follows its own content, so
a project of one repository draws one row instead of a reserved block of empty ones. A count outside
`2..=40` is **forgotten rather than clamped**, the rule `min_priority` follows — forgetting hands the
section back to its content, which is a real answer, where a number of ours would be an invention.
The rule that reads all of it is `components/git/sectionHeights.js` (`.claude/rules/vcs-panel.md`).

`branchFolders` went the other way and is **under the project**, right beside the file tree's
`expanded` and for the same reason: which folders the Git panel's branch list has unfolded is about a
repository's naming convention, where the heights above are about a person. It is an
`Option<Vec<String>>` in Rust, and the `Option` is the point — `None` is "nobody has chosen here" and
the panel unfolds the folder the current branch is in, while `Some([])` is somebody having folded
them all and stays folded. A plain list would collapse the two and there would be no way to fold the
last folder away. The list is cleaned in place rather than forgotten as a whole (blanks, duplicates,
anything past 200 entries), because one junk path is no reason to refold the rest. The rule that
reads it is `components/git/branchTree.js` (`.claude/rules/vcs-panel.md`).

`kanban` is the one that is **global rather than per project**, and deliberately: `columns` (`all` or
`some`) with `alwaysShow`, and `interval` (`all`, `day`, `week`, `month`) with `unlimited`, are a
person's way of reading a board rather than a fact about one repository, and the defaults are today's
behaviour exactly — every column, every task. The rule it feeds is `components/kanban/boardView.js`,
whose two closed lists are written out there and again in `model.rs`: the doubling `SIDE_TABS` and
the storage ladder carry, with the same obligation — what the front end offers must be a subset of
what Rust accepts, or the value loses itself on the next save with nothing on screen to say so.

`git` is the second global section and holds two fields, both **shipped on**, and what they have in
common is the question: what may this app do to a person's repositories without asking each time.

`autoFetch` is the first, and it is the answer to a question no other setting here asks: whether this
app may open a socket by itself. The
Git panel fetches from the selected repository's remote when the window comes back into focus, when
the project changes and on a one-minute tick under that same throttle — once every five minutes per
repository — and this switch is whether any of that happens at all. What it does **not** reach is
the check in the Branches caption: a press is not this app acting on its own, so that button goes
on working with the switch off, which is the only way a count stays refreshable at all in that
state (`.claude/rules/vcs-panel.md`). Global rather than under a
project, on `layout.gitSections`' argument rather than `branchFolders`': what it is about is a
connection and a person — a metered link, a VPN that is not always up, an SSH key with a passphrase
that would fail on every sweep — and none of those is a fact about one repository. The interval
beside it is deliberately **not** a field: a person can reasonably decide whether their machine
reaches the network on its own, and cannot reasonably decide whether four minutes is better than
five, so a number here would be a question with no way to answer it. Default on, because a feature
that does nothing until somebody finds a switch is a feature nobody finds — and the three other
copies of that default, `defaults()` in `stores/settings.js`, `view` in `SettingsWindow.vue` and the
prop default in `settings/GitSettings.vue`, have to agree with `GitSettings::default()` or the switch
draws the opposite of what the app is doing for as long as it takes the first answer to arrive.

`removeWorktrees` is the second, and it is the one field here **the app itself never acts on**: there
is no call to `git worktree` anywhere in `src-tauri/`. The lead agent of a run cuts the worktrees
(`smetana:provisioning`) and removes them (`smetana:merging`, "when the caller's policy says to"), so
the only lever the app has is a line of the run prompt — `agents/prompt.rs`'s run policy, beside
`live_check` and `file_findings`, with both branches written out for the reason those two are: a
silence is read as the default, and the default is not what somebody who has just been to this window
chose. Off says to leave them and **to say so in the report**, which is not decoration, since nothing
in this app can see a worktree or count one: without that sentence a person who forgot the switch
hears about their disk from their disk. It is affirmative rather than `keepWorktrees` so that `true`
is the shipped state and the label names what is done, and it ships on because that is today's
behaviour exactly. The road from the file to the prompt is `agent`'s: `settings::git_remove_worktrees`
→ `runs/service.rs`, read once when a run starts and carried for the whole of it, then a parameter
through `drive` and `spawn_batch` into `Intent::Run`. It rides there as **a field of its own beside
`reports` and `batch`, deliberately not inside `RunSettings`**, which is where its two neighbours in
the prompt live: `settings.json` keeps a per-project mirror of `RunSettings` — what the run dialog
opens on — so anything added there acquires a second, per-project memory of itself, and that stale
copy would ride in from the dialog and silently beat the one global answer set here. Three cases keep
today's behaviour whatever the switch says, and all three are the skills': a parked task keeps its
worktree because somebody is coming to look, a task waiting on a live check keeps one because it is
not closed yet, and a worktree that refuses to go — dirty, locked — is a line in the report and never
a stop.

`notifications` is the third global section, and it holds the two sounds — `runFinished` and
`needsAttention`, each one of `off`, `sound-1` … `sound-4`. Global on `git`'s argument exactly: a
noise is a fact about a person and a room rather than about one repository. Both ship as a sound
rather than as `off`, and as two *different* sounds, for the reason `src/sounds.js` records — a
feature nobody switches on is a feature nobody finds, and a run that ended can be read in the
morning while an agent that is waiting is a night that has stopped moving. The ids are a closed list
written out twice, `SOUNDS` in `model.rs` and `SOUND_IDS` in `sounds.js`, with the obligation
`SIDE_TABS` carries: what the front end offers must be a subset of what Rust accepts. The two
defaults are written out three times over — Rust, `defaults()` in `stores/settings.js`, `view` in
`SettingsWindow.vue` — and have to agree for `git.autoFetch`'s reason. The General tab is where they
are edited, and **choosing one plays it**: the choice is the preview, so there is no play button
beside the list, and that press is also the one gesture a webview's autoplay policy is certain to
allow. Where each sound actually fires is `.claude/rules/notifications.md`.

`window` is the fourth global section and holds one field, `restoreGeometry`, **shipped on** —
today's behaviour to the letter, since the main window has opened where it was left since before
there was a switch over it. Global on `git`'s argument, one step shorter: there is one main window,
and where it sits is a fact about a person's screen. What the switch reaches is the *restoring*
alone; the saving is unconditional in both positions, which is what makes it reversible the way
somebody expects — off for a week, then on again, and the window comes back where it was rather
than at the size in `tauri.conf.json`. The mechanism is `skip_initial_state("main")` on
`tauri-plugin-window-state` rather than its `with_denylist`, and `src-tauri/src/window.rs` carries
why, together with the consequence that put `"visible": false` in the configuration: windows
declared there are built *before* the `setup` hook, so a window shown first and restored second is
a visible jump. **Four** copies of the default, the same four `git.autoFetch` has and for the same
reason — Rust, `defaults()` in `stores/settings.js`, `view` in `SettingsWindow.vue`, and the prop
default in `components/settings/GeneralSettings.vue`. The fourth is the one to say out loud: it is
inert while the window always passes the prop, which is exactly what makes it the copy a sweep of
the other three walks past. It rides the
flat message as `restoreGeometry`, and `applyPatch` checks the type and nothing else: `false` is
the whole point of the field, so a coercion would turn a malformed event into a deliberate-looking
"off".

The per-project four are per project for the reason the rest are: a status has no meaning in another
repository's column order, a branch name has none in another repository, a repository inside one
project is not one inside another, and the attachment folder the bell weighs is a different folder
for every project. The ladder `storageWarnedMib` is validated
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
the four `kanban` fields, both `git` fields, `window.restoreGeometry` and the two `notifications`
sounds. Density is not among them, deliberately: nothing has asked for it yet,
and a screen full of switches nobody wanted is worse than a short one. `?theme=` and `?density=`
still override the first two for one run and are deliberately **not** written back — one visit to the
dev server must not repaint the app forever. `?view=gallery` neither reads nor writes.

## The settings window

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
`SettingsWindow.vue` alone (General, Editor, Agents, Kanban, Git, Storage, About); Rust guards the
*shape* of a `?tab=` name so nothing can smuggle a second parameter into the URL, never its
vocabulary, and an unknown section opens on General. Git sits between Kanban and Storage rather than
at the end, because the tabs before Storage are settings and Storage is the one that is not.

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

**Two things on this screen are not settings at all**, and they are the exceptions that keep the
rule readable. One is a whole tab and the other is a single row.

The row is **Launch at login**, the first of the General tab's Startup pair, and nothing about it
reaches `settings.json` — deliberately, and the rejected alternative is the ordinary boolean beside
`git.autoFetch`. A login item can be removed from outside this app, in macOS System Settings →
General → Login Items or whatever the platform's own list is, and a copy of it in a file of ours
would then disagree with the machine. From there only two things can be done and both are worse
than keeping no copy: bringing the system into line with the file at start-up is the app silently
putting back what somebody removed by hand a minute earlier, and not reconciling at all is a switch
stating a position the app does not hold until the next press — a control claiming a state it has
no evidence for, which is the failure refused everywhere else here. So the operating system's list
is the whole of the truth, read through `autostart_state` when the General tab is opened — on the
same `watch(tab, …)` the Storage numbers ride, not on mounting the window — and written through
`autostart_set`, which answers with the state read *back* rather than with what it was asked for,
so a registration the system declined returns the switch by itself and there is no error branch to
design. `src-tauri/src/autostart.rs` is the module, `stores/app.js` the wrappers, and neither ever
rejects: nobody to ask is `{ supported: false, enabled: false }`. Under `debug_assertions`
`supported` is false and the row is drawn **disabled rather than hidden**, with its own sentence
saying why — enabling it from `npm run tauri dev` would write the path of `target/debug/smetana`
into the machine's list, where it survives the `cargo clean` that removes the binary, and a row
that simply is not there reads as "not built yet" to the next person.

The tab is **Storage**. Nothing on it reaches `settings.json` either: it asks Rust what the attachment store weighs
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

Window size and position are still not *values* in this file — `tauri-plugin-window-state` keeps
them, in its own store — but **whether they are put back is**, and that is the whole of
`window.restoreGeometry` above. `src-tauri/src/window.rs` is what is added on top, and it now holds
both halves. Restoring: the plugin is built with `skip_initial_state("main")`, so it applies nothing
by itself, and `open_main_window` is the one thing that ever calls `restore_state` — under the
setting, in `setup`, where `settings.json` is already being read for `lastProject`. It shows the
window in **both** branches, since a restore that failed must not be able to leave the app with no
window at all. Saving: the plugin writes to disk in exactly one place — `RunEvent::Exit` — so any
run that does not reach a clean exit would leave the last run's geometry behind, and the symptom (a
window opening at the configured 1440×900 whatever size it was left at) is invisible from the front
end, since `settings.json` keeps saving on its own debounce the whole time. So `persist_geometry`
subscribes to `Resized`/`Moved` and saves 500 ms after the last one — a debounce that is not only
about disk traffic, since it also settles handler order: the plugin's own listener has long since
updated the cache by then. Both directions go through the one `FLAGS` constant, `StateFlags::all()`,
so saving less than is restored cannot happen.
