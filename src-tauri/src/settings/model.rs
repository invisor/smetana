//! App settings: types, defaults, file parsing and merging.
//!
//! No I/O here: everything that depends on the disk lives in `file.rs`.
//! That is why this file is the one carrying the tests — same as `tracker/store.rs`.

use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The file's schema version. It grows when an old file can no longer be read as is.
pub const CURRENT_VERSION: u32 = 1;
/// How many projects we remember: the map must not grow forever from one-off visits.
pub const MAX_PROJECTS: usize = 20;
/// How many projects may stay open. The limit is not about taste but about
/// keeping the list in the panel a list, and the file readable.
pub const MAX_OPEN: usize = 50;

/// `system` is not a third colour scheme: it is the absence of a choice, and the
/// effective theme is then whatever the OS says at this moment. The word is
/// stored as it stands and never resolved on this side — the front end watches
/// `prefers-color-scheme` and repaints when the machine changes its mind, so
/// writing a resolved `dark` into the file would freeze somebody's evening.
const THEMES: [&str; 3] = ["system", "dark", "light"];
const DENSITIES: [&str; 2] = ["comfortable", "compact"];
/// A closed list — and it is duplicated on the other side of the IPC: the same
/// three tabs are listed in `src/views/DesktopApp.vue` (the `SIDE_TABS`
/// constant). Change one list and you must change the other: a value missing
/// here silently becomes "files" on its way to disk, and after a restart a
/// person sees something other than what they left.
const SIDE_TABS: [&str; 3] = ["files", "git", "agents"];
/// The right column's own tab row, and the same doubling one column over: the
/// two tabs are listed again in `src/views/DesktopApp.vue` (the `RIGHT_TABS`
/// constant), with the same obligation `SIDE_TABS` carries. `task` is the whole
/// of what that panel drew before there was a row over it, so it is the one a
/// damaged value comes back as.
const RIGHT_TABS: [&str; 2] = ["task", "sessions"];
/// The centre has no closed list of tabs and never will: file tabs come from
/// the project. So we check sanity rather than membership.
const MAX_ID_LEN: usize = 200;
const MAX_PATH_LEN: usize = 4096;
const MAX_EXPANDED: usize = 500;
/// How many branch folders an unfolded list may name. Smaller than the file
/// tree's ceiling above and for a different subject: a repository has a handful
/// of prefixes — `feature`, `fix`, `release` — where a project has hundreds of
/// directories, so a list past this is a hand-edited file rather than somebody
/// who has been unfolding things.
const MAX_BRANCH_FOLDERS: usize = 200;
/// How many branches may be pinned to the top of the branch list. Smaller again
/// than the folders above, and for the reason the feature exists: the list of
/// favourites is drawn above the tree in full, so a person who marked fifty of
/// them has turned the whole panel into one flat list and lost the ordering the
/// pinning was for. The cap is a ceiling on a hand-edited file rather than a
/// budget somebody could reach by working.
const MAX_FAVORITE_BRANCHES: usize = 50;
/// How many file tabs we remember. The limit is not about taste but about
/// keeping the tab row a row, and the settings file readable.
pub const MAX_OPEN_TABS: usize = 50;
/// How many columns an order may name. bd ships eleven statuses and a project
/// adds custom ones; the cap is generous by that measure and only there to stop
/// a garbage list from growing without bound.
const MAX_COLUMNS: usize = 60;
/// How many tabs an order may name. Deliberately well past `MAX_OPEN_TABS`:
/// this list holds every tab of the centre row that is not pinned — the open
/// files, plus the diffs and the shell tabs, neither of which is remembered
/// anywhere and neither of which is counted by that ceiling. The number is only
/// there to stop a hand-edited file growing without bound; the app rewrites the
/// field whole on every drag, from the tabs that exist at that moment, so what
/// it actually holds can never outgrow the row.
const MAX_TAB_ORDER: usize = 200;
/// How many recently opened tasks a project remembers. The palette draws them
/// under `Recent` with an empty query, and `RECENT_LIMIT` in `DesktopApp.vue` is
/// the same number on the other side — three rows is a reminder, and a longer
/// list would be a second search nobody asked for.
const MAX_RECENT_TASKS: usize = 3;

/// Appearance is about the person and their screen, hence shared by all projects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub theme: String,
    pub density: String,
    /// How big the app's own type is, in pixels. Not one size among many: the
    /// front end scales the whole eight-step type scale by this number over its
    /// default, so the hierarchy between a label, a row and a heading survives
    /// the change. The terminal rides along with it, since its font size comes
    /// off the same scale.
    pub ui_font_size: u32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            density: "comfortable".into(),
            ui_font_size: UI_FONT_DEFAULT,
        }
    }
}

/// The code editor's own preferences, deliberately their own section rather
/// than more fields under `appearance`: the size answers a different question —
/// how big code should be — and it is pinned rather than scaled, so the
/// app-wide size does not move it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EditorSettings {
    pub font_size: u32,
    /// Whether a line longer than the pane wraps instead of scrolling sideways.
    /// Off by default, because off is today's behaviour to the letter — the
    /// argument `kanban`'s defaults carry, and deliberately not `git.auto_fetch`'s
    /// ("a switch nobody finds"): wrapping is visible on the first file opened,
    /// so shipping it on would re-lay somebody's editor out without being asked.
    /// A bool has no value outside its set, so `validate` has nothing to say
    /// about it; a missing field takes this default through `serde(default)`.
    pub word_wrap: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self { font_size: EDITOR_FONT_DEFAULT, word_wrap: false }
    }
}

/// What the app does to a person's repositories without asking each time.
///
/// Global rather than under a project, the argument `kanban` and
/// `layout.git_sections` are global on: whether this machine should go to the
/// network by itself, and whether a finished task's checkout is swept up after
/// it, are facts about a connection and a person, not about one repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GitSettings {
    /// Fetch from the remote by itself — on window focus, throttled, and
    /// silently. The front end owns the throttle; this is only the switch.
    pub auto_fetch: bool,
    /// Whether a run removes each task's worktree once the task is merged and
    /// closed. Nothing in this app runs `git worktree` at all: the lead agent
    /// cuts and removes them, so what this field reaches is one line of
    /// `agents::prompt`'s run policy, beside `live_check` and `file_findings`.
    ///
    /// Affirmative rather than `keep_worktrees` so that `true` is the shipped
    /// state and the label on screen names what is done. Shipped `true` because
    /// that is exactly today's behaviour: the skill removes them unconditionally.
    pub remove_worktrees: bool,
}

impl Default for GitSettings {
    fn default() -> Self {
        Self { auto_fetch: true, remove_worktrees: true }
    }
}

/// What the main window does with the size and position it was left at.
///
/// Global rather than under a project, on `GitSettings`' argument exactly:
/// there is one main window and it is a fact about a person's screen, not
/// about a repository.
///
/// Shipped on, because that is today's behaviour to the letter — the geometry
/// has always been restored, and this section is a switch over something that
/// already happens rather than a new ability. Switching it off stops the
/// *restoring* and never the *saving*: `window.rs` says why, and it is what
/// makes the switch reversible the way a person expects, since turning it back
/// on a week later opens the window where it was last left rather than at the
/// size in `tauri.conf.json`.
///
/// One field and no validation: a bool has no value outside its allowed set,
/// and a section whose *type* is wrong loses itself to this default in `parse`
/// like every other section. The two other copies of the default are
/// `defaults()` in `src/stores/settings.js` and `view` in
/// `src/views/SettingsWindow.vue`, and they have to agree with this one for
/// `GitSettings`' reason: the switch would otherwise draw the opposite of what
/// the app is doing for as long as the first answer takes to arrive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WindowSettings {
    /// Open the main window where it was left, rather than at the size the
    /// configuration names.
    pub restore_geometry: bool,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self { restore_geometry: true }
    }
}

/// Whether the app goes to GitHub by itself to ask whether a newer version
/// exists.
///
/// A section rather than a flat field, matching `WindowSettings` above: one
/// field is already the house shape here, the key names the subsystem, and a
/// second update-related preference later has somewhere to go without moving
/// this one.
///
/// Global on `GitSettings`' argument, one step shorter even than `window`'s:
/// there is one application and one release feed, and whether this machine may
/// reach for it is a fact about a person's connection rather than about any
/// repository. It is the second switch in this file over "may this app open a
/// socket by itself", `git.auto_fetch` being the first, and the interval beside
/// it is deliberately not a field for that one's reason: a person can decide
/// whether their machine reaches the network on its own and cannot reasonably
/// decide whether a day is better than two.
///
/// Shipped **on**. An app that never checks is an app whose update system does
/// not exist for anybody who does not go looking, and the switch is there so a
/// person can decline the background request rather than so they have to opt
/// into being told about a release.
///
/// What it reaches is the **timer alone** — `updates::schedule`, which asks for
/// this value at each tick rather than reading it once, so switching it off
/// stops the scheduled check and switching it back on restores it with no
/// restart. It does not reach `updates_check`, the press on the About tab: a
/// press is not this app acting on its own, exactly as `git.auto_fetch` leaves
/// the check in the Branches caption alone. Nor does it discard anything
/// already downloaded — the machine's `ready` state and the staged bytes are
/// untouched by the switch, so an update that is waiting is still waiting and
/// still installable.
///
/// No validation, deliberately, and for `WindowSettings`' reason: a bool has no
/// value outside its allowed set, so a section whose *type* is wrong loses
/// itself to this default in `parse` like every other section. The three other
/// copies of the default are `defaults()` in `src/stores/settings.js`, `view`
/// in `src/views/SettingsWindow.vue` and the prop default in
/// `src/components/settings/GeneralSettings.vue`, and they have to agree with
/// this one for `GitSettings`' reason: the switch would otherwise draw the
/// opposite of what the app is doing for as long as the first answer takes to
/// arrive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UpdateSettings {
    /// Ask the release feed by itself — a minute after launch and once a day
    /// after that. The timer owns the schedule; this is only the switch.
    pub auto_check: bool,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self { auto_check: true }
    }
}

/// The closed list of sound ids `settings.json` may hold, `off` among them.
/// Written out again as `SOUND_IDS` in `src/sounds.js`, with the obligation
/// `SIDE_TABS` and the storage ladder carry: what the front end offers must be
/// a subset of what this accepts, or a choice loses itself on the next save
/// with nothing on screen to say so.
const SOUNDS: [&str; 5] = ["off", "sound-1", "sound-2", "sound-3", "sound-4"];

/// What the app says when a run ends or an agent stops to ask: with which
/// sound, and whether the run's own account is put in front of the person at
/// all.
///
/// Global rather than under a project, on `GitSettings`' argument exactly:
/// whether this machine makes a noise, and which one, is a fact about a person
/// and a room rather than about one repository. `show_report` is global on the
/// same argument one step over — the General tab is global by contract, and
/// wanting the document or not is a habit of reading rather than a fact about
/// one repository.
///
/// Both sounds ship as a sound rather than as `off`, and as two *different*
/// sounds: the events they announce — a run that has ended, an agent that has
/// stopped to ask something — call for different reactions, and a feature
/// nobody switches on is a feature nobody finds. `NOTIFICATION_DEFAULTS` in
/// `src/sounds.js` is the other copy of those two values; neither
/// `only_when_unfocused` nor `show_report` is in that file, which is about
/// sounds and has no business holding a boolean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NotificationSettings {
    /// Played when a run reaches its ending, whichever way its report was
    /// delivered — and whether or not it was delivered at all: the sound is a
    /// separate answer from `show_report` and stays whatever that one says.
    pub run_finished: String,
    /// Played when an agent session enters `needs-you`, in any project.
    pub needs_attention: String,
    /// Whether the two sounds above wait until the main window is in the
    /// background. Shipped **on**, and it is the one default in this file that
    /// changes what the app does rather than preserving it: a sound exists for
    /// the person who is not looking at the screen, and one played at somebody
    /// who is looking is noise. The question is asked of the main window's
    /// document at the moment of the noise — `src/chime.js` over `shouldPlay`
    /// in `src/sounds.js` — so nothing about focus reaches this side at all.
    /// The preview on the General tab is deliberately outside it: choosing a
    /// sound plays it whatever this says.
    pub only_when_unfocused: bool,
    /// Whether a run that has ended puts its report in front of the person.
    /// The whole of the delivery policy — `src/components/run/reportDelivery.js`
    /// asks this and nothing else — and shipped **on**, because that is today's
    /// behaviour and somebody updating the app must not find their reports have
    /// silently stopped arriving.
    pub show_report: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            run_finished: "sound-1".into(),
            needs_attention: "sound-2".into(),
            only_when_unfocused: true,
            show_report: true,
        }
    }
}

impl NotificationSettings {
    /// Nothing is checked for either boolean, deliberately: a boolean has no
    /// values outside its own set, and a hand-edited file carrying something
    /// else there loses the whole `notifications` section through serde and
    /// takes the defaults — exactly what `editor.wordWrap` does one struct over.
    fn validate(&mut self) {
        one_of(&mut self.run_finished, &SOUNDS, "sound-1");
        one_of(&mut self.needs_attention, &SOUNDS, "sound-2");
    }
}

/// The shipped sizes, in pixels: today's `--text-md` for the app and today's
/// `--text-code-size` for the editor. Repeated in the front end
/// (`src/appearance.js`), for the same reason the panel widths are: with no back
/// end the app still has to open looking the same.
pub const UI_FONT_DEFAULT: u32 = 13;
pub const EDITOR_FONT_DEFAULT: u32 = 12;
/// The range both dropdowns offer. Sanity bounds rather than taste: below the
/// floor the interface stops being readable, above the ceiling it stops fitting.
const MIN_FONT: u32 = 10;
const MAX_FONT: u32 = 24;

/// How the board is drawn: which columns are worth a slot on screen, and how
/// far back a card is worth looking at.
///
/// At the root beside `layout` and `editor` rather than under a project, and
/// that is a decision with its eyes open. The honest argument for per-project
/// is the one written on `column_order` below — a custom status of one
/// repository has no meaning in another's — and both lists here are lists of
/// exactly such statuses. What outweighed it is the size of the change: storing
/// them per project would widen the two windows' contract by a project half
/// (`settings:state` carrying the active project's state, `settings:apply` able
/// to edit it, the settings window learning which project it is talking about),
/// and nobody asked for that. The price is paid in the interface instead: the
/// tab draws a stored name that no column of this project matches in a group of
/// its own, so it can be seen and taken off.
///
/// Both scalars default to today's board exactly — every column, every task —
/// so nothing on anybody's screen moves until they go and choose. The same
/// argument that keeps both agent languages at `en` rather than at an Auto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct KanbanSettings {
    /// `all` or `non-empty`.
    pub columns: String,
    /// Columns that stay on the board even when nothing is in them. Only means
    /// anything under `non-empty`.
    pub always_show: Vec<String>,
    /// `all`, `day`, `week` or `month`.
    pub interval: String,
    /// Columns the interval does not reach: they show everything they hold,
    /// whatever the window says.
    pub unlimited: Vec<String>,
}

impl Default for KanbanSettings {
    fn default() -> Self {
        Self {
            columns: "all".into(),
            always_show: Vec::new(),
            interval: "all".into(),
            unlimited: Vec::new(),
        }
    }
}

/// The two closed lists, written out a second time in
/// `src/components/kanban/boardView.js` — the same doubling `SIDE_TABS` and
/// `STORAGE_THRESHOLDS_MIB` carry, and with the same obligation: what the tab
/// offers must be a subset of what this accepts, since a value refused here
/// loses itself on the next save with nothing on screen to say so.
const KANBAN_COLUMNS: [&str; 2] = ["all", "non-empty"];
const KANBAN_INTERVALS: [&str; 4] = ["all", "day", "week", "month"];

impl KanbanSettings {
    fn validate(&mut self) {
        one_of(&mut self.columns, &KANBAN_COLUMNS, "all");
        one_of(&mut self.interval, &KANBAN_INTERVALS, "all");
        // Status names, so the identifier ceiling and the same cap a column
        // order gets. Membership is deliberately not checked here either: bd's
        // set of statuses is unknown on this side, and a name matching nothing
        // is drawn as such by the tab rather than being thrown away — which is
        // what makes a list stored against another project's board removable
        // instead of invisibly at work.
        sane_list(&mut self.always_show, MAX_COLUMNS, MAX_ID_LEN);
        sane_list(&mut self.unlimited, MAX_COLUMNS, MAX_ID_LEN);
    }
}

/// Collapsed state and width of the side panels — also about the screen, not
/// about content.
///
/// The stored width is the one a person dragged to, not the one that fitted the
/// window: fitting it to the current window is the front end's job
/// (`src/views/panelWidths.js`), and a narrow window must not rewrite a
/// preference. Hence the generous ceiling in the check below: it catches
/// garbage, not tightness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Layout {
    pub left_collapsed: bool,
    pub right_collapsed: bool,
    /// Whether the project rail is drawn beside the left panel.
    ///
    /// Per window rather than per project, the same as the widths beside it:
    /// it is a preference about this window's chrome. `serde(default)` on the
    /// struct is what makes a file written before this field existed read back
    /// with the rail open, which is the shipped state.
    pub rail_open: bool,
    pub left_width: u32,
    pub right_width: u32,
    pub git_sections: GitSections,
}

/// How the three sections of the Git panel are folded, and how tall two of them
/// were dragged to.
///
/// Global rather than per project, and for the reason `KanbanSettings` above is:
/// how tall somebody likes their branch list and whether they ever want to look
/// at the repository list are habits of reading, not facts about one
/// repository. It also keeps five fields out of `ProjectState`, where every one
/// of them would have to be listed in the front end's `defaults()` or carry the
/// previous project's value across a switch.
///
/// The two heights are counts of rows and not pixels, which is what lets them
/// survive a change of density or of the app-wide font size — `--row-h` is
/// defined against both. `None` is "never dragged", and it is a real state
/// rather than a stand-in for a number: until somebody drags one, a section
/// follows its own content, so a project of one repository draws one row instead
/// of a reserved block of empty ones. The rule that reads all of this is
/// `src/components/git/sectionHeights.js`.
///
/// `commit_rows` is the third height and the one that is **not** an `Option`,
/// which is the difference worth knowing before reading the three together. The
/// commit box's field has a shipped height rather than a content to follow — it
/// is a `<textarea>` and two rows is what it was fixed at before it could be
/// dragged — so there is no state here that means "let it size itself", and an
/// out-of-range number goes back to that default instead of being forgotten.
/// Its rows are the field's own lines rather than `--row-h`, which is the same
/// argument one unit over: `rows` is what a `<textarea>` measures itself in, so
/// a count follows the type wherever the type goes. The rule that reads it is
/// `src/components/git/commitBox.js`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GitSections {
    pub repos_rows: Option<u32>,
    pub branch_rows: Option<u32>,
    pub commit_rows: u32,
    pub repos_open: bool,
    pub changes_open: bool,
    pub branches_open: bool,
}

impl Default for GitSections {
    fn default() -> Self {
        Self {
            repos_rows: None,
            branch_rows: None,
            commit_rows: COMMIT_ROWS_DEFAULT,
            repos_open: true,
            changes_open: true,
            branches_open: true,
        }
    }
}

/// Sanity bounds on a dragged section, mirrored by `MIN_ROWS` and `MAX_ROWS` in
/// `sectionHeights.js`. Nothing is ever drawn `MAX_SECTION_ROWS` tall — the
/// front end clamps against the panel it is in now — so this catches a
/// hand-edited file, not tightness.
const MIN_SECTION_ROWS: u32 = 2;
const MAX_SECTION_ROWS: u32 = 40;

/// The commit box's field, in its own lines. Mirrored by `MIN_ROWS`, `MAX_ROWS`
/// and `DEFAULT_ROWS` in `commitBox.js` — this is the guard on what a file may
/// carry, and that one is what the panel draws; a value has to pass both.
///
/// The ceiling is lower than a section's 40 because it protects something else:
/// this field is sticky at the top of the change list, so past a dozen lines it
/// stops being a field over a list and becomes a list nobody can see.
const MIN_COMMIT_ROWS: u32 = 1;
const MAX_COMMIT_ROWS: u32 = 12;
const COMMIT_ROWS_DEFAULT: u32 = 2;

/// The defaults are repeated in the front end (`LEFT_DEFAULT`, `RIGHT_DEFAULT`
/// in `panelWidths.js`): with no back end the app must still open looking the same.
pub const LEFT_WIDTH_DEFAULT: u32 = 236;
pub const RIGHT_WIDTH_DEFAULT: u32 = 340;
/// Sanity bounds, not layout ones. A zero would come from a panel collapsed
/// into a rail, and after a restart it would expand into nothing; the upper
/// bound cuts off accidental garbage without getting in a wide monitor's way.
const MIN_PANEL_WIDTH: u32 = 120;
const MAX_PANEL_WIDTH: u32 = 4000;

impl Default for Layout {
    fn default() -> Self {
        Self {
            left_collapsed: false,
            right_collapsed: false,
            rail_open: true,
            left_width: LEFT_WIDTH_DEFAULT,
            right_width: RIGHT_WIDTH_DEFAULT,
            git_sections: GitSections::default(),
        }
    }
}

/// Everything to do with content sits under the project's path. There is no
/// multi-project yet and the entry is always one, but the shape is already the
/// one it will take.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectState {
    pub side_tab: String,
    /// Which of the right column's two tabs is showing: the task, or the
    /// sessions running in this project. Per project on `side_tab`'s argument
    /// exactly — which half of that panel somebody wants open is a habit they
    /// have in one repository and not in another.
    ///
    /// Only the tab is stored. What the `task` tab is *filled* with stays
    /// derived on the front end (`rightPanel` in `DesktopApp.vue`), for the
    /// reason `selected_task` records one field up.
    pub right_tab: String,
    pub active_tab: String,
    pub selected_task: Option<String>,
    /// The last three tasks somebody looked at in this project, newest first.
    /// The front end maintains it — a watch on the selection, so the word means
    /// "looked at" and not "found by searching" — and Rust only has to know the
    /// field exists and what it defaults to, since the two default sets have to
    /// agree or the defaults layer over there cannot clear it on a switch.
    pub recent_tasks: Vec<String>,
    pub selected_path: Option<String>,
    /// Which repository the Git panel is showing, as an absolute path — the
    /// argument every command in `vcs/` takes anyway.
    ///
    /// Per project for the reason `column_order` is: a repository inside one
    /// project means nothing inside another. A hint rather than a truth, the
    /// same as that field — a path no longer among the project's repositories
    /// is passed over in silence by `src/stores/vcs.js` and the first one is
    /// shown instead, because a panel aimed at a folder that is gone would draw
    /// an error about a choice nobody made today.
    pub selected_repo: Option<String>,
    pub expanded: Vec<String>,
    /// Which branch folders the Git panel has unfolded, by whole path —
    /// `feature`, `fix/legacy`. Per project because a branch naming convention
    /// is, the same reasoning `column_order` below carries.
    ///
    /// **`None` and `Some([])` are different states and the type is an
    /// `Option` for exactly that reason.** `None` is "nobody has chosen here"
    /// and the panel unfolds the folder the current branch is in, so the tick
    /// saying where you are is on screen the first time. `Some([])` is somebody
    /// having folded them all, and stays folded — with a plain `Vec` there
    /// would be no way to fold the last folder away, because the empty list
    /// would read as the first case and come back unfolded on the next start.
    pub branch_folders: Option<Vec<String>>,
    /// Which branches the Git panel pins above the tree, by whole name. Per
    /// project and beside `branch_folders` on that field's own argument: which
    /// names are worth keeping in reach is a fact about a repository and its
    /// naming convention, not a habit of reading, so it is not in `layout`.
    ///
    /// **A plain `Vec` and not an `Option`, which is where it parts company
    /// with its neighbour.** `branch_folders` has a real third state — nobody
    /// has chosen here, so unfold the folder the current branch is in — and
    /// this has none: an empty list means exactly "nothing is marked", which is
    /// also what the panel does when nobody has ever marked anything.
    ///
    /// Not checked against the repository, the rule `selected_repo` and
    /// `column_order` both keep: which branches exist is not known here, and a
    /// name that matches nothing simply draws no row.
    pub favorite_branches: Vec<String>,
    /// Open files in tab order. Paths are relative to the project root: the
    /// project's key in the map is already absolute, and duplicating it in
    /// every tab is pointless — it also means a moved folder does not turn the
    /// list into rubbish.
    pub open_tabs: Vec<String>,
    /// Which of them is temporary. There is always exactly one, and it is never
    /// dirty: an edit drops the temporary flag at the same moment it adds the dot.
    pub preview_tab: Option<String>,
    /// The board's columns in the order they were dragged into, named by the
    /// front end's status vocabulary. Per project because the set of statuses
    /// is: bd carries custom ones, and one repository's status has no meaning
    /// in another one's order.
    ///
    /// A hint rather than a truth — bd still owns which columns exist. A name
    /// here that bd no longer has costs nothing and is not pruned, so a status
    /// that comes back finds its place again.
    pub column_order: Vec<String>,
    /// The centre's tab row in the order it was dragged into, by tab id, and
    /// only the tabs that can be dragged: the pinned run — the board and the
    /// Agent tab — is not in it and cannot be moved.
    ///
    /// Beside `open_tabs` rather than instead of it, and the two answer
    /// different questions. That one is the set of files to open again, and the
    /// dirty marks, the focus sweep and the closing of tabs over a deleted file
    /// all hang on it; this one is a sequence, and it names diffs and shell tabs
    /// too — ids of things that die with the app. Merging them would put a dead
    /// session's id in the list that decides which files to read.
    ///
    /// A hint rather than a truth, exactly as `column_order` is: an id nothing
    /// matches is passed over rather than pruned, so after a restart, when only
    /// the file tabs are back, the entries for the diffs and the terminals cost
    /// nothing. The front end rewrites the field whole on the next drag, from
    /// the tabs standing at that moment, so the file cleans itself up.
    pub tab_order: Vec<String>,
    /// What the run dialog was last set to here. `None` until somebody opens
    /// it, which is every settings file written before this existed.
    ///
    /// Per project, for the same reason `column_order` is: a branch name has no
    /// meaning in another repository. And **without the scope** — that comes
    /// from whichever play button was pressed, and remembering it would open
    /// the dialog claiming to run something nobody clicked.
    pub run_settings: Option<RunDefaults>,
    /// The highest attachment-storage threshold this project has already been
    /// warned about, in MiB, or `None` for a project nobody has been warned
    /// about yet. The one thing the notification bell keeps between runs.
    ///
    /// Per project because the folder is: the store is laid out by project key
    /// and the clean-up button reaches one project's folder and no other, so a
    /// number about that folder belongs beside `column_order` for exactly the
    /// same reason a column order does.
    ///
    /// It is re-set after **every** measurement to the highest threshold the
    /// folder still reaches, which is what makes a threshold announce once and
    /// then arm itself again when somebody cleans up — see
    /// `src/components/notifications/notifications.js`, which owns that rule and
    /// the ladder this field is validated against.
    pub storage_warned_mib: Option<u32>,
    /// RFC 3339, stamped on write. Needed only for trimming the map.
    pub used_at: Option<String>,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self {
            side_tab: "files".into(),
            right_tab: "task".into(),
            active_tab: "kanban".into(),
            selected_task: None,
            recent_tasks: Vec::new(),
            selected_path: None,
            selected_repo: None,
            expanded: Vec::new(),
            branch_folders: None,
            favorite_branches: Vec::new(),
            open_tabs: Vec::new(),
            preview_tab: None,
            column_order: Vec::new(),
            tab_order: Vec::new(),
            run_settings: None,
            storage_warned_mib: None,
            used_at: None,
        }
    }
}

/// What the run dialog opens on next time. A mirror of `runs::model::RunSettings`
/// minus the scope, and deliberately its own type rather than a reuse: this one
/// lives in a file people edit by hand and has to tolerate anything, while the
/// other crosses the IPC boundary and must not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RunDefaults {
    pub mode: String,
    pub target_branch: Option<String>,
    /// `None` is "no floor was ever chosen here", and it is a different thing
    /// from any number — an `Option` for exactly the reason `RunSettings`'
    /// own field is one, which is what makes this a mirror rather than a
    /// near-mirror. A run aimed at a task or an epic sends no floor at all;
    /// stored as a 2 it would come back as a choice nobody made, and the queue
    /// dialog would open on it instead of on the project's own
    /// `[defaults] min_priority` — quietly overriding the file with a number
    /// that only ever came from this field's default.
    ///
    /// Left out of the file entirely when there is none, which is the one place
    /// in this schema where that matters. Every change to it so far has been
    /// additive, and an unknown key is ignored — but this field changed *type*,
    /// and a build older than this one reads `u8` here. A present `null` is not
    /// a missing field, so `#[serde(default)]` would not rescue it: the whole
    /// `RunDefaults` would fail, taking `ProjectState` with it, and `projects()`
    /// drops an entry it cannot parse — that project's side tab, open tabs,
    /// expanded folders and selection, gone without a word. The version is
    /// deliberately not bumped for this, which would cost every project's
    /// entry rather than one. Absent, an older build simply takes its own
    /// default, and nothing in the current front end can tell absent from null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_priority: Option<u8>,
    pub live_check: bool,
    pub file_findings: bool,
}

impl Default for RunDefaults {
    fn default() -> Self {
        Self {
            mode: "auto".into(),
            target_branch: None,
            min_priority: None,
            live_check: true,
            file_findings: true,
        }
    }
}

/// The three the dialog offers. Written out here as well as in
/// `runs::model::RunMode` — the one place this codebase accepts that
/// duplication, and for the reason recorded on `IDS`: what crosses the IPC
/// boundary must refuse an unknown value, while what comes off somebody's disk
/// must survive one. `solo` is in the list; whether it is allowed for the scope
/// in front of you is `RunSettings::validate`'s answer, not this file's.
const RUN_MODES: [&str; 3] = ["auto", "supervised", "solo"];

/// bd's priority scale. Anything outside it would silently take everything or
/// nothing.
const MAX_PRIORITY: u8 = 4;

/// The attachment-storage thresholds the bell announces, in MiB. Written out a
/// second time in `src/components/notifications/notifications.js`, which owns
/// the rule; this copy exists so a hand-edited number cannot silence a warning
/// for ever — a value off the ladder loses itself, and the whole cost of that is
/// one repeated warning. The same doubling `SIDE_TABS` carries, with the same
/// obligation: both copies move together.
const STORAGE_THRESHOLDS_MIB: [u32; 3] = [10, 50, 100];

/// The whole file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    pub appearance: Appearance,
    pub layout: Layout,
    /// The code editor's own preferences. A section of its own at the root, for
    /// the reason `EditorSettings` records.
    pub editor: EditorSettings,
    /// How the board is drawn. At the root rather than under a project, for the
    /// reason `KanbanSettings` records.
    pub kanban: KanbanSettings,
    /// What the Git panel may do on its own. At the root for the reason
    /// `GitSettings` records.
    pub git: GitSettings,
    /// What the main window does with the geometry it was left at. At the root
    /// for the reason `WindowSettings` records.
    pub window: WindowSettings,
    /// Whether the app asks about a newer version by itself. At the root for
    /// the reason `UpdateSettings` records.
    pub updates: UpdateSettings,
    /// What the app says out loud. At the root for the reason
    /// `NotificationSettings` records.
    pub notifications: NotificationSettings,
    /// Which CLI agent the app starts. A habit of the person's, not a property
    /// of the repository, so it sits at the root rather than under a project.
    ///
    /// The set of legal values is `agents::IDS` and is not repeated here: the
    /// side-tab list is already written out twice — in this file and in
    /// `src/views/DesktopApp.vue` — and a value missing from one of them comes
    /// back after a restart as something the person did not choose.
    pub agent: String,
    /// The language a CLI agent talks to the person in, the language the prose
    /// of a bd issue it writes is in, the language it writes a git commit
    /// message in, and the language a run's report is written in. Every one of
    /// them sits at the root beside `agent` and for the same reason: which
    /// language somebody wants to be spoken to in is a habit of theirs and
    /// travels with them between repositories.
    ///
    /// The set of legal values is `agents::LANGUAGES` and is not repeated here,
    /// the same as `agent` above.
    pub agent_language: String,
    pub task_language: String,
    /// What `commit_language` moves is the prose of a message; whatever sits in
    /// front of the colon stays as it is, whatever it says.
    pub commit_language: String,
    /// What `report_language` moves is the prose a run's lead writes into
    /// `.smetana/runs/<token>/batch-<n>.json` — the `did` line for each task
    /// and the batch's `notes`. The JSON keys around that prose do not move,
    /// because `runs::report` matches them as literal strings, and neither do
    /// `runs::report`'s own English labels, which are this product's interface
    /// copy. `prompt.rs` records the whole watershed.
    pub report_language: String,
    pub last_project: Option<String>,
    /// The contents and order of the on-screen list — the order things were
    /// added, not how recent they are: rows that jump on every switch are
    /// unreadable.
    pub open_projects: Vec<String>,
    pub projects: BTreeMap<String, ProjectState>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            appearance: Appearance::default(),
            layout: Layout::default(),
            editor: EditorSettings::default(),
            kanban: KanbanSettings::default(),
            git: GitSettings::default(),
            window: WindowSettings::default(),
            updates: UpdateSettings::default(),
            notifications: NotificationSettings::default(),
            agent: "claude".into(),
            agent_language: crate::agents::DEFAULT_LANGUAGE.into(),
            task_language: crate::agents::DEFAULT_LANGUAGE.into(),
            commit_language: crate::agents::DEFAULT_LANGUAGE.into(),
            report_language: crate::agents::DEFAULT_LANGUAGE.into(),
            last_project: None,
            open_projects: Vec::new(),
            projects: BTreeMap::new(),
        }
    }
}

/// What the front end sees: the shared parts (`appearance`, `layout`), one
/// project's state (`project`), the contents of the open list (`open_projects`)
/// and which of them is active (`active_project`). The last two are half of
/// what crosses the IPC: the on-screen list and the highlighted row come from
/// them, and they come back through `settings_save`, because the truth about
/// the list's contents lives in the front end. The map of the remaining
/// projects never crosses the boundary — the front end knows nothing about it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ResolvedSettings {
    pub appearance: Appearance,
    pub layout: Layout,
    /// The code editor's own preferences. See `Settings::editor`.
    pub editor: EditorSettings,
    /// How the board is drawn. See `Settings::kanban`.
    pub kanban: KanbanSettings,
    /// What the Git panel may do on its own. See `Settings::git`.
    pub git: GitSettings,
    /// What the main window does with its geometry. See `Settings::window`.
    pub window: WindowSettings,
    /// Whether the app checks for a newer version by itself. See
    /// `Settings::updates`.
    pub updates: UpdateSettings,
    /// What the app says out loud. See `Settings::notifications`.
    pub notifications: NotificationSettings,
    /// Which CLI agent the app starts. See `Settings::agent`.
    pub agent: String,
    /// The languages. See `Settings::agent_language`.
    pub agent_language: String,
    pub task_language: String,
    pub commit_language: String,
    pub report_language: String,
    pub project: ProjectState,
    pub open_projects: Vec<String>,
    pub active_project: Option<String>,
}

/// Written out rather than derived for the sake of one field: the derived
/// `Default` gave `agent` the empty string, which is not an agent and
/// contradicts the `"claude"` that `Settings` beside it defaults to. Nothing
/// reachable read that value — `resolve` always copies the file's and `merge`
/// validates before writing — but a default that disagrees with its own file
/// is a trap for the next person to lean on it.
impl Default for ResolvedSettings {
    fn default() -> Self {
        Self {
            appearance: Appearance::default(),
            layout: Layout::default(),
            editor: EditorSettings::default(),
            kanban: KanbanSettings::default(),
            git: GitSettings::default(),
            window: WindowSettings::default(),
            updates: UpdateSettings::default(),
            notifications: NotificationSettings::default(),
            agent: "claude".into(),
            agent_language: crate::agents::DEFAULT_LANGUAGE.into(),
            task_language: crate::agents::DEFAULT_LANGUAGE.into(),
            commit_language: crate::agents::DEFAULT_LANGUAGE.into(),
            report_language: crate::agents::DEFAULT_LANGUAGE.into(),
            project: ProjectState::default(),
            open_projects: Vec::new(),
            active_project: None,
        }
    }
}

/// What came out of the file.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Ok(Settings),
    /// Not JSON, or not an object — there is nothing to read.
    Broken,
    /// The file is newer than this build: silently dropping someone else's
    /// fields is not allowed.
    TooNew,
}

pub fn parse(text: &str) -> Outcome {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Outcome::Broken;
    };
    let Some(object) = value.as_object() else {
        return Outcome::Broken;
    };

    let version = object.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version > CURRENT_VERSION as u64 {
        return Outcome::TooNew;
    }
    let object = migrate(object.clone(), version);

    let mut settings = Settings {
        version: CURRENT_VERSION,
        appearance: section(&object, "appearance"),
        layout: section(&object, "layout"),
        editor: section(&object, "editor"),
        kanban: section(&object, "kanban"),
        git: section(&object, "git"),
        window: section(&object, "window"),
        updates: section(&object, "updates"),
        notifications: section(&object, "notifications"),
        agent: object.get("agent").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| "claude".into()),
        agent_language: language_field(&object, "agentLanguage"),
        task_language: language_field(&object, "taskLanguage"),
        commit_language: language_field(&object, "commitLanguage"),
        report_language: language_field(&object, "reportLanguage"),
        last_project: object.get("lastProject").and_then(Value::as_str).map(str::to_owned),
        open_projects: section(&object, "openProjects"),
        projects: projects(&object),
    };
    adopt_last_project(&mut settings);
    settings.validate();
    Outcome::Ok(settings)
}

/// A file written before the list existed knows only `lastProject`. Without
/// this step `validate` would see an active project outside an empty list and
/// clear it — someone who just updated the app would meet an empty panel
/// instead of their project. The schema version does not help here: those files
/// carry `version: 1` too, so the decision goes by content.
///
/// On file reads only. An empty list from the front end is a deliberate "I
/// closed the last project", and it must not be resurrected:
/// `ResolvedSettings::validate` knows nothing of this leniency.
fn adopt_last_project(settings: &mut Settings) {
    if settings.open_projects.is_empty() {
        if let Some(last) = settings.last_project.clone() {
            settings.open_projects.push(last);
        }
    }
}

/// What we hand the front end: the shared parts, the open list and the active
/// project's state. `active` means "show me this one": that is how the front
/// end gets another project's state when switching, without restarting the app.
/// With no argument we take the active project from the file.
pub fn resolve(file: &Settings, active: Option<&str>) -> ResolvedSettings {
    let active = active.map(str::to_owned).or_else(|| file.last_project.clone());
    ResolvedSettings {
        appearance: file.appearance.clone(),
        layout: file.layout.clone(),
        editor: file.editor.clone(),
        kanban: file.kanban.clone(),
        git: file.git.clone(),
        window: file.window.clone(),
        updates: file.updates.clone(),
        notifications: file.notifications.clone(),
        agent: file.agent.clone(),
        agent_language: file.agent_language.clone(),
        task_language: file.task_language.clone(),
        commit_language: file.commit_language.clone(),
        report_language: file.report_language.clone(),
        project: active
            .as_deref()
            .and_then(|path| file.projects.get(path))
            .cloned()
            .unwrap_or_default(),
        open_projects: file.open_projects.clone(),
        active_project: active,
    }
}

/// Puts the resolved view back into the file. `now` comes from outside so the
/// function stays pure and testable.
pub fn merge(file: &mut Settings, mut resolved: ResolvedSettings, now: String) {
    resolved.validate();
    file.version = CURRENT_VERSION;
    file.appearance = resolved.appearance;
    file.layout = resolved.layout;
    file.editor = resolved.editor;
    file.kanban = resolved.kanban;
    file.git = resolved.git;
    file.window = resolved.window;
    file.updates = resolved.updates;
    file.notifications = resolved.notifications;
    file.agent = resolved.agent;
    file.agent_language = resolved.agent_language;
    file.task_language = resolved.task_language;
    file.commit_language = resolved.commit_language;
    file.report_language = resolved.report_language;
    file.open_projects = resolved.open_projects;
    file.last_project = resolved.active_project.clone();

    // The last project was closed — there is nobody to write state for, but the
    // list and the appearance still have to be saved.
    if let Some(active) = resolved.active_project {
        let mut state = resolved.project;
        state.used_at = Some(now);
        file.projects.insert(active.clone(), state);
        trim(&mut file.projects, Some(&active), &file.open_projects);
    } else {
        trim(&mut file.projects, None, &file.open_projects);
    }
}

/// A section is read independently of its neighbours: a broken type in one must
/// not take the whole file down.
fn section<T: Default + serde::de::DeserializeOwned>(object: &Map<String, Value>, key: &str) -> T {
    object
        .get(key)
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

fn projects(object: &Map<String, Value>) -> BTreeMap<String, ProjectState> {
    let Some(map) = object.get("projects").and_then(Value::as_object) else {
        return BTreeMap::new();
    };
    // And every entry stands on its own too.
    map.iter()
        .filter_map(|(path, value)| {
            serde_json::from_value::<ProjectState>(value.clone())
                .ok()
                .map(|state| (path.clone(), state))
        })
        .collect()
}

/// A seam for migrations, not a migration.
///
/// `parse` reads `version`, calls this function and stamps `CURRENT_VERSION`
/// itself. Today both arms return the object as is: a file with no version is
/// the first schema, it has the same fields, and it reads without rework. When
/// the schema does diverge from the old one, the field rewriting will appear
/// here; there is no chain of steps yet, and pretending there is would be idle.
fn migrate(object: Map<String, Value>, from: u64) -> Map<String, Value> {
    match from {
        0 | 1 => object,
        // `parse` never lets versions newer than the current one reach here.
        _ => object,
    }
}

/// Keeps the MAX_PROJECTS most recent among those that may be evicted. An entry
/// with no `usedAt` counts as the oldest: it comes from a hand-written file and
/// has nothing to defend itself with.
///
/// `usedAt` is compared as an instant, not as a string: RFC 3339 allows `Z`,
/// `+00:00` and any offset, and lexicographically they do not line up by age.
/// A mark that cannot be parsed is treated as missing.
///
/// Two kinds are never evicted: the current project and any open one. Because
/// of that MAX_PROJECTS stops being a hard size for the map — it holds every
/// open project plus closed ones until the total reaches the limit. The limit
/// was put there against growth from one-off visits, not against what a person
/// opened themselves.
fn trim(projects: &mut BTreeMap<String, ProjectState>, current: Option<&str>, open: &[String]) {
    if projects.len() <= MAX_PROJECTS {
        return;
    }
    let protected =
        |path: &str| current == Some(path) || open.iter().any(|p| p == path);

    let mut ordered: Vec<(String, Option<DateTime<FixedOffset>>)> = projects
        .iter()
        .filter(|(path, _)| !protected(path))
        .map(|(path, state)| {
            let stamp =
                state.used_at.as_deref().and_then(|text| DateTime::parse_from_rfc3339(text).ok());
            (path.clone(), stamp)
        })
        .collect();
    // None is less than any Some, so sorting descending puts the recent ones first.
    ordered.sort_by(|a, b| b.1.cmp(&a.1));

    // The slots taken by the untouchable entries are already spent.
    let taken = projects.len() - ordered.len();
    let keep = MAX_PROJECTS.saturating_sub(taken);
    for (path, _) in ordered.into_iter().skip(keep) {
        projects.remove(&path);
    }
}

impl Settings {
    /// A value outside the allowed set is no reason to throw the file away:
    /// only the field itself is lost.
    pub fn validate(&mut self) {
        one_of(&mut self.agent, &crate::agents::IDS, "claude");
        known_language(&mut self.agent_language);
        known_language(&mut self.task_language);
        known_language(&mut self.commit_language);
        known_language(&mut self.report_language);
        self.appearance.validate();
        self.layout.validate();
        self.editor.validate();
        self.kanban.validate();
        self.notifications.validate();
        for state in self.projects.values_mut() {
            state.validate();
        }
        sane_list(&mut self.open_projects, MAX_OPEN, MAX_PATH_LEN);
        active_in(&mut self.last_project, &self.open_projects);
    }
}

impl ResolvedSettings {
    pub fn validate(&mut self) {
        one_of(&mut self.agent, &crate::agents::IDS, "claude");
        known_language(&mut self.agent_language);
        known_language(&mut self.task_language);
        known_language(&mut self.commit_language);
        known_language(&mut self.report_language);
        self.appearance.validate();
        self.layout.validate();
        self.editor.validate();
        self.kanban.validate();
        self.notifications.validate();
        self.project.validate();
        sane_list(&mut self.open_projects, MAX_OPEN, MAX_PATH_LEN);
        active_in(&mut self.active_project, &self.open_projects);
    }
}

impl RunDefaults {
    fn validate(&mut self) {
        one_of(&mut self.mode, &RUN_MODES, "auto");
        forget_if_junk(&mut self.target_branch, MAX_PATH_LEN);
        // Out of range is forgotten rather than clamped: a 9 in this field is
        // not somebody meaning "the lowest priority", it is a file that has
        // been edited wrongly, and guessing which way they meant it is how a
        // run silently takes work nobody wanted taken. Forgotten rather than
        // replaced with a number of ours, too — with no floor remembered the
        // dialog falls back to the project's own configured default, which is
        // a real answer, where a 2 from here would be an invention.
        if self.min_priority.is_some_and(|floor| floor > MAX_PRIORITY) {
            self.min_priority = None;
        }
    }
}

impl Appearance {
    fn validate(&mut self) {
        one_of(&mut self.theme, &THEMES, "dark");
        one_of(&mut self.density, &DENSITIES, "comfortable");
        font_in_range(&mut self.ui_font_size, UI_FONT_DEFAULT);
    }
}

impl EditorSettings {
    fn validate(&mut self) {
        font_in_range(&mut self.font_size, EDITOR_FONT_DEFAULT);
    }
}

/// A hand-edited size outside what the dropdown offers takes the shipped one —
/// the same rule as `one_of` and `in_range`: the field is damaged, not the
/// section around it. Clamping instead would keep a `2` as a `10` and leave a
/// file claiming a size nobody picked.
fn font_in_range(value: &mut u32, fallback: u32) {
    if !(MIN_FONT..=MAX_FONT).contains(value) {
        *value = fallback;
    }
}

impl Layout {
    fn validate(&mut self) {
        in_range(&mut self.left_width, LEFT_WIDTH_DEFAULT);
        in_range(&mut self.right_width, RIGHT_WIDTH_DEFAULT);
        self.git_sections.validate();
    }
}

impl GitSections {
    fn validate(&mut self) {
        forget_odd_rows(&mut self.repos_rows);
        forget_odd_rows(&mut self.branch_rows);
        // Back to the shipped height rather than forgotten, since there is no
        // "let it size itself" for this one to be handed back to — `in_range`'s
        // rule and not `forget_odd_rows`'s, for a field that is not an Option.
        if !(MIN_COMMIT_ROWS..=MAX_COMMIT_ROWS).contains(&self.commit_rows) {
            self.commit_rows = COMMIT_ROWS_DEFAULT;
        }
    }
}

/// Out of range is forgotten rather than clamped, the rule `min_priority`
/// follows: a 900 in this field is not somebody meaning "as tall as it goes",
/// it is a file that has been edited wrongly, and forgetting it hands the
/// section back to its own content — which is a real answer, where a number of
/// ours would be an invention.
fn forget_odd_rows(value: &mut Option<u32>) {
    if value.is_some_and(|rows| !(MIN_SECTION_ROWS..=MAX_SECTION_ROWS).contains(&rows)) {
        *value = None;
    }
}

/// A value out of range loses the field and takes the default — the same rule
/// as `one_of`: one field is damaged, not the whole section.
fn in_range(value: &mut u32, fallback: u32) {
    if !(MIN_PANEL_WIDTH..=MAX_PANEL_WIDTH).contains(value) {
        *value = fallback;
    }
}

impl ProjectState {
    fn validate(&mut self) {
        one_of(&mut self.side_tab, &SIDE_TABS, "files");
        one_of(&mut self.right_tab, &RIGHT_TABS, "task");
        forget_if_junk(&mut self.selected_task, MAX_ID_LEN);
        // Ids, so the identifier ceiling — and the same cleaning every other
        // list here gets: empty and duplicate entries out, the length capped at
        // what the front end itself keeps. A hand-edited file with thirty of
        // them is trimmed rather than refused; the list is a convenience and no
        // entry in it is load-bearing.
        sane_list(&mut self.recent_tasks, MAX_RECENT_TASKS, MAX_ID_LEN);
        forget_if_junk(&mut self.selected_path, MAX_PATH_LEN);
        // A path, so the same ceiling. Membership is deliberately not checked:
        // which repositories a project has is not known here, and a name that
        // matches nothing is passed over by the panel anyway — the rule
        // `column_order` keeps one line above.
        forget_if_junk(&mut self.selected_repo, MAX_PATH_LEN);
        sane_list(&mut self.expanded, MAX_EXPANDED, MAX_PATH_LEN);
        // Cleaned in place rather than forgotten as a whole: the list is a
        // record of what somebody unfolded, and one junk entry in it is no
        // reason to refold the rest. An empty list survives — it is the state
        // that says "all of them, folded, on purpose".
        if let Some(folders) = self.branch_folders.as_mut() {
            sane_list(folders, MAX_BRANCH_FOLDERS, MAX_PATH_LEN);
        }
        // A branch name is path-like, so the path ceiling and not the
        // identifier one. Cleaned in place for the reason the folders above
        // are: the list is a record of what somebody marked, and one junk entry
        // is no reason to unmark the rest. There is no membership check here
        // either — see the field's own note.
        sane_list(&mut self.favorite_branches, MAX_FAVORITE_BRANCHES, MAX_PATH_LEN);
        sane_list(&mut self.open_tabs, MAX_OPEN_TABS, MAX_PATH_LEN);
        // A status name, not a path — hence the identifier ceiling. Membership
        // is deliberately not checked: bd's set of statuses is not known here,
        // and a name that matches nothing is passed over by the board anyway.
        sane_list(&mut self.column_order, MAX_COLUMNS, MAX_ID_LEN);
        // A tab id, which for a file tab is a path — hence the path ceiling and
        // not the identifier one `column_order` takes a line above, and hence a
        // count well clear of `MAX_OPEN_TABS`: the diffs and the shell tabs are
        // in this list too and in neither of the other two. Membership is
        // deliberately not checked, for the same reason as the column order: an
        // id that matches no tab is passed over by the row.
        sane_list(&mut self.tab_order, MAX_TAB_ORDER, MAX_PATH_LEN);
        if let Some(run) = self.run_settings.as_mut() {
            run.validate();
        }
        // Off the ladder is forgotten rather than rounded, the same rule
        // `min_priority` keeps: a number nobody could have been warned at is a
        // hand-edited file, and the honest reading of it is that no warning has
        // happened here. Rounding down would keep a warning suppressed on the
        // strength of a value the app never wrote.
        if self.storage_warned_mib.is_some_and(|at| !STORAGE_THRESHOLDS_MIB.contains(&at)) {
            self.storage_warned_mib = None;
        }

        // A preview tab that is not among the open ones cannot exist: it would
        // be drawn in italics over nothing, or replaced in a slot that is not there.
        if self.preview_tab.as_deref().is_some_and(|p| !self.open_tabs.iter().any(|t| t == p)) {
            self.preview_tab = None;
        }

        // There is no closed list of tabs in the centre: `kanban` always exists,
        // `terminal` is a name the front end may or may not be drawing right
        // now, and everything else is an open file. Hence the limit is on path
        // length, not identifier length — a file tab with a long path used to
        // silently become the board on every restart.
        //
        // `terminal` passes whether or not that tab exists, and deliberately.
        // It is drawn only while the project has an agent session, and sessions
        // do not survive a restart, so on every launch this value names a tab
        // that is not there yet — but how many sessions there are is not known
        // here, and rewriting the value would destroy the only place the `chat`
        // migration below can land. The front end repairs it instead, in
        // `restoreTabs` (`src/stores/tabs.js`), beside the identical repair for
        // a diff tab. A terminal tab's own id is a different case and needs
        // nothing: it begins with a zero byte, so it is neither of the two names
        // nor any file path, and falls through to the board below.
        //
        // `chat` was this tab's name before it grew a terminal. Files with that
        // name already sit on people's disks, and without the substitution the
        // tab would fail the check below and silently become the board.
        //
        // Only when nothing open is called `chat`, though: a project with a
        // file of that name at its root has an active tab that means exactly
        // what it says, and migrating it would take a person off their own
        // file. An open tab is evidence; the old name is only a guess.
        if self.active_tab == "chat" && !self.open_tabs.iter().any(|t| t == "chat") {
            self.active_tab = "terminal".into();
        }
        let known = self.active_tab == "terminal"
            || self.active_tab == "kanban"
            || self.open_tabs.iter().any(|t| *t == self.active_tab);
        if !known || self.active_tab.len() > MAX_PATH_LEN {
            self.active_tab = "kanban".into();
        }
    }
}

fn one_of(value: &mut String, allowed: &[&str], fallback: &str) {
    if !allowed.contains(&value.as_str()) {
        *value = fallback.to_owned();
    }
}

/// One language id off the file, before validation. A missing field is the
/// ordinary case — every file written before these two existed has neither.
fn language_field(object: &Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| crate::agents::DEFAULT_LANGUAGE.to_owned())
}

/// `one_of` for a language, and it takes the same shape for the same reason:
/// a value nobody ships loses that one field rather than the section around it.
/// The list itself is `agents::LANGUAGES` and is asked rather than repeated,
/// exactly as `agents::IDS` is above.
fn known_language(value: &mut String) {
    if !crate::agents::known_language(value) {
        *value = crate::agents::DEFAULT_LANGUAGE.to_owned();
    }
}

/// An empty string arrives from the front end as "nothing is selected", an
/// overlong one as garbage. Both are better forgotten than kept.
fn forget_if_junk(value: &mut Option<String>, max: usize) {
    if let Some(text) = value {
        if text.is_empty() || text.len() > max {
            *value = None;
        }
    }
}

/// A list of names — paths, statuses — from the file or from the front end.
/// Empty strings and overlong ones are garbage, duplicates are pointless, and
/// the length is capped. `max_item` differs by what the list holds: a path may
/// legitimately be thousands of characters, a status name may not.
fn sane_list(items: &mut Vec<String>, max: usize, max_item: usize) {
    let mut seen = HashSet::new();
    items.retain(|item| !item.is_empty() && item.len() <= max_item && seen.insert(item.clone()));
    items.truncate(max);
}

/// The active project must be in the open list: otherwise the board would show
/// something the list does not contain, and no row would be highlighted. An
/// empty list is a legitimate "there are no projects", not a breakage.
fn active_in(active: &mut Option<String>, open: &[String]) {
    let known = active.as_deref().is_some_and(|path| open.iter().any(|p| p == path));
    if !known {
        *active = open.first().cloned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_of(text: &str) -> Settings {
        match parse(text) {
            Outcome::Ok(settings) => settings,
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn missing_version_reads_as_the_first_schema() {
        let settings = settings_of(r#"{"appearance":{"theme":"light","density":"compact"}}"#);
        assert_eq!(settings.version, CURRENT_VERSION);
        assert_eq!(settings.appearance.theme, "light");
        assert_eq!(settings.appearance.density, "compact");
    }

    #[test]
    fn newer_version_is_not_read() {
        assert_eq!(parse(r#"{"version":99,"appearance":{"theme":"light"}}"#), Outcome::TooNew);
    }

    #[test]
    fn broken_json_is_not_settings() {
        assert_eq!(parse("{not json"), Outcome::Broken);
        assert_eq!(parse("[1,2,3]"), Outcome::Broken);
    }

    #[test]
    fn unknown_value_falls_back_field_by_field() {
        let settings = settings_of(r#"{"version":1,"appearance":{"theme":"neon","density":"compact"}}"#);
        assert_eq!(settings.appearance.theme, "dark");
        assert_eq!(settings.appearance.density, "compact", "the neighbouring field must survive");
    }

    #[test]
    fn system_is_a_theme_the_file_keeps_as_it_stands() {
        // The word is the whole point: resolving it to dark on the way in would
        // freeze the app at whatever the machine happened to say once.
        let settings = settings_of(r#"{"version":1,"appearance":{"theme":"system"}}"#);
        assert_eq!(settings.appearance.theme, "system");

        let mut written = Settings::default();
        merge(&mut written, resolve(&settings, None), "2026-08-01T00:00:00+00:00".into());
        assert_eq!(written.appearance.theme, "system", "and it survives the way back to disk");
    }

    #[test]
    fn a_file_written_before_the_font_sizes_opens_at_the_shipped_ones() {
        // Every settings file on a person's disk right now is this file.
        let settings = settings_of(r#"{"version":1,"appearance":{"theme":"light"}}"#);
        assert_eq!(settings.appearance.ui_font_size, UI_FONT_DEFAULT);
        assert_eq!(settings.editor.font_size, EDITOR_FONT_DEFAULT);
    }

    #[test]
    fn a_font_size_outside_the_range_loses_only_itself() {
        let settings = settings_of(
            r#"{"version":1,"appearance":{"uiFontSize":80,"density":"compact"},"editor":{"fontSize":2}}"#,
        );
        assert_eq!(settings.appearance.ui_font_size, UI_FONT_DEFAULT);
        assert_eq!(settings.editor.font_size, EDITOR_FONT_DEFAULT);
        assert_eq!(settings.appearance.density, "compact", "the neighbouring field must survive");
    }

    #[test]
    fn both_ends_of_the_range_are_legal_sizes() {
        let settings = settings_of(
            r#"{"version":1,"appearance":{"uiFontSize":10},"editor":{"fontSize":24}}"#,
        );
        assert_eq!(settings.appearance.ui_font_size, 10);
        assert_eq!(settings.editor.font_size, 24);
    }

    /// The same walk `a_chosen_agent_does_not_quietly_become_claude_again` makes,
    /// and for the same reason: a section added to the structs but not wired into
    /// `parse`, `resolve` and `merge` reads as the default forever, and the
    /// struct-alone tests cannot see it.
    #[test]
    fn the_font_sizes_survive_disk_to_front_end_and_back() {
        let file = settings_of(
            r#"{"version":1,"appearance":{"uiFontSize":16},"editor":{"fontSize":18}}"#,
        );
        assert_eq!(file.appearance.ui_font_size, 16, "parse must read them off the disk");
        assert_eq!(file.editor.font_size, 18);

        let resolved = resolve(&file, None);
        assert_eq!(resolved.appearance.ui_font_size, 16, "resolve must carry them to the front end");
        assert_eq!(resolved.editor.font_size, 18);

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-01T00:00:00+00:00".into());
        assert_eq!(written.appearance.ui_font_size, 16, "merge must carry them back into the file");
        assert_eq!(written.editor.font_size, 18);
    }

    /// Default on, because a feature that does nothing until somebody finds a
    /// switch is a feature nobody finds. The switch exists for the machines where
    /// background network is not free — a metered connection, a VPN that is not
    /// always up, a key with a passphrase that would fail on every sweep.
    #[test]
    fn auto_fetch_defaults_on_and_a_stored_false_survives_the_merge() {
        assert!(Settings::default().git.auto_fetch);
        // Every settings file on a person's disk right now is this file.
        assert!(settings_of(r#"{"version":1,"appearance":{"theme":"light"}}"#).git.auto_fetch);

        let file = settings_of(r#"{"version":1,"git":{"autoFetch":false}}"#);
        assert!(!file.git.auto_fetch, "parse must read it off the disk");

        let resolved = resolve(&file, None);
        assert!(!resolved.git.auto_fetch, "resolve must carry it to the front end");

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-01T00:00:00+00:00".into());
        assert!(!written.git.auto_fetch, "merge must carry it back into the file");
    }

    /// Default on, because it is today's behaviour to the letter: the window
    /// has always opened where it was left, and the switch is over something
    /// that already happens. Wired through `parse`, `resolve` and `merge` here
    /// rather than asserted on the struct alone — a section added to the types
    /// and missed in one of the three reads as the default for ever, and no
    /// struct-alone test sees it.
    #[test]
    fn restoring_the_geometry_defaults_on_and_a_stored_false_survives_the_merge() {
        assert!(Settings::default().window.restore_geometry);
        // Every settings file on a person's disk right now is this file.
        assert!(settings_of(r#"{"version":1,"appearance":{"theme":"light"}}"#).window.restore_geometry);

        let file = settings_of(r#"{"version":1,"window":{"restoreGeometry":false}}"#);
        assert!(!file.window.restore_geometry, "parse must read it off the disk");

        let resolved = resolve(&file, None);
        assert!(!resolved.window.restore_geometry, "resolve must carry it to the front end");

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-01T00:00:00+00:00".into());
        assert!(!written.window.restore_geometry, "merge must carry it back into the file");
    }

    /// Default on, because an app that never asks is an app whose update
    /// system does not exist for anybody who does not go looking. The walk is
    /// `restoreGeometry`'s and for its reason: a section added to the types and
    /// missed in one of `parse`, `resolve` and `merge` reads as the default for
    /// ever, and no struct-alone test sees it.
    #[test]
    fn checking_for_updates_defaults_on_and_a_stored_false_survives_the_merge() {
        assert!(Settings::default().updates.auto_check);
        // Every settings file written by a build before this switch existed,
        // which is every file on a person's disk right now.
        assert!(settings_of(r#"{"version":1,"appearance":{"theme":"light"}}"#).updates.auto_check);

        let file = settings_of(r#"{"version":1,"updates":{"autoCheck":false}}"#);
        assert!(!file.updates.auto_check, "parse must read it off the disk");

        let resolved = resolve(&file, None);
        assert!(!resolved.updates.auto_check, "resolve must carry it to the front end");

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-01T00:00:00+00:00".into());
        assert!(!written.updates.auto_check, "merge must carry it back into the file");
    }

    /// The same failure `a_window_section_of_the_wrong_type_falls_back_to_restoring`
    /// pins, one section over and for the same reason: `false` is a real answer
    /// here, so damage must land on the shipped `true` rather than on the
    /// `false` a bool deserializes to when nobody was asked.
    #[test]
    fn an_updates_section_of_the_wrong_type_falls_back_to_checking() {
        let settings =
            settings_of(r#"{"version":1,"updates":"daily","appearance":{"theme":"light"}}"#);
        assert_eq!(settings.updates, UpdateSettings::default());
        assert!(settings.updates.auto_check);
        assert_eq!(settings.appearance.theme, "light", "the neighbouring section must survive");

        let field = settings_of(r#"{"version":1,"updates":{"autoCheck":"no"}}"#);
        assert!(field.updates.auto_check, "a field of the wrong type loses the section");
    }

    /// A section whose type is wrong loses the whole section to its defaults,
    /// the way `kanban` does one field over: `false` is a real answer here, so
    /// the failure has to land on the shipped `true` rather than on the `false`
    /// a bool deserializes to when nobody was asked.
    #[test]
    fn a_window_section_of_the_wrong_type_falls_back_to_restoring() {
        let settings =
            settings_of(r#"{"version":1,"window":"remember","appearance":{"theme":"light"}}"#);
        assert_eq!(settings.window, WindowSettings::default());
        assert!(settings.window.restore_geometry);
        assert_eq!(settings.appearance.theme, "light", "the neighbouring section must survive");

        let field = settings_of(r#"{"version":1,"window":{"restoreGeometry":"no"}}"#);
        assert!(field.window.restore_geometry, "a field of the wrong type loses the section");
    }

    /// Default on, because that is today's behaviour exactly: the running-tasks
    /// skill removes a merged task's worktree unconditionally, so a switch that
    /// shipped off would change what the app does the moment it was added.
    ///
    /// The neighbour is asserted in the same walk on purpose. `git` is read as
    /// a whole section, so a field added to the struct and a file written
    /// before it existed have to leave `auto_fetch` alone — the failure a
    /// per-field test would not see is the two of them arriving together.
    #[test]
    fn removing_worktrees_defaults_on_and_a_stored_false_survives_beside_auto_fetch() {
        assert!(Settings::default().git.remove_worktrees);
        // Every settings file on a person's disk right now is this file: it has
        // `autoFetch` and no `removeWorktrees` at all.
        let older = settings_of(r#"{"version":1,"git":{"autoFetch":false}}"#);
        assert!(!older.git.auto_fetch);
        assert!(older.git.remove_worktrees, "a key the file never had is the shipped answer");

        let file = settings_of(r#"{"version":1,"git":{"autoFetch":false,"removeWorktrees":false}}"#);
        assert!(!file.git.remove_worktrees, "parse must read it off the disk");
        assert!(!file.git.auto_fetch, "and must not lose its neighbour doing it");

        let resolved = resolve(&file, None);
        assert!(!resolved.git.remove_worktrees, "resolve must carry it to the front end");
        assert!(!resolved.git.auto_fetch);

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-01T00:00:00+00:00".into());
        assert!(!written.git.remove_worktrees, "merge must carry it back into the file");
        assert!(!written.git.auto_fetch);
    }

    /// Shipped on rather than off, and as two different sounds, for the reason
    /// `NotificationSettings` records. The walk is the one the font sizes make:
    /// a section added to the structs but not wired into `parse`, `resolve` and
    /// `merge` reads as the default for ever, and no struct-alone test sees it.
    #[test]
    fn notification_sounds_default_on_and_survive_the_round_trip() {
        let shipped = Settings::default();
        assert_eq!(shipped.notifications.run_finished, "sound-1");
        assert_eq!(shipped.notifications.needs_attention, "sound-2");

        let file = settings_of(
            r#"{"version":1,"notifications":{"runFinished":"off","needsAttention":"sound-4"}}"#,
        );
        assert_eq!(file.notifications.run_finished, "off", "parse must read it off the disk");
        assert_eq!(file.notifications.needs_attention, "sound-4");

        let resolved = resolve(&file, None);
        assert_eq!(
            resolved.notifications.run_finished, "off",
            "resolve must carry it to the front end"
        );

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-22T10:00:00Z".into());
        assert_eq!(
            written.notifications.run_finished, "off",
            "merge must carry it back into the file"
        );
        assert_eq!(written.notifications.needs_attention, "sound-4");
    }

    /// Shipped on, and the same walk the sounds beside it make: the field
    /// shares their section, so a section wired for two values and read for
    /// three would answer `true` for ever and no struct-alone test would see it.
    #[test]
    fn showing_the_report_defaults_on_and_survives_the_round_trip() {
        let shipped = Settings::default();
        assert!(shipped.notifications.show_report, "the shipped answer is today's behaviour");

        let file = settings_of(r#"{"version":1,"notifications":{"showReport":false}}"#);
        assert!(!file.notifications.show_report, "parse must read it off the disk");
        assert_eq!(
            file.notifications.run_finished, "sound-1",
            "and must leave the sounds beside it at their defaults"
        );

        let resolved = resolve(&file, None);
        assert!(!resolved.notifications.show_report, "resolve must carry it to the front end");

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-22T10:00:00Z".into());
        assert!(!written.notifications.show_report, "merge must carry it back into the file");
    }

    /// Shipped on, and the same walk the fields beside it make. The default is
    /// the one here that changes today's behaviour rather than keeping it, so
    /// this test is also where that decision is written down: somebody watching
    /// the app stops hearing the two sounds, which is what was asked for.
    #[test]
    fn playing_only_when_unfocused_defaults_on_and_survives_the_round_trip() {
        let shipped = Settings::default();
        assert!(
            shipped.notifications.only_when_unfocused,
            "a sound is for the person who is not looking at the screen"
        );

        let file = settings_of(r#"{"version":1,"notifications":{"onlyWhenUnfocused":false}}"#);
        assert!(!file.notifications.only_when_unfocused, "parse must read it off the disk");
        assert_eq!(
            file.notifications.run_finished, "sound-1",
            "and must leave the sounds beside it at their defaults"
        );
        assert!(
            file.notifications.show_report,
            "and the other switch in the section with it"
        );

        let resolved = resolve(&file, None);
        assert!(
            !resolved.notifications.only_when_unfocused,
            "resolve must carry it to the front end"
        );

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-23T10:00:00Z".into());
        assert!(
            !written.notifications.only_when_unfocused,
            "merge must carry it back into the file"
        );
    }

    #[test]
    fn a_file_written_before_the_switch_existed_opens_with_the_report_showing() {
        // Every settings file on a person's disk right now is this file, and
        // the whole reason the default is `true`.
        let settings = settings_of(r#"{"version":1,"notifications":{"runFinished":"off"}}"#);
        assert!(settings.notifications.show_report);
        assert_eq!(settings.notifications.run_finished, "off");
    }

    #[test]
    fn a_sound_nobody_ships_loses_its_field_and_leaves_the_other_alone() {
        // The rule the whole schema follows: the field is damaged, not the
        // section around it. A hand-edited file naming a fifth sound gets the
        // shipped one back for that event and keeps its choice for the other.
        let settings = settings_of(
            r#"{"version":1,"notifications":{"runFinished":"sound-9","needsAttention":"off"}}"#,
        );
        assert_eq!(settings.notifications.run_finished, "sound-1");
        assert_eq!(settings.notifications.needs_attention, "off");
    }

    #[test]
    fn a_file_written_before_the_section_existed_opens_with_both_sounds() {
        // Every settings file on a person's disk right now is this file.
        let settings = settings_of(r#"{"version":1,"appearance":{"theme":"light"}}"#);
        assert_eq!(settings.notifications.run_finished, "sound-1");
        assert_eq!(settings.notifications.needs_attention, "sound-2");
        assert!(
            settings.notifications.show_report,
            "and with its report showing, which is what the app did before the switch existed"
        );
        assert!(
            settings.notifications.only_when_unfocused,
            "and holding those sounds while somebody is looking, which is the change \
             this default makes on purpose"
        );
    }

    #[test]
    fn a_broken_editor_section_does_not_take_the_rest_of_the_file() {
        let settings = settings_of(r#"{"version":1,"editor":"large","appearance":{"theme":"light"}}"#);
        assert_eq!(settings.editor, EditorSettings::default());
        assert_eq!(settings.appearance.theme, "light");
    }

    #[test]
    fn a_file_written_before_word_wrap_keeps_the_editor_it_always_had() {
        // Every settings file on a person's disk right now is this file: an
        // `editor` section with a size in it and no `wordWrap`. The field takes
        // its default rather than the section losing itself, and the default is
        // today's behaviour — long lines scroll sideways.
        let settings = settings_of(r#"{"version":1,"editor":{"fontSize":18}}"#);
        assert_eq!(settings.editor.font_size, 18, "the section survives the missing field");
        assert!(!settings.editor.word_wrap, "a missing wordWrap is off, not a lost section");
    }

    /// The walk `the_font_sizes_survive_disk_to_front_end_and_back` makes, for
    /// the reason written over it: a field added to the struct but not carried
    /// by `parse`, `resolve` and `merge` reads as its default for ever.
    #[test]
    fn word_wrap_survives_disk_to_front_end_and_back() {
        let file = settings_of(r#"{"version":1,"editor":{"fontSize":14,"wordWrap":true}}"#);
        assert!(file.editor.word_wrap, "parse must read it off the disk");

        let resolved = resolve(&file, None);
        assert!(resolved.editor.word_wrap, "resolve must carry it to the front end");

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-01T00:00:00+00:00".into());
        assert!(written.editor.word_wrap, "merge must carry it back into the file");
    }

    #[test]
    fn a_file_written_before_the_board_settings_draws_the_board_it_always_did() {
        // Every settings file on a person's disk right now is this file, and
        // the shipped values have to be today's board exactly: every column,
        // every task.
        let settings = settings_of(r#"{"version":1,"appearance":{"theme":"light"}}"#);
        assert_eq!(settings.kanban, KanbanSettings::default());
        assert_eq!(settings.kanban.columns, "all");
        assert_eq!(settings.kanban.interval, "all");
        assert!(settings.kanban.always_show.is_empty());
        assert!(settings.kanban.unlimited.is_empty());
    }

    #[test]
    fn a_board_value_outside_its_closed_list_loses_only_itself() {
        let settings = settings_of(
            r#"{"version":1,"kanban":{"columns":"some","interval":"week","alwaysShow":["ready"]}}"#,
        );
        assert_eq!(settings.kanban.columns, "all");
        assert_eq!(settings.kanban.interval, "week", "the neighbouring field must survive");
        assert_eq!(settings.kanban.always_show, vec!["ready".to_string()]);

        let settings = settings_of(r#"{"version":1,"kanban":{"columns":"non-empty","interval":"fortnight"}}"#);
        assert_eq!(settings.kanban.interval, "all");
        assert_eq!(settings.kanban.columns, "non-empty", "the neighbouring field must survive");
    }

    #[test]
    fn a_board_section_of_the_wrong_type_is_lost_whole() {
        let settings =
            settings_of(r#"{"version":1,"kanban":"compact","appearance":{"theme":"light"}}"#);
        assert_eq!(settings.kanban, KanbanSettings::default());
        assert_eq!(settings.appearance.theme, "light", "and it takes nothing with it");
    }

    #[test]
    fn the_board_column_lists_lose_blanks_duplicates_and_overlong_names() {
        let long = "x".repeat(MAX_ID_LEN + 1);
        let text = serde_json::json!({
            "version": 1,
            "kanban": { "alwaysShow": ["ready", "ready", "", long, "done"], "unlimited": ["ready"] }
        });

        let settings = settings_of(&text.to_string());

        assert_eq!(settings.kanban.always_show, vec!["ready".to_string(), "done".to_string()]);
        assert_eq!(settings.kanban.unlimited, vec!["ready".to_string()]);
    }

    /// The same walk the font sizes make above, and for the same reason: a
    /// section added to the structs but not wired into `parse`, `resolve` and
    /// `merge` reads as the default forever, and no struct-alone test sees it.
    #[test]
    fn the_board_settings_survive_disk_to_front_end_and_back() {
        let file = settings_of(
            r#"{"version":1,"kanban":{"columns":"non-empty","alwaysShow":["ready"],
                "interval":"day","unlimited":["done"]}}"#,
        );
        assert_eq!(file.kanban.columns, "non-empty", "parse must read them off the disk");

        let resolved = resolve(&file, None);
        assert_eq!(resolved.kanban.interval, "day", "resolve must carry them to the front end");
        assert_eq!(resolved.kanban.unlimited, vec!["done".to_string()]);

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-01T00:00:00+00:00".into());
        assert_eq!(written.kanban.columns, "non-empty", "merge must carry them back into the file");
        assert_eq!(written.kanban.always_show, vec!["ready".to_string()]);
        assert_eq!(written.kanban.interval, "day");
    }

    #[test]
    fn a_board_value_the_front_end_should_not_have_sent_does_not_reach_the_file() {
        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            kanban: KanbanSettings {
                columns: "some".into(),
                interval: "week".into(),
                always_show: vec!["ready".into(), "ready".into()],
                unlimited: Vec::new(),
            },
            ..ResolvedSettings::default()
        };

        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());

        assert_eq!(file.kanban.columns, "all", "validated on the way in, not only on the way out");
        assert_eq!(file.kanban.interval, "week");
        assert_eq!(file.kanban.always_show, vec!["ready".to_string()], "the duplicate fell out");
    }

    #[test]
    fn broken_section_does_not_take_the_rest_of_the_file() {
        let settings = settings_of(r#"{"version":1,"appearance":5,"layout":{"leftCollapsed":true}}"#);
        assert_eq!(settings.appearance, Appearance::default());
        assert!(settings.layout.left_collapsed);
    }

    #[test]
    fn a_file_without_widths_opens_at_the_shipped_ones() {
        // A file written before the panels learned to be dragged.
        let settings = settings_of(r#"{"version":1,"layout":{"leftCollapsed":true}}"#);
        assert!(settings.layout.left_collapsed);
        assert_eq!(settings.layout.left_width, LEFT_WIDTH_DEFAULT);
        assert_eq!(settings.layout.right_width, RIGHT_WIDTH_DEFAULT);
    }

    #[test]
    fn a_width_out_of_range_loses_only_itself() {
        let settings = settings_of(r#"{"version":1,"layout":{"leftWidth":0,"rightWidth":300}}"#);
        assert_eq!(settings.layout.left_width, LEFT_WIDTH_DEFAULT);
        assert_eq!(settings.layout.right_width, 300, "the neighbouring field must survive");

        let huge = settings_of(r#"{"version":1,"layout":{"leftWidth":999999}}"#);
        assert_eq!(huge.layout.left_width, LEFT_WIDTH_DEFAULT);
    }

    #[test]
    fn a_width_wider_than_any_window_survives_the_trip() {
        // The check catches garbage, not tightness: the front end fits the width
        // to the window, and a number that does not fit today must survive to
        // reach a wide monitor.
        let settings = settings_of(r#"{"version":1,"layout":{"leftWidth":1200}}"#);
        assert_eq!(settings.layout.left_width, 1200);
    }

    #[test]
    fn a_file_without_git_sections_opens_with_all_three_unfolded() {
        // A file written before the Git panel's sections learned to fold. Every
        // section open and neither height dragged is the panel exactly as it was
        // before any of this — an update must not fold something away.
        let settings = settings_of(r#"{"version":1,"layout":{"leftWidth":300}}"#);
        let git = &settings.layout.git_sections;
        assert!(git.repos_open && git.changes_open && git.branches_open);
        assert_eq!(git.repos_rows, None);
        assert_eq!(git.branch_rows, None);
        // And the commit box at the height it was fixed at before it could be
        // dragged, for the same reason: an update must not resize a field
        // somebody never touched.
        assert_eq!(git.commit_rows, COMMIT_ROWS_DEFAULT);
    }

    #[test]
    fn a_dragged_section_height_survives_the_trip() {
        let settings =
            settings_of(r#"{"version":1,"layout":{"gitSections":{"branchRows":12,"changesOpen":false}}}"#);
        assert_eq!(settings.layout.git_sections.branch_rows, Some(12));
        assert!(!settings.layout.git_sections.changes_open);
        assert!(settings.layout.git_sections.repos_open, "the neighbouring fold must survive");
    }

    #[test]
    fn a_section_height_out_of_range_is_forgotten_rather_than_clamped() {
        // Forgotten hands the section back to its own content, which is a real
        // answer; a number of ours would be an invention. And it loses only
        // itself — the field beside it is untouched.
        let settings =
            settings_of(r#"{"version":1,"layout":{"gitSections":{"reposRows":900,"branchRows":6}}}"#);
        assert_eq!(settings.layout.git_sections.repos_rows, None);
        assert_eq!(settings.layout.git_sections.branch_rows, Some(6));

        let tiny = settings_of(r#"{"version":1,"layout":{"gitSections":{"reposRows":1}}}"#);
        assert_eq!(tiny.layout.git_sections.repos_rows, None);
    }

    #[test]
    fn a_dragged_message_field_survives_the_trip_and_an_absurd_one_goes_back_to_two() {
        let settings = settings_of(r#"{"version":1,"layout":{"gitSections":{"commitRows":8}}}"#);
        assert_eq!(settings.layout.git_sections.commit_rows, 8);

        // Not forgotten the way a section's height is: there is no "follow the
        // content" for a `<textarea>` to be handed back to, so the shipped
        // height is the only honest answer.
        let absurd = settings_of(r#"{"version":1,"layout":{"gitSections":{"commitRows":900}}}"#);
        assert_eq!(absurd.layout.git_sections.commit_rows, COMMIT_ROWS_DEFAULT);

        let none = settings_of(r#"{"version":1,"layout":{"gitSections":{"commitRows":0}}}"#);
        assert_eq!(none.layout.git_sections.commit_rows, COMMIT_ROWS_DEFAULT);
    }

    #[test]
    fn a_settings_file_written_before_the_rail_keeps_it_open() {
        // The rail shipped after this struct did, so every file already on disk
        // is missing the field. Open is the shipped state, and a person who
        // never asked for anything must not have it taken away by an upgrade.
        let stored = settings_of(r#"{"version":1,"layout":{"leftWidth":300}}"#);
        assert!(stored.layout.rail_open);
        assert_eq!(stored.layout.left_width, 300, "the neighbouring field must survive");

        let hidden = settings_of(r#"{"version":1,"layout":{"railOpen":false}}"#);
        assert!(!hidden.layout.rail_open, "a stored flag survives the load");
    }

    #[test]
    fn unknown_side_tab_falls_back_to_files() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"sideTab":"tarot","activeTab":"terminal"}}}"#);
        let state = &settings.projects["/p"];
        assert_eq!(state.side_tab, "files");
        assert_eq!(state.active_tab, "terminal", "a tab outside the closed list stays");
    }

    #[test]
    fn unknown_right_tab_falls_back_to_task() {
        // The same rule one column over, and the same reason to pin it: the
        // right column's row is a closed list written out twice, and a value
        // off it must lose itself here rather than reach the panel.
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"rightTab":"tarot","sideTab":"git"}}}"#);
        let state = &settings.projects["/p"];
        assert_eq!(state.right_tab, "task");
        assert_eq!(state.side_tab, "git", "one damaged field does not take its neighbour");
    }

    #[test]
    fn a_stored_sessions_right_tab_survives_the_load() {
        // The other half: the tab a person left the panel on comes back, which
        // is the whole point of storing it. A file written before the field
        // existed has none, and takes the default beside it.
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"rightTab":"sessions"},"/old":{"sideTab":"git"}}}"#);
        assert_eq!(settings.projects["/p"].right_tab, "sessions");
        assert_eq!(settings.projects["/old"].right_tab, "task");
    }

    #[test]
    fn a_broken_project_entry_does_not_take_its_neighbours() {
        let settings = settings_of(r#"{"version":1,"projects":{"/bad":7,"/good":{"sideTab":"agents"}}}"#);
        assert!(!settings.projects.contains_key("/bad"));
        assert_eq!(settings.projects["/good"].side_tab, "agents");
    }

    #[test]
    fn merge_writes_into_the_current_project_and_stamps_it() {
        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            appearance: Appearance { theme: "light".into(), ..Appearance::default() },
            layout: Layout { left_collapsed: true, left_width: 420, ..Layout::default() },
            project: ProjectState {
                selected_task: Some("bd-a1b2".into()),
                ..ProjectState::default()
            },
            open_projects: vec!["/work/smetana".into()],
            active_project: Some("/work/smetana".into()),
            ..ResolvedSettings::default()
        };

        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());

        assert_eq!(file.version, CURRENT_VERSION);
        assert_eq!(file.appearance.theme, "light");
        assert!(file.layout.left_collapsed);
        assert_eq!(file.layout.left_width, 420);
        assert_eq!(file.last_project.as_deref(), Some("/work/smetana"));
        let state = &file.projects["/work/smetana"];
        assert_eq!(state.selected_task.as_deref(), Some("bd-a1b2"));
        assert_eq!(state.used_at.as_deref(), Some("2026-08-01T09:12:00+00:00"));
    }

    #[test]
    fn a_value_the_front_end_should_not_have_sent_does_not_reach_the_file() {
        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            appearance: Appearance { theme: "neon".into(), ..Appearance::default() },
            project: ProjectState { side_tab: "tarot".into(), ..ProjectState::default() },
            open_projects: vec!["/p".into()],
            active_project: Some("/p".into()),
            ..ResolvedSettings::default()
        };

        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());

        assert_eq!(file.appearance.theme, "dark", "validated on the way in, not only on the way out");
        assert_eq!(file.projects["/p"].side_tab, "files");
    }

    #[test]
    fn the_expanded_list_loses_blanks_duplicates_and_everything_past_the_limit() {
        let mut paths = vec![String::from("/a"), String::from("/a"), String::new(), "x".repeat(MAX_PATH_LEN + 1)];
        for i in 0..MAX_EXPANDED {
            paths.push(format!("/dir{i:04}"));
        }
        let text = serde_json::json!({"version": 1, "projects": {"/p": {"expanded": paths}}});

        let settings = settings_of(&text.to_string());

        let expanded = &settings.projects["/p"].expanded;
        let last_kept = format!("/dir{:04}", MAX_EXPANDED - 2);
        assert_eq!(expanded.len(), MAX_EXPANDED, "the list is not stored past the limit");
        assert_eq!(expanded[0], "/a");
        assert_eq!(expanded[1], "/dir0000", "the duplicate, the empty string and the overlong path fell out");
        assert_eq!(expanded.last(), Some(&last_kept), "the tail is trimmed, not the head");
    }

    /// The whole reason the field is an `Option`. A file written before this
    /// existed has no key at all, and that must not read as "everything was
    /// folded on purpose" — the panel would open on a repository with the
    /// current branch hidden inside a folder nobody closed.
    #[test]
    fn a_file_with_no_branch_folders_is_not_a_file_with_none_unfolded() {
        let text = serde_json::json!({"version": 1, "projects": {"/p": {"expanded": []}}});

        let settings = settings_of(&text.to_string());

        assert_eq!(settings.projects["/p"].branch_folders, None);
    }

    /// The other half of it: an empty list is a choice and survives the trip as
    /// one, so folding the last folder away stays folded after a restart.
    #[test]
    fn unfolded_branch_folders_survive_the_trip_and_so_does_an_empty_list() {
        let text = serde_json::json!({
            "version": 1,
            "projects": {
                "/p": {"branchFolders": ["feature", "fix/legacy", "", "feature"]},
                "/q": {"branchFolders": []}
            }
        });

        let settings = settings_of(&text.to_string());

        assert_eq!(
            settings.projects["/p"].branch_folders,
            Some(vec![String::from("feature"), String::from("fix/legacy")]),
            "the blank and the duplicate fall out, the rest keeps its order"
        );
        assert_eq!(
            settings.projects["/q"].branch_folders,
            Some(Vec::new()),
            "folded on purpose is not the same as never chosen"
        );
    }

    /// The list of pinned branches is cleaned where it stands, exactly as the
    /// folders beside it are: the blanks and the repeats go, the survivors keep
    /// the order they were written in, and one junk entry unmarks nothing else.
    #[test]
    fn the_favourite_branches_lose_blanks_and_duplicates_and_keep_their_order() {
        let text = serde_json::json!({
            "version": 1,
            "projects": {
                "/p": {"favoriteBranches": ["main", "", "feature/x", "main", "release/7"]}
            }
        });

        let settings = settings_of(&text.to_string());

        assert_eq!(
            settings.projects["/p"].favorite_branches,
            vec![
                String::from("main"),
                String::from("feature/x"),
                String::from("release/7")
            ]
        );
    }

    /// Unlike its neighbour there is no third state to keep, so a file written
    /// before this existed reads as "nothing is marked" — which is also what an
    /// explicit empty list means.
    #[test]
    fn a_file_with_no_favourite_branches_reads_as_none_marked() {
        let text = serde_json::json!({"version": 1, "projects": {"/p": {"expanded": []}}});

        let settings = settings_of(&text.to_string());

        assert!(settings.projects["/p"].favorite_branches.is_empty());
    }

    /// The ceiling, and the head of the list is what survives it: a name near
    /// the top was marked first and is the one somebody would miss.
    #[test]
    fn the_favourite_branches_stop_at_the_ceiling() {
        let names: Vec<String> =
            (0..MAX_FAVORITE_BRANCHES + 10).map(|i| format!("branch/{i:04}")).collect();
        let text =
            serde_json::json!({"version": 1, "projects": {"/p": {"favoriteBranches": names}}});

        let settings = settings_of(&text.to_string());

        let kept = &settings.projects["/p"].favorite_branches;
        assert_eq!(kept.len(), MAX_FAVORITE_BRANCHES);
        assert_eq!(kept[0], "branch/0000");
        assert_eq!(
            kept.last(),
            Some(&format!("branch/{:04}", MAX_FAVORITE_BRANCHES - 1)),
            "the tail is trimmed, not the head"
        );
    }

    /// A name too long to be a ref is garbage rather than a branch, and it goes
    /// on its own without taking the rest of the list with it.
    #[test]
    fn an_overlong_favourite_branch_falls_out_alone() {
        let long = "x".repeat(MAX_PATH_LEN + 1);
        let text = serde_json::json!({
            "version": 1,
            "projects": {"/p": {"favoriteBranches": ["main", long, "develop"]}}
        });

        let settings = settings_of(&text.to_string());

        assert_eq!(
            settings.projects["/p"].favorite_branches,
            vec![String::from("main"), String::from("develop")]
        );
    }

    #[test]
    fn merge_keeps_only_the_newest_projects() {
        let mut file = Settings::default();
        for i in 0..MAX_PROJECTS + 5 {
            file.projects.insert(
                format!("/p{i:02}"),
                ProjectState {
                    used_at: Some(format!("2026-01-{:02}T00:00:00+00:00", i + 1)),
                    ..ProjectState::default()
                },
            );
        }

        let resolved = ResolvedSettings {
            open_projects: vec!["/p-new".into()],
            active_project: Some("/p-new".into()),
            ..ResolvedSettings::default()
        };
        merge(&mut file, resolved, "2026-08-01T00:00:00+00:00".into());

        assert_eq!(file.projects.len(), MAX_PROJECTS);
        assert!(file.projects.contains_key("/p-new"), "the current project always stays");
        assert!(!file.projects.contains_key("/p00"), "the oldest goes first");
    }

    #[test]
    fn trim_compares_instants_not_strings_and_never_evicts_the_current_project() {
        let mut projects: BTreeMap<String, ProjectState> = BTreeMap::new();

        // The current project carries the oldest mark of all — and still has to
        // stay: it is excluded from the selection structurally, not because its
        // date happened to be the largest (as it was in the old test).
        projects.insert(
            "/current".into(),
            ProjectState { used_at: Some("2000-01-01T00:00:00+00:00".into()), ..ProjectState::default() },
        );

        // Filler — deliberately the most recent entries, not eligible for
        // eviction under any comparison; they take every slot but one.
        let filler_count = MAX_PROJECTS - 2;
        for i in 0..filler_count {
            projects.insert(
                format!("/filler{i:02}"),
                ProjectState {
                    used_at: Some(format!("2026-07-{:02}T00:00:00+00:00", i + 1)),
                    ..ProjectState::default()
                },
            );
        }

        // The instant "/newer-instant" really is later than "/older-instant" —
        // 23:00Z against 22:00Z of the same UTC day (01:00+03:00 is exactly
        // 22:00Z). But as a *string* "2026-05-01..." is greater than
        // "2026-04-30..." — the day digits compare, "05" > "04" — and
        // lexicographic order would line them up the other way round.
        projects.insert(
            "/older-instant".into(),
            ProjectState { used_at: Some("2026-05-01T01:00:00+03:00".into()), ..ProjectState::default() },
        );
        projects.insert(
            "/newer-instant".into(),
            ProjectState { used_at: Some("2026-04-30T23:00:00Z".into()), ..ProjectState::default() },
        );

        assert_eq!(projects.len(), MAX_PROJECTS + 1, "check on the way in: trimming must happen");

        trim(&mut projects, Some("/current"), &[]);

        assert_eq!(projects.len(), MAX_PROJECTS);
        assert!(projects.contains_key("/current"), "the current project is never evicted");
        assert!(
            projects.contains_key("/newer-instant"),
            "the later instant stays even when its string is lexicographically smaller"
        );
        assert!(
            !projects.contains_key("/older-instant"),
            "the earlier instant goes even when its string is lexicographically larger"
        );
        for i in 0..filler_count {
            let path = format!("/filler{i:02}");
            assert!(projects.contains_key(&path), "deliberately recent entries should not have been trimmed");
        }
    }

    #[test]
    fn resolve_gives_defaults_for_an_unknown_project() {
        let file = settings_of(r#"{"version":1,"projects":{"/other":{"sideTab":"agents"}}}"#);
        assert_eq!(resolve(&file, Some("/mine")).project, ProjectState::default());
    }

    #[test]
    fn open_projects_lose_blanks_duplicates_and_overlong_paths() {
        let long = "x".repeat(MAX_PATH_LEN + 1);
        let text = serde_json::json!({
            "version": 1,
            "openProjects": ["/a", "/a", "", long, "/b"],
            "lastProject": "/a"
        });

        let settings = settings_of(&text.to_string());

        assert_eq!(settings.open_projects, vec!["/a".to_string(), "/b".to_string()]);
    }

    #[test]
    fn the_active_project_has_to_be_in_the_list() {
        let settings = settings_of(r#"{"version":1,"openProjects":["/a","/b"],"lastProject":"/gone"}"#);
        assert_eq!(settings.last_project.as_deref(), Some("/a"), "an active project outside the list is replaced by the first one");

        let settings = settings_of(r#"{"version":1,"openProjects":["/a","/b"]}"#);
        assert_eq!(settings.last_project.as_deref(), Some("/a"), "a list with no active project — take the first");
    }

    #[test]
    fn an_empty_list_leaves_the_app_without_an_active_project() {
        // This is what a file written after the last project was closed looks
        // like: the list is empty and there is no active project. Nothing to resurrect.
        let settings = settings_of(r#"{"version":1,"openProjects":[],"lastProject":null}"#);
        assert_eq!(settings.last_project, None);
        assert!(settings.open_projects.is_empty());
    }

    #[test]
    fn a_file_from_before_the_list_keeps_the_project_it_remembered() {
        // A file written before this branch: lastProject is there, openProjects is not.
        let settings = settings_of(r#"{"version":1,"lastProject":"/work/smetana"}"#);
        assert_eq!(settings.open_projects, vec!["/work/smetana".to_string()], "otherwise the panel is empty");
        assert_eq!(settings.last_project.as_deref(), Some("/work/smetana"));

        // The same case, but the list was written empty — by content it is
        // indistinguishable, and the schema version (also 1) says nothing here.
        let settings = settings_of(r#"{"version":1,"openProjects":[],"lastProject":"/work/smetana"}"#);
        assert_eq!(settings.open_projects, vec!["/work/smetana".to_string()]);
        assert_eq!(settings.last_project.as_deref(), Some("/work/smetana"));
    }

    #[test]
    fn an_empty_list_from_the_front_end_is_not_resurrected() {
        // The leniency towards old files lives on the file-reading side only. A
        // front end that closed the last project sends an empty list — and it
        // has to stay that way, otherwise removing the last row would undo itself.
        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            open_projects: Vec::new(),
            active_project: Some("/work/smetana".into()),
            ..ResolvedSettings::default()
        };

        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());

        assert!(file.open_projects.is_empty());
        assert_eq!(file.last_project, None, "there is no active project outside the list");
        assert!(file.projects.is_empty(), "there is nobody to write state for");
    }

    #[test]
    fn merge_writes_the_list_and_the_active_project() {
        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            open_projects: vec!["/one".into(), "/two".into()],
            active_project: Some("/two".into()),
            project: ProjectState { selected_task: Some("bd-a1b2".into()), ..ProjectState::default() },
            ..ResolvedSettings::default()
        };

        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());

        assert_eq!(file.open_projects, vec!["/one".to_string(), "/two".to_string()]);
        assert_eq!(file.last_project.as_deref(), Some("/two"));
        assert_eq!(file.projects["/two"].selected_task.as_deref(), Some("bd-a1b2"));
        assert!(!file.projects.contains_key("/one"), "state is written only for the active project");
    }

    #[test]
    fn merge_without_an_active_project_writes_no_state() {
        let mut file = Settings::default();

        merge(&mut file, ResolvedSettings::default(), "2026-08-01T09:12:00+00:00".into());

        assert_eq!(file.last_project, None);
        assert!(file.projects.is_empty(), "the last project was closed — there is nobody to write state for");
    }

    #[test]
    fn trim_never_evicts_an_open_project() {
        let mut file = Settings::default();
        // The oldest entry of all — and an open one at that.
        file.projects.insert(
            "/open-and-ancient".into(),
            ProjectState { used_at: Some("2000-01-01T00:00:00+00:00".into()), ..ProjectState::default() },
        );
        for i in 0..MAX_PROJECTS + 5 {
            file.projects.insert(
                format!("/p{i:02}"),
                ProjectState {
                    used_at: Some(format!("2026-01-{:02}T00:00:00+00:00", i + 1)),
                    ..ProjectState::default()
                },
            );
        }

        let resolved = ResolvedSettings {
            open_projects: vec!["/open-and-ancient".into(), "/current".into()],
            active_project: Some("/current".into()),
            ..ResolvedSettings::default()
        };
        merge(&mut file, resolved, "2026-08-01T00:00:00+00:00".into());

        assert!(file.projects.contains_key("/open-and-ancient"), "an open project is never evicted, however old");
        assert!(file.projects.contains_key("/current"));
        assert!(!file.projects.contains_key("/p00"), "closed projects go oldest first");
        assert!(file.projects.len() <= MAX_PROJECTS.max(2));
    }

    #[test]
    fn resolve_carries_the_list_and_the_state_of_the_asked_project() {
        let file = settings_of(
            r#"{"version":1,"openProjects":["/a","/b"],"lastProject":"/a",
                "projects":{"/b":{"sideTab":"agents"}}}"#,
        );

        let view = resolve(&file, Some("/b"));
        assert_eq!(view.active_project.as_deref(), Some("/b"));
        assert_eq!(view.project.side_tab, "agents");
        assert_eq!(view.open_projects, vec!["/a".to_string(), "/b".to_string()]);

        let view = resolve(&file, None);
        assert_eq!(view.active_project.as_deref(), Some("/a"), "with no argument — the active project from the file");
        assert_eq!(view.project, ProjectState::default());
    }

    #[test]
    fn tabs_are_read_and_written() {
        let settings = settings_of(
            r#"{"version":1,"projects":{"/p":{
                "openTabs":["src/App.vue","README.md"],
                "previewTab":"README.md",
                "activeTab":"src/App.vue"}}}"#,
        );
        let state = &settings.projects["/p"];
        assert_eq!(state.open_tabs, vec!["src/App.vue".to_string(), "README.md".to_string()]);
        assert_eq!(state.preview_tab.as_deref(), Some("README.md"));
        assert_eq!(state.active_tab, "src/App.vue");
    }

    #[test]
    fn the_preview_tab_has_to_be_among_the_open_ones() {
        let settings = settings_of(
            r#"{"version":1,"projects":{"/p":{"openTabs":["a.txt"],"previewTab":"b.txt"}}}"#,
        );
        assert_eq!(settings.projects["/p"].preview_tab, None);
    }

    #[test]
    fn the_active_tab_is_terminal_kanban_or_one_of_the_open_files() {
        let settings = settings_of(
            r#"{"version":1,"projects":{
                "/gone":{"openTabs":["a.txt"],"activeTab":"b.txt"},
                "/chat":{"activeTab":"chat"},
                "/file":{"openTabs":["a.txt"],"activeTab":"a.txt"}}}"#,
        );
        assert_eq!(settings.projects["/gone"].active_tab, "kanban", "no such tab — nothing to be active");
        assert_eq!(settings.projects["/chat"].active_tab, "terminal", "the old tab name migrates");
        assert_eq!(settings.projects["/file"].active_tab, "a.txt");
    }

    #[test]
    fn a_chat_tab_from_an_old_file_becomes_terminal() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"activeTab":"chat"}}}"#);
        assert_eq!(settings.projects["/p"].active_tab, "terminal");
    }

    #[test]
    fn an_open_file_named_chat_does_not_migrate_to_terminal() {
        // A file with that name at the project root is an ordinary thing, and it
        // predates the tab's rename: the migration must not take a person off
        // their own file.
        let settings =
            settings_of(r#"{"version":1,"projects":{"/p":{"openTabs":["chat"],"activeTab":"chat"}}}"#);
        assert_eq!(settings.projects["/p"].active_tab, "chat");
    }

    #[test]
    fn the_terminal_tab_passes_validation() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"activeTab":"terminal"}}}"#);
        assert_eq!(settings.projects["/p"].active_tab, "terminal");
    }

    #[test]
    fn a_long_path_can_be_the_active_tab() {
        // active_tab used to be cut at MAX_ID_LEN (200); a path can be longer,
        // and the tab would silently become the board on every restart.
        let long = format!("src/{}/App.vue", "very-long-directory-name".repeat(12));
        assert!(long.len() > MAX_ID_LEN && long.len() < MAX_PATH_LEN);
        let text = serde_json::json!({
            "version": 1,
            "projects": {"/p": {"openTabs": [long.clone()], "activeTab": long.clone()}}
        });

        let settings = settings_of(&text.to_string());

        assert_eq!(settings.projects["/p"].active_tab, long);
    }

    #[test]
    fn the_tab_list_loses_junk_and_is_trimmed() {
        let mut tabs = vec![String::from("a.txt"), String::from("a.txt"), String::new()];
        for i in 0..MAX_OPEN_TABS {
            tabs.push(format!("f{i:03}.txt"));
        }
        let text = serde_json::json!({"version": 1, "projects": {"/p": {"openTabs": tabs}}});

        let settings = settings_of(&text.to_string());

        let open = &settings.projects["/p"].open_tabs;
        assert_eq!(open.len(), MAX_OPEN_TABS);
        assert_eq!(open[0], "a.txt");
        assert_eq!(open[1], "f000.txt", "the duplicate and the empty string fell out");
    }

    #[test]
    fn a_file_written_before_tabs_reads_without_them() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"sideTab":"agents"}}}"#);
        let state = &settings.projects["/p"];
        assert!(state.open_tabs.is_empty());
        assert_eq!(state.preview_tab, None);
        assert_eq!(state.active_tab, "kanban");
    }

    #[test]
    fn the_column_order_is_read_and_written() {
        let settings =
            settings_of(r#"{"version":1,"projects":{"/p":{"columnOrder":["done","ready"]}}}"#);
        assert_eq!(
            settings.projects["/p"].column_order,
            vec!["done".to_string(), "ready".to_string()]
        );

        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            project: ProjectState {
                column_order: vec!["running".into(), "ready".into()],
                ..ProjectState::default()
            },
            open_projects: vec!["/p".into()],
            active_project: Some("/p".into()),
            ..ResolvedSettings::default()
        };
        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());
        assert_eq!(
            file.projects["/p"].column_order,
            vec!["running".to_string(), "ready".to_string()]
        );
    }

    #[test]
    fn the_tab_order_is_read_and_written() {
        let settings =
            settings_of(r#"{"version":1,"projects":{"/p":{"tabOrder":["b.rs","a.rs"]}}}"#);
        assert_eq!(
            settings.projects["/p"].tab_order,
            vec!["b.rs".to_string(), "a.rs".to_string()]
        );

        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            project: ProjectState {
                tab_order: vec!["a.rs".into(), "b.rs".into()],
                ..ProjectState::default()
            },
            open_projects: vec!["/p".into()],
            active_project: Some("/p".into()),
            ..ResolvedSettings::default()
        };
        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());
        assert_eq!(
            file.projects["/p"].tab_order,
            vec!["a.rs".to_string(), "b.rs".to_string()]
        );
    }

    #[test]
    fn a_file_written_before_the_tab_order_reads_without_it() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"sideTab":"agents"}}}"#);
        assert!(
            settings.projects["/p"].tab_order.is_empty(),
            "nothing rearranged here, and the row draws its own order"
        );
    }

    /// The ceilings are not `column_order`'s and the difference is the whole
    /// point of the field: an entry is a tab id, and a file tab's id is a path,
    /// so a name far longer than a status name survives. The count is well past
    /// `MAX_OPEN_TABS` because the diffs and the shell tabs share this list.
    #[test]
    fn the_tab_order_measures_an_entry_as_a_path_and_not_as_an_identifier() {
        let long = "x".repeat(MAX_ID_LEN + 1);
        assert!(long.len() < MAX_PATH_LEN);
        let text = serde_json::json!({"version": 1, "projects": {"/p": {"tabOrder": [long]}}});

        let stored = &settings_of(&text.to_string()).projects["/p"].tab_order;

        assert_eq!(stored.len(), 1, "a long path is an ordinary tab id here");
        assert!(MAX_TAB_ORDER > MAX_OPEN_TABS, "diffs and shells are in this list too");
    }

    #[test]
    fn the_tab_order_loses_blanks_duplicates_and_everything_past_the_limit() {
        let mut order = vec![
            String::from("a.rs"),
            String::from("a.rs"),
            String::new(),
            "x".repeat(MAX_PATH_LEN + 1),
        ];
        for i in 0..MAX_TAB_ORDER {
            order.push(format!("f{i:04}.rs"));
        }
        let text = serde_json::json!({"version": 1, "projects": {"/p": {"tabOrder": order}}});

        let stored = &settings_of(&text.to_string()).projects["/p"].tab_order;

        assert_eq!(stored.len(), MAX_TAB_ORDER, "the order is not stored past the limit");
        assert_eq!(stored[0], "a.rs");
        assert_eq!(stored[1], "f0000.rs", "the duplicate, the empty id and the overlong one fell out");
        assert_eq!(
            stored.last(),
            Some(&format!("f{:04}.rs", MAX_TAB_ORDER - 2)),
            "the tail is trimmed, not the head"
        );
    }

    /// A diff tab and a shell tab die with the app, so on the next launch their
    /// ids name nothing. They are kept rather than pruned, exactly as a status
    /// bd no longer has is: the row passes them over, and the first drag
    /// rewrites the field from the tabs standing at that moment.
    #[test]
    fn the_tab_order_keeps_an_id_nothing_matches() {
        let settings =
            settings_of(r#"{"version":1,"projects":{"/p":{"tabOrder":["a.rs","\u0000term:4","b.rs"]}}}"#);
        assert_eq!(
            settings.projects["/p"].tab_order,
            vec!["a.rs".to_string(), "\u{0}term:4".to_string(), "b.rs".to_string()]
        );
    }

    #[test]
    fn the_announced_storage_threshold_is_read_and_written() {
        let settings =
            settings_of(r#"{"version":1,"projects":{"/p":{"storageWarnedMib":50}}}"#);
        assert_eq!(settings.projects["/p"].storage_warned_mib, Some(50));

        let mut file = Settings::default();
        let resolved = ResolvedSettings {
            project: ProjectState { storage_warned_mib: Some(10), ..ProjectState::default() },
            open_projects: vec!["/p".into()],
            active_project: Some("/p".into()),
            ..ResolvedSettings::default()
        };
        merge(&mut file, resolved, "2026-08-01T09:12:00+00:00".into());
        assert_eq!(file.projects["/p"].storage_warned_mib, Some(10));
    }

    #[test]
    fn a_file_written_before_the_storage_threshold_reads_without_it() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"sideTab":"agents"}}}"#);
        assert_eq!(
            settings.projects["/p"].storage_warned_mib, None,
            "nobody has been warned here yet"
        );
    }

    /// A number nobody could have been warned at loses itself, and the whole
    /// cost of that is one repeated warning. Rounding it down instead would keep
    /// a warning suppressed on the strength of a value the app never wrote.
    #[test]
    fn a_storage_threshold_off_the_ladder_is_forgotten() {
        for text in [
            r#"{"version":1,"projects":{"/p":{"storageWarnedMib":37}}}"#,
            r#"{"version":1,"projects":{"/p":{"storageWarnedMib":0}}}"#,
            r#"{"version":1,"projects":{"/p":{"storageWarnedMib":1000000}}}"#,
        ] {
            assert_eq!(settings_of(text).projects["/p"].storage_warned_mib, None, "{text}");
        }
    }

    /// The section is lenient about its neighbours and this field is no
    /// exception: a wrong type here costs the whole entry, the same as anywhere
    /// else, and the rest of the file survives.
    #[test]
    fn a_storage_threshold_of_the_wrong_type_costs_that_project_and_no_other() {
        let settings = settings_of(
            r#"{"version":1,"projects":{"/p":{"storageWarnedMib":"lots","sideTab":"agents"},"/q":{"sideTab":"agents"}}}"#,
        );
        assert!(!settings.projects.contains_key("/p"), "the damaged entry is dropped");
        assert_eq!(settings.projects["/q"].side_tab, "agents");
    }

    #[test]
    fn a_file_written_before_the_column_order_reads_without_it() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"sideTab":"agents"}}}"#);
        assert!(settings.projects["/p"].column_order.is_empty(), "no order is bd's own order");
    }

    #[test]
    fn a_file_written_before_the_recent_tasks_reads_without_them() {
        let settings = settings_of(r#"{"version":1,"projects":{"/p":{"sideTab":"agents"}}}"#);
        assert!(
            settings.projects["/p"].recent_tasks.is_empty(),
            "nothing opened here yet, and the palette draws no Recent section"
        );
    }

    #[test]
    fn the_recent_tasks_keep_the_newest_and_lose_the_duplicates() {
        let text = serde_json::json!({"version": 1, "projects": {"/p": {
            "recentTasks": ["bd-9", "bd-9", "", "bd-8", "bd-7", "bd-6"]
        }}});

        let stored = &settings_of(&text.to_string()).projects["/p"].recent_tasks;

        assert_eq!(
            stored,
            &["bd-9", "bd-8", "bd-7"],
            "newest first, the repeat and the empty entry gone, and the tail cut at the limit"
        );
    }

    #[test]
    fn the_column_order_loses_blanks_duplicates_and_everything_past_the_limit() {
        let mut order = vec![
            String::from("done"),
            String::from("done"),
            String::new(),
            "x".repeat(MAX_ID_LEN + 1),
        ];
        for i in 0..MAX_COLUMNS {
            order.push(format!("gen{i:03}"));
        }
        let text = serde_json::json!({"version": 1, "projects": {"/p": {"columnOrder": order}}});

        let stored = &settings_of(&text.to_string()).projects["/p"].column_order;

        assert_eq!(stored.len(), MAX_COLUMNS, "the order is not stored past the limit");
        assert_eq!(stored[0], "done");
        assert_eq!(stored[1], "gen000", "the duplicate, the empty name and the overlong one fell out");
        assert_eq!(
            stored.last(),
            Some(&format!("gen{:03}", MAX_COLUMNS - 2)),
            "the tail is trimmed, not the head"
        );
    }

    /// bd's set of statuses is unknown on this side of the IPC, so an unknown
    /// name is kept rather than pruned — the board passes it over, and a status
    /// that comes back finds the place it was left in.
    #[test]
    fn the_column_order_keeps_a_status_nothing_matches() {
        let settings =
            settings_of(r#"{"version":1,"projects":{"/p":{"columnOrder":["done","gone","ready"]}}}"#);
        assert_eq!(
            settings.projects["/p"].column_order,
            vec!["done".to_string(), "gone".to_string(), "ready".to_string()]
        );
    }

    #[test]
    fn a_fresh_file_runs_claude_code() {
        assert_eq!(Settings::default().agent, "claude");
        // The resolved view has its own hand-written Default, and the two have
        // to name the same agent: an empty string there is not one.
        assert_eq!(ResolvedSettings::default().agent, "claude");
    }

    #[test]
    fn an_agent_nobody_ships_loses_the_field_and_not_the_section() {
        let mut settings = Settings { agent: "cursor".into(), ..Settings::default() };
        settings.validate();
        assert_eq!(settings.agent, "claude");
        assert_eq!(settings.appearance.theme, "dark", "the section around it survives");
    }

    #[test]
    fn every_agent_the_app_knows_is_a_legal_setting() {
        for id in crate::agents::IDS {
            let mut settings = Settings { agent: id.to_string(), ..Settings::default() };
            settings.validate();
            assert_eq!(settings.agent, id);
        }
    }

    /// A field added to `Settings` and `ResolvedSettings` but not wired into
    /// `parse`, `resolve` and `merge` reads as `"claude"` forever no matter
    /// what the file says — the struct-alone tests above cannot see that,
    /// since they never call any of the three. This one walks the real path a
    /// hand-written file takes: disk, to the front end, and back to disk.
    #[test]
    fn a_chosen_agent_does_not_quietly_become_claude_again() {
        let file = settings_of(r#"{"version":1,"agent":"codex"}"#);
        assert_eq!(file.agent, "codex", "parse must read it off the disk");

        let resolved = resolve(&file, None);
        assert_eq!(resolved.agent, "codex", "resolve must carry it to the front end");

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-01T00:00:00+00:00".into());
        assert_eq!(written.agent, "codex", "merge must carry it back into the file");
    }

    #[test]
    fn a_file_with_no_languages_in_it_speaks_english() {
        // Every file on a person's disk right now is this file.
        let settings = settings_of(r#"{"version":1}"#);
        assert_eq!(settings.agent_language, "en");
        assert_eq!(settings.task_language, "en");
        assert_eq!(settings.commit_language, "en");
        assert_eq!(settings.report_language, "en");
        assert_eq!(Settings::default().agent_language, "en");
        assert_eq!(Settings::default().commit_language, "en");
        assert_eq!(Settings::default().report_language, "en");
        assert_eq!(ResolvedSettings::default().task_language, "en");
        assert_eq!(ResolvedSettings::default().commit_language, "en");
        assert_eq!(ResolvedSettings::default().report_language, "en");
    }

    #[test]
    fn every_language_the_app_ships_survives_a_load() {
        for (id, _) in crate::agents::LANGUAGES {
            let settings = settings_of(&format!(
                r#"{{"version":1,"agentLanguage":"{id}","taskLanguage":"{id}","commitLanguage":"{id}","reportLanguage":"{id}"}}"#
            ));
            assert_eq!(settings.agent_language, id);
            assert_eq!(settings.task_language, id);
            assert_eq!(settings.commit_language, id);
            assert_eq!(settings.report_language, id);
        }
    }

    #[test]
    fn a_language_nobody_ships_loses_the_field_and_not_the_section() {
        // The shape `an_agent_nobody_ships_loses_the_field_and_not_the_section`
        // already has, and for the same reason: a hand-edited value is no
        // reason to throw the rest of somebody's file away.
        let settings = settings_of(
            r#"{"version":1,"agentLanguage":"xx","taskLanguage":"ru","commitLanguage":"zz",
                "reportLanguage":"qq","agent":"codex","appearance":{"theme":"light"}}"#,
        );
        assert_eq!(settings.agent_language, "en", "the bad one falls back");
        assert_eq!(settings.commit_language, "en", "and so does its neighbour");
        assert_eq!(settings.report_language, "en", "and so does the report language");
        assert_eq!(settings.task_language, "ru", "the good one is untouched");
        assert_eq!(settings.agent, "codex", "and so is the rest of the file");
        assert_eq!(settings.appearance.theme, "light");
    }

    /// The same walk `a_chosen_agent_does_not_quietly_become_claude_again`
    /// makes, for the same reason: a field added to the two structs but not
    /// wired into `parse`, `resolve` and `merge` reads as `"en"` forever no
    /// matter what the file says.
    #[test]
    fn a_chosen_language_does_not_quietly_become_english_again() {
        let file = settings_of(
            r#"{"version":1,"agentLanguage":"ru","taskLanguage":"ja","commitLanguage":"de","reportLanguage":"it"}"#,
        );
        assert_eq!(file.agent_language, "ru", "parse must read it off the disk");
        assert_eq!(file.task_language, "ja");
        assert_eq!(file.commit_language, "de");
        assert_eq!(file.report_language, "it");

        let resolved = resolve(&file, None);
        assert_eq!(resolved.agent_language, "ru", "resolve must carry it to the front end");
        assert_eq!(resolved.task_language, "ja");
        assert_eq!(resolved.commit_language, "de");
        assert_eq!(resolved.report_language, "it");

        let mut written = Settings::default();
        merge(&mut written, resolved, "2026-08-01T00:00:00+00:00".into());
        assert_eq!(written.agent_language, "ru", "merge must carry it back into the file");
        assert_eq!(written.task_language, "ja");
        assert_eq!(written.commit_language, "de");
        assert_eq!(written.report_language, "it");
    }

    #[test]
    fn a_settings_file_written_before_the_run_dialog_existed_still_loads() {
        // Every file on a person's disk right now is this file.
        let state: ProjectState =
            serde_json::from_str(r#"{"sideTab":"files"}"#).expect("deserializes");
        assert_eq!(state.run_settings, None);
    }

    #[test]
    fn an_unknown_run_mode_loses_the_field_and_not_the_section() {
        // The same leniency every other single value gets: a bad value costs
        // that value, a bad type costs the section it is in.
        let mut state = ProjectState {
            run_settings: Some(RunDefaults { mode: "yolo".into(), ..RunDefaults::default() }),
            ..ProjectState::default()
        };
        state.validate();
        let run = state.run_settings.expect("the section survives");
        assert_eq!(run.mode, "auto");
        assert!(run.live_check, "nothing else in the section was touched");
    }

    #[test]
    fn a_priority_outside_bds_scale_is_forgotten() {
        // Clamping would guess: a 9 here is a wrongly edited file, and reading
        // it as "the lowest priority" makes a run take work nobody wanted taken.
        // No floor sends the dialog back to the project's own configured one.
        let mut state = ProjectState {
            run_settings: Some(RunDefaults { min_priority: Some(9), ..RunDefaults::default() }),
            ..ProjectState::default()
        };
        state.validate();
        assert_eq!(state.run_settings.expect("kept").min_priority, None);
    }

    #[test]
    fn a_run_with_no_floor_is_remembered_as_having_none() {
        // Every run aimed at a task or an epic writes this shape: a floor only
        // means something for the queue, so the dialog sends none. Read back as
        // a number it would open the next queue run on a choice nobody made,
        // over the top of `[defaults] min_priority` in the project's own config.
        //
        // The written shape is asserted as well as the read one, and the key
        // has to be genuinely absent rather than a null: a build older than
        // this one reads `u8` here, and a null it cannot parse costs that whole
        // project entry. See the field's own comment.
        let state = ProjectState {
            run_settings: Some(RunDefaults::default()),
            ..ProjectState::default()
        };
        let json = serde_json::to_string(&state).expect("serializes");
        assert!(!json.contains("minPriority"), "no floor is no key at all: {json}");
        let back: ProjectState = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.run_settings.expect("kept").min_priority, None);

        // And the same file read by this build once more, written by hand the
        // way every file on disk before this change looks.
        let old: ProjectState = serde_json::from_str(
            r#"{"runSettings":{"mode":"auto","liveCheck":true,"fileFindings":true}}"#,
        )
        .expect("deserializes");
        assert_eq!(old.run_settings.expect("kept").min_priority, None);
    }

    #[test]
    fn a_floor_inside_the_scale_survives_the_round_trip() {
        // The other half of the pair above: what the queue dialog did choose
        // has to come back, and come back as the same number.
        let mut state = ProjectState {
            run_settings: Some(RunDefaults { min_priority: Some(1), ..RunDefaults::default() }),
            ..ProjectState::default()
        };
        state.validate();
        let json = serde_json::to_string(&state).expect("serializes");
        let back: ProjectState = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.run_settings.expect("kept").min_priority, Some(1));
    }

    #[test]
    fn solo_is_a_mode_this_file_accepts_even_though_a_queue_would_refuse_it() {
        // Whether solo fits the scope is RunSettings::validate's answer. This
        // file only knows the three names, and dropping solo here would make
        // the dialog forget a legitimate choice on every restart.
        let mut state = ProjectState {
            run_settings: Some(RunDefaults { mode: "solo".into(), ..RunDefaults::default() }),
            ..ProjectState::default()
        };
        state.validate();
        assert_eq!(state.run_settings.expect("kept").mode, "solo");
    }
}
