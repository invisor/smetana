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
panel, and `gitSections` beside them), `editor` with its own `fontSize` and `wordWrap`, `agent`, the id of the CLI agent to
start, `agentLanguage`, `taskLanguage`, `commitLanguage` and `reportLanguage`, the languages that agent
works in, `agentPrompt`, the person's own standing instruction for every session they are in,
`kanban`, how
the board is drawn, `git`, what the app does to a person's repositories without asking each time,
`window`, whether the main window opens where it was left, `updates`, whether the app asks
about a newer version by itself, and `notifications`,
which sound each of the two announcements makes and whether a finished run shows its report. Below that, `openProjects` is the list of projects the window has open,
`lastProject` is the one active when it last closed, and `projects` is a map from each project's
absolute path to its content state (side tab, right tab, active tab, selected task, `recentTasks`,
selected path, `selectedRepo`, expanded folders, `branchFolders`, `openTabs`, `previewTab`,
`columnOrder`, `tabOrder`, `runSettings`, `storageWarnedMib`, `usedAt`).

`tabOrder` sits beside `openTabs` rather than replacing it, and the two answer different questions:
that one is the **set of files to open again** — the dirty marks, the focus sweep and the closing of
tabs over a deleted file all hang on it — while this one is a **sequence**, naming the diffs and the
shell tabs too, whose ids die with the app. Merging them would put a dead session's id in the list
that decides which files to read. It is validated with `sane_list` like every list here, and with
ceilings that are deliberately not `column_order`'s: an entry is a tab id and a file tab's id is a
path, so the item limit is `MAX_PATH_LEN`, and the count is well past `MAX_OPEN_TABS` because three
kinds of tab share the one list. A hint rather than a truth, exactly as the column order is — the
rule that reads it is `components/shell/tabOrder.js` (`.claude/rules/files-and-editor.md`).

`agentPrompt` sits at the root beside the four languages and for their reason rather than a new one:
a standing instruction of the "talk to me briefly", "this machine has no Docker" kind is a fact about
a **person**, not about one repository, and it travels with them between projects. A per-project
field was considered and refused on the argument `kanban` and `git.autoFetch` below record, with a
second reason of its own: an instruction meant for one repository already has a better home the
harness reads by itself — `CLAUDE.md` or `AGENTS.md` — and a project half would have widened the two
windows' contract for something nobody asked for. What the field reaches, and by which road, is
`.claude/rules/agents.md`; this file only stores it.

The ceiling is `MAX_AGENT_PROMPT`, 4000 bytes, checked in both `validate` bodies through
`forget_if_too_long`. Over it the value is **forgotten whole rather than truncated** — the rule
`min_priority` and the Git panel's section heights already follow, with a second reason here that
carries more force: a truncated instruction ends mid-sentence, and that is precisely the shape
`agents/prompt.rs` refuses everywhere, since `no_prompt_stops_mid_sentence` walks every intent to
keep dangling punctuation out of a prompt. A ceiling that halved somebody's paragraph would be this
app producing exactly what that test forbids. The interface cannot reach the ceiling — the field is a
bounded `Textarea` — so the check is for a hand-edited file.

The empty string is a **legal value** of this field rather than junk: it is how a person clears it.
That is why `forget_if_junk` one field up is wrong here, and it is also why the front end's guard in
`applyPatch` is `typeof patch.agentPrompt === 'string'` and nothing more, where the four languages
beside it test truthiness as well. For a language an empty id is nothing anybody chose; here a
truthiness guard would swallow the clearing, leaving the old text in the app window's state and in
the next session started while the field on screen looked empty. The shape to copy is
`editorWordWrap`'s, not `agentLanguage`'s.

`layout.gitSections` is the other thing at the root that could plausibly have gone under a project and
did not: how the Git panel's three sections are folded, how tall two of them were dragged to, and how
tall the commit box's message field was. The
argument is `kanban`'s below — a habit of reading rather than a fact about one repository — and it
also keeps five fields out of `ProjectState`, where each would have to be listed in the front end's
`defaults()` or carry the previous project's value across a switch. The two heights are **counts of
rows**, so they survive a change of density or of the app-wide font size, and `null` is a real state
rather than a stand-in for a number: until somebody drags one, a section follows its own content, so
a project of one repository draws one row instead of a reserved block of empty ones. A count outside
`2..=40` is **forgotten rather than clamped**, the rule `min_priority` follows — forgetting hands the
section back to its content, which is a real answer, where a number of ours would be an invention.
The rule that reads all of it is `components/git/sectionHeights.js` (`.claude/rules/vcs-panel.md`).

`commitRows` sits with them and is the exception to every sentence above, which is why it is worth
naming rather than counting: it is a plain number and never `null`, because the field it sizes is a
`<textarea>` with a shipped height of two rows rather than a content to follow — there is no "let it
size itself" to hand it back to, so a count outside `1..=12` takes the default instead of being
forgotten. Its rows are the field's own lines and not `--row-h`, which is the same argument one unit
over: `rows` is what a `<textarea>` measures itself in, so a count follows the type wherever the type
goes. Three places have to agree on the default — `COMMIT_ROWS_DEFAULT` in `model.rs`, `DEFAULT_ROWS`
in `components/git/commitBox.js`, and the front end's `defaults()` — and the rule that reads it is
`commitBox.js` (`.claude/rules/vcs-panel.md`).

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

`editor` holds the two the code editor in the centre column reads, and the second of them is the
one whose default argument runs the other way from `git.autoFetch`'s. `wordWrap` says whether a line
longer than the pane wraps instead of scrolling sideways, and it **ships off**, because off is
today's behaviour to the letter — the argument `kanban`'s defaults carry. `autoFetch`'s reason below
("a switch nobody finds is a feature nobody has") is deliberately declined here: wrapping shows
itself on the first file opened and on every file after it, so shipping it on would re-lay somebody's
editor out without being asked, where a background fetch shipped on is invisible until it helps.
**Four** copies of that default, the same four `git.autoFetch` has and under the same obligation —
`EditorSettings::default()` in Rust, `defaults()` in `stores/settings.js`, `view` in
`SettingsWindow.vue`, and the prop default in `components/settings/EditorSettings.vue`: a
disagreement draws the switch in the opposite position for exactly as long as it takes the first
answer to arrive. Rust validates nothing about it,
and deliberately: a bool has no value outside its set, so a damaged one is a damaged *type*, which
loses the whole `editor` section to its defaults through `serde` — the case
`a_broken_editor_section_does_not_take_the_rest_of_the_file` already pins. What makes the switch
reach an editor somebody already has open is a CodeMirror compartment rather than a rebuilt state
(`components/files/editor/compartments.js`, `.claude/rules/files-and-editor.md`), so the caret, the
selection and the undo history survive the flip.

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

`notifications` is the third global section, and it holds four fields: the two sounds —
`runFinished` and `needsAttention`, each one of `off`, `sound-1` … `sound-4` — and two booleans
beside them, `onlyWhenUnfocused` and `showReport`. Global on `git`'s argument exactly: a
noise is a fact about a person and a room rather than about one repository. Both sounds ship as a sound
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

`onlyWhenUnfocused` is the third field and the one condition over both sounds: on, they play only
while the main window is **not** focused, and off is the behaviour that stood before it existed. It
is one field rather than one per sound deliberately — a person is either at the screen or not, and
two switches would ask the same question twice. It is **shipped on**, and it is the one default in
this file that *changes* what the app does rather than preserving it; the argument is named rather
than smuggled, and it is the one the request itself made: a sound exists for the person who is not
looking at the screen, so a sound played at somebody who is looking is noise. **Four** copies of that
default, the same four `git.autoFetch` has — Rust, `defaults()`, `view` in `SettingsWindow.vue`, and
the prop default in `components/settings/GeneralSettings.vue` — and it rides the flat message as
`notificationOnlyWhenUnfocused`, named for what it decides rather than for its section, with
`applyPatch` checking the type and nothing else for `showReport`'s reason. Rust validates nothing
about it, on that same field's argument. Its row is drawn **inside** the Notifications group and
under both sound lists, which is the opposite placing to `showReport` below and says the same thing
in reverse: this is one condition over those two sounds rather than a third kind of announcement.
What "focused" means, and the one place the switch deliberately does not reach — the preview a
dropdown plays — is `.claude/rules/notifications.md`.

`showReport` is the fourth field of that section, and it and `onlyWhenUnfocused` are both written
out in `defaults()` by hand rather than taken from `NOTIFICATION_DEFAULTS`: that constant lives in
`src/sounds.js`, which is the closed list of sounds and the two shipped ones, and neither a boolean
about a document nor one about when to make a noise has any business in it. It is **shipped on**, on `window.restoreGeometry`'s argument
exactly — today's behaviour to the letter, since a finished run has put its report in front of
somebody since before there was a switch over it, and an update that quietly stopped reports arriving
would be a feature taken away rather than added. **Four** copies of that default, the same four
`git.autoFetch` has and for the same reason — Rust, `defaults()`, `view` in `SettingsWindow.vue`, and
the prop default in `components/settings/GeneralSettings.vue`. It rides the flat message as
`notificationShowReport`, named for what it decides rather than for its section, and `applyPatch`
checks the type and nothing else, for `restoreGeometry`'s reason: `false` is the whole point of the
field. Rust validates nothing about it, deliberately — a boolean has no values outside its own set,
and a hand-edited file carrying something else there loses the whole `notifications` section through
serde and takes the defaults, exactly as `editor.wordWrap` does. What the switch actually decides —
that it is the **whole** of the delivery policy rather than one condition of two — is
`.claude/rules/notifications.md`. Its row is drawn **above** the tab's Notifications
caption rather than under it: a notification is what the app says while nobody is looking, and this
switch decides what it opens when somebody is. It sat inside that group at first and read as a third
kind of announcement beside the two sounds.

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

`updates` is the fifth global section and holds one field, `autoCheck`, **shipped on** — and it is
the second switch in this file over whether the app may open a socket by itself, `git.autoFetch`
being the first. A section rather than a flat field on `window`'s precedent: one field is already
the house shape, the key names the subsystem, and a second update preference later has somewhere to
go. Default **on** rather than today's-behaviour-to-the-letter, since there was no timer at all
before smetana-vcv: an app that never checks is an app whose update system does not exist for
anybody who does not go looking, so the switch is there to let a person decline the background
request rather than to make them opt into being told. The interval beside it is deliberately not a
field, on `autoFetch`'s argument exactly, and neither is a choice of channel: there is one channel,
and a control over a set of one says nothing.

What it reaches is the **timer alone** (`src-tauri/src/updates.rs`, `schedule`), which asks
`settings::updates_auto_check` at **every tick** rather than reading it once — that is the whole of
"off and on again without a restart", and the timer keeps ticking either way so there is nothing to
start when it comes back on. It does not reach `updates_check`, the press on About, for the reason
`autoFetch` does not reach the check in the Branches caption: a press is not the app acting on its
own. And it does not reach anything already downloaded — the machine's `ready` state and the staged
bytes are untouched, so an update that is waiting is still waiting and still installable.
**Four** copies of the default, the same four `git.autoFetch` has — Rust, `defaults()` in
`stores/settings.js`, `view` in `SettingsWindow.vue`, and the prop default in
`components/settings/GeneralSettings.vue` — and it rides the flat message as `updatesAutoCheck`,
with `applyPatch` checking the type and nothing else for `restoreGeometry`'s reason. Rust validates
nothing about it, on that same field's argument. Its row is the last of the General tab's Startup
group, under Launch at login and Restore window position: all three are what the app does by itself
around starting up, and a group for this one row would be a heading for its own sake.

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
`appearance.theme`, `appearance.uiFontSize`, both `editor` fields, `agent`, the languages beside it,
the four `kanban` fields, both `git` fields, `window.restoreGeometry`, `updates.autoCheck` and all
four `notifications` fields. Density is not among them, deliberately: nothing has asked for it yet,
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
window cannot outlive the main one, so `close_children_with_main` takes it — and the compare window
beside it, the other child of the same parent — down on the main window's `Destroyed`, which is also
what lets the app still exit on its last window.

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

A tab is a stack of `SettingsRow`s, and where a run of them belongs together it is wrapped in
`SettingsGroup`. The group draws two marks and they say different halves of one thing: a **caption**
in mono caps with a hairline running out to the right edge, and a **spine** — `border-left` in
`--border-strong` with `--space-6` of indent — down the rows themselves. The spine is what a caption
alone could never do, because it has an end: it starts at the first row's top edge and finishes on
the last row's own bottom rule, so a person can see that Startup stops before the tab does. It spans
exactly the slot's contents, so there is no arithmetic anywhere to keep in step with the rows. Before
it there was only a line of sans text in the flow, which reads as a row missing its control rather
than as a heading over what follows.

The spine is a **border weight rather than a hue**, deliberately: the left edge of a thing is where
this app puts dependency and status meaning, and a group of settings is neither. `--border-strong`
against the row rules' `--border-subtle` is enough to read as deliberate in both themes and both
densities with no second rule anywhere. The rhythm is `--space-8` over a caption against `--space-1`
under it — the group reads tighter inside than the gap that precedes it, which is what makes a
caption belong to the rows below rather than to the rows above — and the rows themselves are
untouched apart from the indent. The caption is **not a control**: no press, no focus, no hover, so a
tab's tab order stays the list of things a person can actually change. **One level of nesting**, and
no more: a group inside a group needs a second spine beside the first, and a tab wanting more
structure takes another top-level group instead.

The label is optional and the headerless form is a real case rather than a fallback. General uses the
named form for Notifications and Startup; the Kanban tab uses the bare one, because its lists of
columns are exceptions to the row above them rather than groups with names of their own — the
sentence over each stays sans prose outside the group, and what the group is used for there is the
spine alone. The sentence standing in for a board nobody has open is outside it too: with no boxes to
mark, a spine beside one line saying there is nothing would be marking its own absence.

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
language pickers under it are the same doubling against `agents::LANGUAGES`, accepted for the
same reason — Rust validates the ids, so drift costs a stale label rather than a lost setting — and
every row on the tab shares one control column, wider than the shipped default because `Dropdown`
ellipsises a label that does not fit and "Chinese (Simplified)" is the longest label any of the lists
holds. The languages sit in a `SettingsGroup` captioned Languages and the Agent row stays outside it,
which is General's own drawing: a second group over that one row would be a caption for its own sake.
Commit language reaches two places rather than one — the button in the Git
panel and a run's own commits — for the reason `.claude/rules/agents.md` records.

Report language is last in that group and the one row on this tab with a condition on it, and the
condition lives on another tab: `SettingsWindow.vue` hands `view.notificationShowReport` to
`AgentSettings.vue` as `showReport` — the section prefix dropped and the field name kept, the way
`GitSettings.vue` takes `gitAutoFetch` as `autoFetch`, so that one grep still finds both ends of the
pair — and with **Show run report** off the `Dropdown` is drawn
`disabled` and the row's description says why and names the tab the switch is on. Disabled rather
than removed, which is General's own answer in the Launch at login row of its Startup group, drawn
disabled with `autostartDescription` naming the reason: a control that refuses a
press without saying why is worse than one that is not there. A row vanishing from this tab
because of a switch on another is the second half of it — a change nobody sees happen. The description is a `computed` of
that same shape, deliberately not a pure module
of its own: `usage.js` exists because its sentences are a rule with cases, and this is one boolean
and two strings. Two things it must keep saying: the stored value is untouched while the row is shut,
so turning the switch back on brings the choice back rather than `en`, and the Off sentence says
reports are not *shown* — it must never claim the document is not written, because
`runs::service::finish` writes it either way. The
subscription block under it was a placeholder with dashes and is now the reading itself: the tab asks
`agent_usage`, which is `runs/usage.rs`'s probe — the same one the run gate makes before every batch
— put from the other end of the app. Four things about it are decisions rather than mechanics. The
answer has **three distinguishable states** and not an `Option`: an agent with no `usage_command`
(Codex) reads differently from one that was asked and could not answer, since those are different
sentences for a person and different things to do about them. The **band comes from Rust**, through
the existing `usage::decide`, so `REDUCED_THRESHOLD` and `PAUSE_THRESHOLD` keep one copy — a second
copy in JS would drift from the first silently. And the answer **names the agent that actually
replied**, because `agents::pick` substitutes the first installed profile for a configured one that
is not on `PATH`, so a heading taken from the dropdown could say Claude Code over Codex's allowance;
with nobody to name, the heading is the bare word rather than the selection. The fourth came later:
**a percentage may be absent**, since either of the two lines `/usage` prints can be reworded away,
and the half that was not read travels as `null` rather than as the zero it used to become — the
block draws exactly the rows whose percentage arrived, one of them if that is all there was, and a
real `0%` is still a row (smetana-7rp). Plan and Status are gone rather than kept as dashes —
`/usage` reports two percentages and two reset times and nothing about a tariff, so those rows could
only ever have stayed empty. It is read on **opening the tab**, the way the Storage numbers and the
login item are, and the argument is stronger here: the probe is somebody else's CLI under a
60-second ceiling, so asking on mounting the window would start it for everybody who came to change
the theme, while asking only on a press leaves the block empty at first glance — which was the
original complaint. There is no timer, and a reading is cached nowhere. The sentences are
`components/settings/usage.js`, another of the `branchChoice.js` family, and the window clears the
reading at the start of every read: switch the agent and the block has to stop talking about the
previous one before it knows anything about the new one — which is also why the guard there is a
sequence number rather than the busy flag alone, since a change of agent must supersede a probe
already out. Kanban is the same shape one tab over, and the one tab whose lists are not a closed
vocabulary at all — the columns it offers are the active project's own, read from the tracker, so
with no project open or no answer yet it says so rather than drawing an empty list.

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

**About also carries the whole of the update machine**, and it is the fifth part of this window that
is not a setting: nothing about it reaches `settings.json`, and `FIELDS` in `SettingsWindow.vue`
deliberately does not name it. `src-tauri/src/updates.rs` owns the state — `idle`, `checking`,
`available`, `downloading`, `ready`, `failed`, one tagged value that travels whole — and
`src/stores/updates.js` mirrors it: read once through `updates_state`, kept up to date by the
`updates:state` event, with `updates_check` and `updates_install` as the two presses. The window
subscribes on **mounting** rather than on opening the tab, unlike Storage and the subscription probe,
because the answer is a subscription as much as a read and an event arriving while somebody is on the
General tab has to be there when they walk over to About.

`settings/update.js` is the pure half, another of the `branchChoice.js` family beside `storage.js`:
state in, the sentence and the one control out. It knows a **seventh kind of its own**, `unavailable`,
which is what a `null` from the store and a tag this build has never heard of both come to — in a
browser there is nobody to ask, so About draws no update row at all rather than an "up to date" said
by a window that never asked anybody. That is the same silence `appVersion()` produces by answering
`null` and the version line drawing a dash. `mockBackend.js` answers `updates_state` with `null`
rather than refusing it, since a read that threw would put an error in the console on every start of
`npm run dev`.

Two states offer no control — `available` and `downloading` are flows already in hand and finish by
themselves — `checking` offers the button disabled rather than taking it away for the length of a
round trip, and `ready` offers **Install and restart, live, always**. It is never drawn dead on a
guess: the run gate is Rust's to answer, this window cannot see a run in a project nobody is looking
at, and the refusal arrives as `UpdateError`'s `{kind, detail}` naming the projects — `installRefusal`
turns it into a sentence under the row, the way `runFailure` does one window over. A control that will
not act and will not say why sends somebody to guess, which is the whole reason the refusal travels
with its reason attached.

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

`rightTab` is the same shape one column over and carries the same obligation — `RIGHT_TABS` in both
files, `task` and `sessions`, defaulting to `task`, which is the whole of what that panel drew before
it had a row of tabs at all. Only the *tab* is stored: what the `task` tab is filled with stays
derived on the front end (`rightPanel` in `DesktopApp.vue`), for the reason `selectedTask` carries —
a panel choice that wrote to a remembered field would turn a glance into an edit of a preference.

The tab moves by itself only **towards** `task`, never to `sessions`: an agent that needs an answer
already has the bell, the scope bar and the left column, and taking the panel out from under somebody
reading a task is not on that list. Three things move it, and only one of them is a watch. A draft or
a run's claimed list arriving in the column is the watch, on `rightPanel`. A card picked on the board
or in the command palette (`selectFromBoard`) and an agent row whose work is an issue
(`selectAgent`) each write the field where the click is handled instead, because both open their
issue on the board's own selection and so leave `rightPanel` on `'board'` with nothing for a watch to
fire on — and without the second of those, two rows of one session list would answer the same click
oppositely. A watch on `selectedTask` covering all three is the version that was thrown away:
`loadProjectLayout` writes that field in the same tick as the `rightTab` beside it, so the watch
would overwrite the remembered tab a microtask after restoring it and the setting would never survive
a restart.

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
