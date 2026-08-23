<script setup>
/* Desktop app shell — three-column agent control room: scope bar, tab bar,
   kanban and task inspector.

   The right column carries no log any more: what it held was a fixture, and a
   pane of invented output under a real issue said the app knew something it did
   not. A session's actual output is the terminal tab, which is one click away.

   The core moment this screen is built for: you come back after two hours and
   read, in three seconds, what finished, what stalled, and what is waiting for
   you. Hence the loud budget — exactly one card and one callout shout here. */
import { computed, nextTick, onMounted, onUnmounted, ref, watch, watchEffect } from 'vue'
import ScopeIndicator from '../components/shell/ScopeIndicator.vue'
import Panel from '../components/shell/Panel.vue'
import Resizer from '../components/shell/Resizer.vue'
import TabBar from '../components/shell/TabBar.vue'
import { NEW_TAB_ITEMS } from '../components/shell/newTabMenu.js'
import { headline } from '../components/shell/headline.js'
import FileTree from '../components/files/FileTree.vue'
import ConflictModal from '../components/git/ConflictModal.vue'
import NewBranchModal from '../components/git/NewBranchModal.vue'
import GitPanel from '../components/git/GitPanel.vue'
import { gitActions } from '../components/git/gitActions.js'
import {
  NO_VISIT,
  answeredCount,
  changesVisible,
  enterGitTab,
  gitAnswered,
  toggleChanges
} from '../components/git/changesFold.js'
import KanbanBoard from '../components/kanban/KanbanBoard.vue'
import { orderColumns } from '../components/kanban/columnOrder.js'
import { mergeOrder, visibleColumns } from '../components/kanban/boardView.js'
import { isParked, needsReadyWarning, openQuestions, READY } from '../components/kanban/parked.js'
import { MENU_W, taskMenuItems } from '../components/kanban/taskMenu.js'
import Button from '../components/core/Button.vue'
import NewTaskModal from '../components/kanban/NewTaskModal.vue'
import PromoteColumnModal from '../components/kanban/PromoteColumnModal.vue'
import SetupProjectModal from '../components/run/SetupProjectModal.vue'
import RunBar from '../components/run/RunBar.vue'
import RunModal from '../components/run/RunModal.vue'
import ReportView from '../components/run/ReportView.vue'
import { isReportPath, reportTabPath } from '../components/run/reportTab.js'
import { deliveryFor } from '../components/run/reportDelivery.js'
import TaskInspector from '../components/kanban/TaskInspector.vue'
import DraftInspector from '../components/kanban/DraftInspector.vue'
import ClaimedTasks from '../components/agent/ClaimedTasks.vue'
import EmptyState from '../components/core/EmptyState.vue'
import Modal from '../components/overlays/Modal.vue'
import MenuButton from '../components/overlays/MenuButton.vue'
import Toast from '../components/overlays/Toast.vue'
import ProjectRail from '../components/shell/ProjectRail.vue'
import { projectSummary } from '../components/shell/projectState.js'
import Skeleton from '../components/core/Skeleton.vue'
import Icon from '../components/core/Icon.vue'
import Tooltip from '../components/core/Tooltip.vue'
import IconButton from '../components/core/IconButton.vue'
import { CommandPalette, TaskSearchButton, TerminalView } from '../components/index.js'
import AgentList from '../components/agent/AgentList.vue'
import {
  agentCounts,
  agentRows,
  createSession,
  createShell,
  initTerminals,
  lastHandover,
  lastRunStart,
  liveAgentCount,
  loadSessions,
  projectStates,
  removeSession,
  send,
  terminalState
} from '../stores/terminals.js'
import {
  boardColumns,
  clearSemantic,
  deleteIssue,
  dependencyEdges,
  initTracker,
  isLockIssue,
  issueById,
  searchSemantic,
  searchState,
  toUiStatus,
  trackerState,
  updateIssue
} from '../stores/tracker.js'
import NotificationPanel from '../components/notifications/NotificationPanel.vue'
import {
  dismiss as dismissNotification,
  markRunDelivered,
  measureStorage,
  notificationsState
} from '../stores/notifications.js'
import { initSettingsBridge, settings } from '../stores/settings.js'
import {
  announceBoardColumns,
  copyText,
  openExternal,
  openSettingsWindow,
  revealInFileManager,
  watchBoardHello
} from '../stores/app.js'
import { paintRoot } from './useAppearance.js'
import {
  activePath,
  addProject,
  adoptInitialProject,
  basename,
  initActive,
  projectRows,
  removeProject,
  switchTo
} from '../stores/projects.js'
import { gitState, loadBranches, loadHead } from '../stores/git.js'
/* The Git panel's own state, beside git.js rather than inside it: that store is
   the branch in the scope bar and spawns no process, this one runs git. */
import {
  abortConflict,
  autoFetch,
  fetchNow,
  checkout,
  commit,
  createBranch,
  dirtyCount,
  dismissConflict,
  draftMessage,
  loadRepos,
  merge,
  pull,
  push,
  rebase,
  refresh as refreshGit,
  selectRepo,
  setMessage,
  suggestMessage,
  vcsState
} from '../stores/vcs.js'
import {
  attachFiles,
  attachmentsState,
  clearAttachments,
  pickImages,
  removeAttachment,
  watchDrops
} from '../stores/attachments.js'
import {
  configError,
  initRuns,
  loadBrowserTools,
  loadConfig,
  loadRun,
  needsSetup,
  runsState,
  startRun,
  stopRun
} from '../stores/runs.js'
import { initUpdates } from '../stores/updates.js'
import { liveCheckBlock } from '../components/run/browserTools.js'
import { absolutePath, folderOf, parentOf, relativePath } from '../components/files/fileMenu.js'
import { checkNewName } from '../components/files/newEntry.js'
/* One import straight from `src/paths.js` rather than through a store: the
   conversion below is between two path spaces and belongs to neither of the
   stores that hold them. `tabs.js` reaches for the same function for the same
   join. */
import { relativeTo } from '../paths.js'
import { dropText } from '../components/terminal/dropPaths.js'
import { workingKey } from '../components/run/configFreshness.js'
import { scopeBusyReason } from '../components/run/runScopes.js'
import {
  LEFT_DEFAULT,
  PROJECT_RAIL,
  RAIL,
  RIGHT_DEFAULT,
  STEP,
  clampWidth,
  resolveDrag
} from './panelWidths.js'
import { RAIL_EXPAND, headerLabel, nextFromHeader, nextFromRail } from './leftChrome.js'
import {
  basenameOf,
  createDir,
  createFile,
  fileErrorText,
  filesState,
  isStubPath,
  listDir,
  makeErrorText,
  refreshDirs,
  saveErrorText,
  setRoot,
  statFiles,
  trashErrorText,
  trashPath,
  treeNodes
} from '../stores/files.js'
import {
  activeBuffer,
  buffers,
  closeDiff,
  closeTab,
  closeTerminalTab,
  confirmUnsaved,
  diffTab,
  diffTabs,
  discardTabs,
  dropAgentTab,
  hasAgentTab,
  isDiffTab,
  isDirty,
  isTerminalTab,
  keepMine,
  markGone,
  markStale,
  onUnsaved,
  openDiff,
  openFile,
  promote,
  reloadTab,
  restoreTabs,
  saveTab,
  saveTabs,
  setText,
  tabList,
  terminalTab,
  terminalTabFor
} from '../stores/tabs.js'
import FileEditor from '../components/files/FileEditor.vue'
import DiffView from '../components/files/editor/DiffView.vue'
import { keepOnly } from '../components/files/editor/states.js'

const props = defineProps({
  theme: { type: String, default: 'dark' },
  density: { type: String, default: 'comfortable' }
})

/* Both switches live on the document root: every token is defined against them.
   So does the type scale, which the settings window's app-wide font size
   rewrites there token by token — that way no component knows about it and the
   editor and the terminal come along for free (see `useAppearance.js`). The
   theme arrives already resolved: `system` is App.vue's to answer, since it is
   the machine's answer and not a stored one. */
watchEffect(() =>
  paintRoot(document.documentElement, {
    theme: props.theme,
    density: props.density,
    uiFontSize: settings.appearance.uiFontSize,
    editorFontSize: settings.editor.fontSize
  })
)

/* This window is the only writer of settings.json, and this is what makes the
   settings window's edits arrive here rather than going to the file behind our
   back — from here they are ordinary changes to the same reactive object every
   panel writes to, and the store's debounce takes them to disk. */
onMounted(initSettingsBridge)

/* The other half of the settings window's picture, and deliberately not part of
   that three-event contract: which columns this project's board has, for the
   Kanban tab's checkbox lists. The watcher above announces every change; this
   answers a window that opened after the last one, which is otherwise a tab
   with an empty list until the board next moves. Torn down on unmount so a
   view that is gone does not go on answering. */
let stopBoardHello = null
onMounted(async () => {
  try {
    stopBoardHello = await watchBoardHello(() => announceBoardColumns(projectColumns.value))
  } catch (err) {
    /* A browser, or an ACL — the app is fully usable without a settings window. */
    console.warn('[app] the board columns will not be announced:', err)
  }
})
onUnmounted(() => stopBoardHello?.())

/* Everything that survives a restart lives in settings: the panels in layout,
   the selection inside a project in project. Local refs are left only for what
   belongs to the current moment: the log, the modal, the title draft. */
const layout = settings.layout
const project = settings.project

/* ---- panel widths ------------------------------------------------------ */
/* `panelWidths.js` holds the rules and the reasons; what belongs here is the
   part that needs a window and a pointer. Two things:

   The viewport width has to be reactive, or the clamp would only ever be
   recomputed by a drag and a narrowed window would leave a panel sticking out
   over the board until someone touched a separator.

   And a drag remembers the width it began at. Every delta a Resizer emits is
   measured from its own start, so clamping against the previous frame instead
   would let the panel drift away from the pointer — each clamped move would
   quietly become the new origin. */
const viewport = ref(window.innerWidth)
const onViewport = () => {
  viewport.value = window.innerWidth
}
onMounted(() => window.addEventListener('resize', onViewport))
onUnmounted(() => window.removeEventListener('resize', onViewport))

const dragBase = { left: 0, right: 0 }

/* Whether the project rail is drawn beside the left panel: the preference, and
   the column not being folded, since a folded column draws no rail. It is part
   of every width sum below rather than only of the left one — the rail comes out
   of the same window, so the right panel's ceiling is measured against it too. */
const railOpen = computed(() => layout.railOpen && !layout.leftCollapsed)

/* The left column's two buttons walk a cycle of three states, and which state
   follows which is `leftChrome.js`'s — a `.vue` file is the one thing no test
   here can reach. All that is left on this side is writing the answer back into
   the two flags settings.json already keeps. */
const applyLeftChrome = (next) => {
  layout.railOpen = next.railOpen
  layout.leftCollapsed = next.leftCollapsed
}

/* The neighbour's *stored* width, not its drawn one: the drawn one is itself a
   clamp against this panel, and the two computeds would chase each other. In a
   window narrow enough for the difference to show, the stored number is the
   larger of the two, so the error is on the side of offering less room — which
   is the harmless direction. */
const geometry = (side) => ({
  side,
  other: side === 'left' ? layout.rightWidth : layout.leftWidth,
  otherCollapsed: side === 'left' ? layout.rightCollapsed : layout.leftCollapsed,
  viewport: viewport.value,
  railOpen: railOpen.value
})

const leftWidth = computed(() => clampWidth(layout.leftWidth, geometry('left')))
const rightWidth = computed(() => clampWidth(layout.rightWidth, geometry('right')))

const startDrag = (side) => {
  dragBase[side] = side === 'left' ? layout.leftWidth : layout.rightWidth
}

const onDrag = (side, delta) => {
  const next = resolveDrag(side, {
    ...geometry(side),
    base: dragBase[side],
    delta,
    collapsed: side === 'left' ? layout.leftCollapsed : layout.rightCollapsed
  })
  if (side === 'left') {
    layout.leftWidth = next.width
    layout.leftCollapsed = next.collapsed
  } else {
    layout.rightWidth = next.width
    layout.rightCollapsed = next.collapsed
  }
}

/* Double click on a separator is the way back to the shipped proportions —
   including out of the rail, since a panel folded by a drag is reopened by the
   same control that folded it. */
const resetWidth = (side) => {
  if (side === 'left') {
    layout.leftWidth = LEFT_DEFAULT
    layout.leftCollapsed = false
  } else {
    layout.rightWidth = RIGHT_DEFAULT
    layout.rightCollapsed = false
  }
}

/* FileTree expects a "path → expanded" map, while what lies on disk is a list
   of expanded directories: in a file people read with their eyes, a list is
   more honest than a map of nothing but true. The tree needs the Set to know
   where to descend. */
const expandedSet = computed(() => new Set(project.expanded))
const expanded = computed(() => Object.fromEntries(project.expanded.map((path) => [path, true])))
const tree = computed(() => treeNodes(expandedSet.value))

/* The sidebar holds three views of the same worktree, one at a time: its files,
   its git state, the agents working in it.

   This set is duplicated across the IPC boundary: the same three ids are the
   closed list in `src-tauri/src/settings/model.rs` (SIDE_TABS). Change one and
   you must change the other — a fourth tab added only here would work all
   session and come back as Files after a restart, with no error anywhere. */
const SIDE_TABS = [
  { id: 'files', label: 'Files' },
  { id: 'git', label: 'Git' },
  { id: 'agents', label: 'Agents' }
]
const hoveredSideTab = ref(null)
onMounted(initTracker)
onMounted(adoptInitialProject)
onMounted(initTerminals)
// The run's own event can fire before the webview is listening, which is what
// the loadRun calls beside every loadConfig are for.
onMounted(initRuns)
/* Nothing on this window draws the update state itself — About does, over in
   the settings window, which asks Rust on its own. What this subscription is
   for is the bell: an update that finished downloading has to reach somebody
   who never opens the settings window, and the card is the only thing that
   goes looking for them. Rust's first check waits a minute after launch, so
   there is no race to lose here. */
onMounted(initUpdates)

/* A new agent becomes the one you're looking at right away: that is what it was
   created for. Both switches sit before the await, the same as on the other two
   routes into a session — a spawn takes about a second, and waiting it out
   leaves the button a person pressed doing nothing visible for that second.
   What used to make the switch wait was a failed spawn jumping to an empty
   terminal; that is now the store's business rather than each caller's, and it
   is answered properly: the row and the selection appear at once and are both
   taken back if nothing starts, so a failure ends where it began instead of on
   a blank pane.

   The side tab is set here even though the button that calls this only exists
   while the panel is already on Agents: where a session is started from is not
   what should decide where the window ends up, and the three routes say the
   same two lines so that adding a fourth is one decision rather than two.

   The catch below swallows the rejection: createSession already logged it and
   set terminalState.lastError, which renders as a toast, so nothing is lost by
   not rethrowing — this catch exists only to stop Vue's own
   unhandled-rejection warning from repeating what the store already said. */
async function newAgent() {
  try {
    project.sideTab = 'agents'
    project.activeTab = 'terminal'
    await createSession(activePath.value, { kind: 'bare' })
  } catch {
    // already reported — see comment above
  }
}

/* A shell of the person's own, in the project's root, in a tab of its own.

   No side tab and no centre tab are set here, unlike `newAgent` above: the tab
   this opens is derived from the session, so it appears when the worker answers
   — about a second — and there is nothing to point at before then. What a
   ticket buys an agent is a panel that would otherwise draw an empty state over
   a row somebody just asked for, and a tab that is not there yet draws nothing
   at all.

   It is `activeTab` and not `sideTab` that moves, and only once the session is
   real: a shell has no row in the Agents panel, so there is nothing there to
   show anybody.

   `cwd` is a folder inside the project, as the file tree's menu names one: a
   path relative to the root, `null` for the root itself. The `+` menu's own row
   passes nothing, which is what it has always meant. Whether that folder may be
   a working directory is Rust's answer and not this function's — a refusal
   comes back as a null session and a toast, exactly like a shell that would not
   start. */
async function newTerminal(cwd = null) {
  const path = activePath.value
  if (!path) return
  const session = await createShell(path, cwd)
  /* Null is a refusal, already on screen as a toast — see `createShell`. */
  if (!session) return
  const tab = terminalTabFor(session.id)
  if (tab) project.activeTab = tab.id
}

/* Which row does what. The rows themselves are `newTabMenu.js`'s — two callers
   and no test can reach a template, the same split every other menu in this app
   keeps. The third row opens no tab: it is a second door onto the new-task
   dialog already mounted below, the same one the `+` above the `ready` column
   opens, so it sets that flag and nothing else. */
const onNewTab = (item) => {
  if (item.kind === 'agent') newAgent()
  else if (item.kind === 'terminal') newTerminal()
  else if (item.kind === 'task') newTaskOpen.value = true
}

/* The project whose setup is being offered. Null when the dialog is closed —
   it is asked about one project at a time, and the path is what the session
   needs. */
/* A project is runnable once it has a configuration that could be read. A
   damaged one deliberately does not count: the run would be against gates
   nobody can see, and `configError` is what says so on screen. */
const configured = computed(() => runsState.config.state === 'ok')
const runConfig = computed(() => (configured.value ? runsState.config.config : null))

/* Whether the run dialog can be reached at all, which is not the same question
   as whether a run can start. A damaged configuration is offered the dialog on
   purpose: it is the only surface wide enough to quote the parser and name the
   section that failed. Take the route away and the state is exactly what it was
   before this task — a board with no play buttons and nothing anywhere saying
   why. The repair itself is not here any more: "Set up again" lives in the
   project row's right-click menu, which is where the other two actions on a
   project already were.

   The dialog itself is what refuses: `configError` disables its Run. Letting
   the play through to a dialog that says no is the opposite of the rule about
   `runBlockedReason` below only in appearance — that rule is about a refusal
   arriving after the form is filled in and confirmed, and this one arrives
   before anything is answered, next to the way out. */
const configBroken = computed(() => runsState.config.state === 'broken')
const runOffered = computed(() => configured.value || configBroken.value)

/* The message the dialog quotes, and '' when there is nothing wrong — which is
   the same thing the dialog reads as "runnable". Derived from the state and not
   from the message, with a sentence of our own where the state says broken and
   the message is somehow empty: otherwise a blank complaint would leave the
   dialog live and its Run button enabled over a file the worker is about to
   refuse, which is the one way these two views of `config` could disagree. */
const configErrorText = computed(() =>
  configBroken.value ? configError.value || 'The file could not be parsed.' : ''
)

/* Which cards may be run — applied in `orderedColumns`, which is the one place
   a card is built for the board. Decided here rather than in the tracker store,
   because it is a product rule and it depends on something the store knows
   nothing about: whether this project has a configuration at all.

   A task under an epic is offered too, and that is a deliberate answer to a
   real question rather than an oversight: running one child alone can leave its
   epic half merged. The judgement is the person's — they are looking at the
   board and they know what the epic is — and taking the button away is not a
   way to have that conversation with them.

   Three exclusions. Done: the run would claim a closed issue, and there is
   nothing there to do. Blocked: the column is computed from unfinished
   blockers, so the run would put an agent on work whose prerequisite is not
   merged yet — and nothing here can go stale, because the blocker closing is
   what moves the card into Ready and brings the button with it. Parked: an
   agent already gave this task up over something it could not settle, and
   starting another one on it walks the next agent into the same question. That
   is the same refusal the Ready warning makes, and it has to be made here too —
   otherwise the play is the way around the dialog, one row above it in the very
   same menu. `bdStatus` rather than the normalized status, because parking is
   bd's own custom word and this is the front end's one reading of it. */
const runnableTask = (task) =>
  runOffered.value &&
  task.status !== 'done' &&
  task.status !== 'blocked' &&
  !isParked(task.bdStatus)

/* Why the queue's own play is inactive, or '' when it is not. A project holds
   several runs now, so a run going is no longer a reason to grey every play —
   only the play that would start the same run again: two runs both told to
   take the whole queue are two leads racing for the same tasks, and the worker
   refuses exactly that (runs/service.rs `admit`). This is the same refusal,
   said where the decision is made rather than after somebody has filled the
   dialog in and confirmed. It is the project's own runs and nothing else: a
   run in another project is no reason to grey anything here.

   Each card's play carries its own reason the same way, computed per card in
   `orderedColumns` below and riding the task object like `runnable` does —
   one string for the whole board stopped being honest the moment a queue run
   beside a task run became legal.

   Lowercase, because it is never shown on its own: both plays interpolate it
   into "Run this — …" / "Run the queue — …", and a capital there would read as
   two sentences joined by a dash. The rule and the words live in runScopes.js,
   which is the part of this a test can reach. */
const runBlockedReason = computed(() => scopeBusyReason({ kind: 'queue' }, runsState.runs))

/* Whether the Git panel may write to the repository it is showing — switch
   branch, merge, rebase — and why not when it may not. The rule is
   `components/git/gitActions.js` — the same family and the same reason as the
   line above: the runs are the whole of what it reads, so an agent session a
   person started themselves never reaches it. */
const gitWrites = computed(() => gitActions(runsState.runs))

/* Which branch the new-branch dialog was opened from, and null for closed. The
   branch is the state rather than a boolean beside it: what the dialog is about
   is entirely the row somebody right-clicked, and a flag with the name kept
   somewhere else is two things to clear instead of one.

   It is deliberately not held in the store. Nothing about a dialog somebody has
   half filled in survives a project switch, and the store is where the things
   that do survive live. */
const newBranchFrom = ref(null)

/* The dialog closes first and git runs after, which is the shape of every write
   in this panel: the spinner lands on the row the branch is cut from, and a
   refusal is drawn where the panel draws the rest of git's refusals. A dialog
   held open over that spinner would be a second place saying the same thing. */
const cutBranch = (ask) => {
  newBranchFrom.value = null
  createBranch(ask)
}

/* The Git panel's own folds and section heights. They live in `layout` rather
   than under the project — how tall somebody likes their branch list is a habit
   of reading, not a fact about one repository — and they reach the disk through
   the same 400 ms debounce every panel drag already uses, so there is no second
   save path here.

   The panel does the arithmetic, because the measurements are its: what lands
   here is a row count already resolved by `sectionHeights.js`, or `null` for a
   section given back to its content by a double click. */
const gitSections = computed(() => settings.layout.gitSections)

/* The two sections whose press is a plain inversion of what is stored. The
   changes are deliberately not in here any more — they are the one section a
   visit can be holding open, so the press is resolved against what is drawn
   rather than against the stored flag, in `toggleGitSection` below. */
const FOLD_KEY = { repos: 'reposOpen', branches: 'branchesOpen' }
const ROWS_KEY = { repos: 'reposRows', branches: 'branchRows' }

/* The one fold a visit to the tab is allowed to overrule, and the two fields
   that hold it — `changesFold.js` is the rule, this is the half of it that
   needs a tab and a store. Neither field reaches `settings.json`, and that is
   the point rather than an omission: the reason is in the module's own header.

   A visit starts on any move onto the Git tab and on the app starting or the
   project changing with Git already open, which is exactly what this pair of
   sources says with `immediate`. Both are watched rather than the tab alone,
   because moving to another project leaves `sideTab` reading `'git'` before and
   after, and the tree it is about is a different tree.

   **And that last case is why the count is not simply read**: on a switch the
   store still holds the departing project's tree, so a count taken here would
   be about somewhere else. Which counts may be believed is `answeredCount` in
   `changesFold.js`, with the window it closes and the measurement behind it —
   the rule is there and not here for the reason the module exists at all, since
   nothing in this file can be reached by a test. What is left in the view is
   the three field reads and the wiring, which genuinely cannot move. */
const changesVisit = ref(NO_VISIT)
watch(
  [activePath, () => project.sideTab],
  ([path, tab]) => {
    if (tab !== 'git') return
    const store = { project: vcsState.project, loading: vcsState.loading, count: dirtyCount.value }
    changesVisit.value = enterGitTab(answeredCount(store, path))
  },
  { immediate: true }
)
/* And the other half, for the ordinary case where the tab is on screen before
   git has answered.

   The source is the **tree** and not `dirtyCount`, because what settles a visit
   is git having answered at all — the moment the tree is *replaced*, which
   `loadStatus` always does rather than writing into the object in hand. Not
   "the moment it stops being `null`": on a project switch it goes from the
   departing project's object straight to the arriving one's and passes through
   `null` not at all, and that case is the whole reason this watcher is keyed on
   an identity. A count would not do it — it fires only when the number moves,
   so six changes in one project followed by six in the next would leave a visit
   armed for good. The count is still the only spelling of "how dirty": it is
   read here, not re-derived. */
watch(
  () => vcsState.tree,
  () => {
    changesVisit.value = gitAnswered(changesVisit.value, dirtyCount.value)
  }
)

/* What the panel is handed: the stored folds with the one the visit may have
   overruled resolved first, so `GitPanel` goes on reading a single
   `changesOpen` and neither its prop nor its events change shape. */
const resolvedGitSections = computed(() => ({
  ...gitSections.value,
  changesOpen: changesVisible(gitSections.value.changesOpen, changesVisit.value)
}))

const toggleGitSection = (section) => {
  /* The changes are the one section whose press is not an inversion of what is
     stored — under a forced-open fold that would write `true` and fold nothing.
     What a person folds is what they can see; the module says why. */
  if (section === 'changes') {
    const pressed = toggleChanges(gitSections.value.changesOpen, changesVisit.value)
    gitSections.value.changesOpen = pressed.changesOpen
    changesVisit.value = pressed.visit
    return
  }
  const key = FOLD_KEY[section]
  if (key) gitSections.value[key] = !gitSections.value[key]
}
const resizeGitSection = ({ section, rows }) => {
  const key = ROWS_KEY[section]
  if (key) gitSections.value[key] = rows
}

/* Which branch folders are unfolded, and this one *is* under the project: a
   `feature/…` prefix is a repository's convention where the heights above are a
   person's habit. What arrives is the whole new list, already resolved by
   `branchTree.js` — which is what writes the seeded folder out on the first
   press, so folding it away reaches an empty list rather than the `null` it
   started from. */
const toggleBranchFolders = (folders) => {
  project.branchFolders = folders
}

/* The second door out of a conflict, and the one this view has to carry: the
   store cannot open a tab or move a side tab, and everything else about the
   conflict is already in `vcsState`.

   The dialog is taken down first and the tree is left exactly as git left it —
   that is the whole of what this door does to the repository. Then it is the
   same three lines "Ask agent to edit" is: the agents panel, the terminal in
   the centre, and one session.

   The path handed to `createSession` is the **project**, as every other session
   here is: a session's directory is the project directory and there is no
   second one. Which repository inside it the work is in rides in the intent,
   named absolutely, and the prompt is what says so. */
const resolveConflictWithAgent = async () => {
  const conflict = vcsState.conflict
  const path = activePath.value
  if (!conflict || !path) return
  dismissConflict()
  project.sideTab = 'agents'
  project.activeTab = 'terminal'
  try {
    await createSession(path, {
      kind: 'resolveConflict',
      repo: conflict.repo,
      op: conflict.op,
      ours: conflict.ours ?? '',
      theirs: conflict.theirs,
      files: conflict.files
    })
  } catch {
    // already reported — see newAgent above
  }
}

const runOpen = ref(false)
const runScope = ref({ kind: 'queue' })
const runError = ref('')
const runStarting = ref(false)

/* Loaded when the dialog opens rather than on every project switch: it is a
   directory read, but nobody needs it until they are looking at the field.

   The dialog goes up first and the branches follow it, deliberately: a click on
   play must not sit there doing nothing for as long as an IPC round trip takes.
   What used to make that order wrong was the dialog filling its branch field
   exactly once, on opening, against a list that was still empty — it fills again
   when the list lands (RunModal, `fillBranch`), and the store no longer empties
   this project's list to go and read it (stores/git.js). */
const openRun = async (scopeValue) => {
  runScope.value = scopeValue
  runError.value = ''
  runOpen.value = true
  /* Both after the dialog is up, and both late for the same reason. What can
     drive a browser is four file reads and two directory listings plus a
     question to the run worker — cheap, and nobody needs it until they are
     looking at the toggle. Together rather than in sequence: neither answer
     depends on the other, and the toggle's own late-answer watcher is what makes
     the order stop mattering. */
  await Promise.all([loadBranches(activePath.value), loadBrowserTools(activePath.value)])
}

/* Why the live-check toggle cannot be switched on, or '' when it can. Only for
   `mode = "browser"`: a declared command needs no browser, and `none` is
   `liveCheckAvailable`'s business and has its own words. The rule is in
   browserTools.js, which is the part of it a test can reach. */
const liveCheckBlocked = computed(() =>
  liveCheckBlock(runConfig.value?.live_check?.mode, runsState.browserTools)
)

/* What is left to do under an issue. Parenthood in bd is the parent-child
   relation and nothing else: the parent's own type has no part in it, and
   neither does its status — this tracker's own `smetana-29j` is a closed
   `feature` with two open children under it. Reading the type instead was
   wrong, and it was wrong silently: the card said "part of smetana-29j" while
   the dialog said nothing at all.

   Done children are left out because a run would not take them, which is what
   makes this the number worth showing and worth deciding from. */
const childrenOf = (id) =>
  [...trackerState.issues.values()].filter(
    (issue) => issue.parent === id && toUiStatus(issue.status) !== 'done'
  )

/* The scope a card's play would start: an issue with children is run as its
   children — that is what the epic scope means, and the parent issue itself is
   never the work. One function rather than two copies, because the play's own
   grey (the per-card reason in `orderedColumns`) has to be about the very
   scope the click would send, or a card would grey over one run and start
   another. */
const cardScope = (id) => ({ kind: childrenOf(id).length ? 'epic' : 'task', id })

const runTask = (id) => {
  const issue = issueById(id)
  if (!issue) return
  openRun({ ...cardScope(id), title: issue.title })
}

/* The issue above the one in the dialog, if there is one. Read here rather
   than carried in the scope: the scope is what the run is aimed at, and this is
   context about it — it also has to stay right when `rescope` changes the aim.

   `siblings` is how many other unfinished children it has, and the advice hangs
   off it rather than off the parent's status: running the parent runs its
   children, so a closed parent is no reason to stay quiet, while being its only
   unfinished child means running it together is the same run by another name. */
const runParent = computed(() => {
  if (runScope.value.kind !== 'task') return null
  const parent = issueById(issueById(runScope.value.id)?.parent)
  if (!parent) return null
  return { id: parent.id, title: parent.title, siblings: childrenOf(parent.id).length - 1 }
})

/* Taking the advice, without closing what is already filled in. */
const runTheEpicInstead = () => {
  const epic = runParent.value
  if (epic) runScope.value = { kind: 'epic', id: epic.id, title: epic.title }
}

/* How much is in front of the run, for the line the dialog ends on. The whole
   ready column, not the drawn one: a run reads the board in Rust and the view
   settings do not reach it, so a number taken from what is on screen would
   describe something other than what the run is about to take. Only a period
   filter can make the two differ, and then the honest number is the run's. */
const runCount = computed(() => {
  if (runScope.value.kind === 'task') return 1
  if (runScope.value.kind === 'epic') return childrenOf(runScope.value.id).length
  return orderedColumns.value.find((c) => c.status === ADD_TO)?.tasks.length ?? 0
})

/* `path` and `chosen`, neither of them `project`, `settings` or `runSettings`:
   this function is the one place where the dialog's answer and the project's
   own state meet, and naming a local after something already in scope hid a
   defect here once. The local `project` used to be the active path — a string —
   so `project.runSettings = {...}` threw in strict mode, inside the try, and
   the catch put the exception's text under a dialog that stayed open over a run
   that had in fact started. The two objects are not interchangeable either:
   what the dialog hands over is snake_case and carries the scope, what the
   project remembers is camelCase and deliberately does not. */
const startTheRun = async (chosen) => {
  const path = activePath.value
  if (!path || runStarting.value) return
  runStarting.value = true
  runError.value = ''
  try {
    await startRun(path, chosen)
    /* Answered, so it goes — whether or not any of the rest below applies. */
    runOpen.value = false
    /* Moving to another project can start while this is still in its await, on
       a click in the project list, and this file checks after every await for
       exactly that (see the comment over onMounted). `project` is the *active*
       project's state by now, so the three writes below would put this run's
       branch and this run's tabs under a project it was never aimed at. The run
       itself is safely started and stays started; what is left here is only the
       screen, and the screen belongs to somebody else now. */
    if (activePath.value !== path) return
    /* Remembered for next time, minus the scope — that comes from whichever
       button was pressed.

       The floor is only in the payload when the run was the queue's, and what
       is remembered is the queue's floor: writing this run's absence over it
       would drop somebody's choice back to the config default every time they
       ran a single task from a card. */
    const floor = chosen.min_priority ?? project.runSettings?.minPriority
    project.runSettings = {
      mode: chosen.mode,
      targetBranch: chosen.target_branch,
      ...(floor == null ? {} : { minPriority: floor }),
      liveCheck: chosen.live_check,
      fileFindings: chosen.file_findings
    }
    /* A run is agent sessions, and watching them is the point — the same move
       filing a task and "Ask agent to edit" already make. The side panel only,
       and the centre deliberately not: `run_start` answers as soon as the worker
       has noted the request, which is before preflight and well before the first
       batch has a session, and this window mints no start ticket for a run — the
       run worker asks the terminal worker itself. So there is no agent yet and
       therefore no Agent tab, and a centre pointed at one would have named a tab
       that is not in the row, drawn no tab as active, and shown an empty
       terminal at the very moment somebody pressed Start run. The centre is
       moved by the `lastRunStart` watcher below, when the batch's session
       actually arrives, which is the one place that rule is written.

       What this line does promise is only where the agents will appear, not that
       there is anything to see yet: for the whole of preflight the panel draws
       AgentList's own empty state, which says no agent is running and offers the
       + row — the same untruth the centre tab was just taken off, one panel over
       and in much quieter type. It is left standing because it is pre-existing,
       because the panel is genuinely the destination, and because a side panel
       saying "nothing yet" is a smaller lie than a centre column drawing a pane
       behind a tab nobody can see. Do not read this line as evidence the empty
       state was thought through. The press's visible effect meanwhile is the run
       bar, which is where a run's own progress is reported. */
    project.sideTab = 'agents'
  } catch (err) {
    runError.value = runFailure(err)
  } finally {
    runStarting.value = false
  }
}

/* Rust's own words, which are already written for a person: the broken-config
   one names the section that will not parse, and that is the whole of why it is
   worth showing rather than replacing with something of our own. */
const runFailure = (err) => {
  const detail = err?.detail ?? err?.message ?? String(err)
  if (err?.kind === 'not_configured') return 'This project has no run configuration yet.'
  /* Named, because "a run is already going" stopped being an answer when a
     project could hold several: the person has to hear which of them is in the
     way. The fragment is the worker's own (RunScope::describe), riding in the
     error's detail. */
  if (err?.kind === 'already_running') {
    return `A run over ${err.detail?.scope ?? 'this scope'} is already going in this project.`
  }
  /* Deliberately not the sentence above. The run this one is about has stopped
     — the bar says so in the same breath — and only its loop is still winding
     down, so claiming a run is going would read as the stop not having taken. */
  if (err?.kind === 'winding_down') return 'The previous run is still finishing. Try again in a moment.'
  return detail || 'The run could not be started.'
}

/* By token, because the button lives on one bar segment and the stop has to
   reach exactly that run — its neighbours in the same project keep going. */
const stopTheRun = (token) => {
  stopRun(token)
}

const setupFor = ref(null)
/* Whether the project being asked about already has a file. Held beside the
   path rather than derived from `runsState` at render time: the dialog stays
   open across a config reload, and a value recomputed under it would change the
   words somebody is in the middle of reading. Every route sets both through
   `openSetup`, which is what stops the two drifting apart. */
const setupExisting = ref(false)
const settingUp = ref(false)

const openSetup = (path, existing) => {
  setupFor.value = path
  setupExisting.value = existing
}

const closeSetup = () => {
  setupFor.value = null
}

/* Adding a project is a read until this point: the dialog is where it becomes
   a session and a file in somebody's repository. */
const onAddProject = async () => {
  const added = await addProject()
  if (!added) return
  await loadConfig(added)
  if (needsSetup.value) openSetup(added, false)
}

/* The setup agent runs inside this window's own terminal tab, so the person
   watching it never leaves and never returns — window focus, which is how
   every other outside writer (an agent on a branch, a person in a terminal)
   gets noticed, simply never fires. terminalState.sessions already carries
   state for every session, active or not (see stores/terminals.js), so that is
   the signal to watch instead of a timer: every time a session of this project
   stops working, or one starts, the file may have changed and loadConfig reads
   it again. Both edges, deliberately — the key is what is working now, not
   what has just finished — so a session going idle, picking up again and then
   exiting costs two reads rather than one. That is the frequency to weigh
   before touching this channel, and it is a small toml parse against a
   `catchUp` that re-lists every expanded directory.

   The rule is `workingKey`, and it lives outside this file for the reason the
   whole `branchChoice.js` family does. What it replaces was a watcher created
   inside `startSetup`, over one session id, which tore itself down for good on
   its first callback for another project or for a session already gone from
   `terminalState.sessions` — and nothing anywhere re-established it, so a
   window that then never switched project and never lost focus went on drawing
   "Not set up for runs" over a configuration that existed, with the board's
   play buttons hidden behind the same `configured` (smetana-0ag).

   Declared at module-body scope like the rest of the watchers here, so Vue ties
   its lifetime to the component's and there is nothing left to stop by hand.
   The mark still clears on a read and never on the optimism that a session
   ended: this only asks the question again, and `needsSetup` moves when the
   answer comes back `ok`.

   A project switch moves the key too, and pays for up to two extra reads of a
   small file — the sessions of the project just left stop matching, then
   loadSessions brings the new project's in. The activePath watcher below reads
   the same file at the same moment; loadConfig is idempotent and guarded
   against its own stale response, so the duplicate costs the read and nothing
   else. */
watch(
  () => workingKey(terminalState.sessions, activePath.value),
  () => {
    if (activePath.value) loadConfig(activePath.value)
  }
)

const startSetup = async () => {
  const path = setupFor.value
  if (!path || settingUp.value) return
  settingUp.value = true
  try {
    project.sideTab = 'agents'
    project.activeTab = 'terminal'
    await createSession(path, { kind: 'setup' })
    closeSetup()
  } catch {
    // already reported by createSession; the dialog stays open
  } finally {
    settingUp.value = false
  }
}

/* Whose work the right column is showing under the question block: the id of
   the agent row a person opened, or null for the board's own selection. It
   cannot be folded into `project.selectedTask` — a draft is not an issue and
   has no id to put there, and a run has several issues rather than one.

   An id rather than a mode, and that is what keeps it from going stale. Every
   reader below requires it to still equal `terminalState.activeId`, so anything
   that moves the selection without a person clicking — `loadSessions` repairing
   onto the new project's last session after a switch, `removeSession` repairing
   onto whatever is left — drops the focus by arithmetic, with no watcher to
   forget and nothing cleared that nobody asked to clear. A mode would have
   survived those moves and hidden the new project's remembered card behind a
   draft belonging to the project just left. Session ids come from one
   monotonic counter in the worker, so an id cannot be reused by another
   project's session and accidentally match.

   Not in settings, unlike the selected task: it names a session, and sessions
   do not survive a restart. */
const rightFocus = ref(null)

/* The one move of the selection that the rule above must not treat as a loss:
   the row being watched was a start, the worker has answered for it, and it
   keeps its place in the panel under a session's id. Without this the draft
   vanished about a second after a task was filed — which is exactly the second
   somebody spends looking at the row they just created.

   This is a watcher, and it is deliberately not the watcher on `activeId` that
   this design refuses. That one cannot tell a handover from a repair, so it
   would drag the focus onto whatever a project switch or a removed session
   happened to land on. This one fires on nothing else: `lastHandover` is only
   ever written when a ticket becomes a session in this panel, which the store
   is the only party able to recognise. */
watch(lastHandover, (handover) => {
  if (handover && rightFocus.value === handover.ticket) rightFocus.value = handover.session
})

/* Picking an agent's row brings its session forward — the terminal centre tab,
   showing that agent — and opens the work behind it in the right column. The
   row names an agent, and what a person wants from an agent is to watch it: the
   list and the terminal are one gesture, the same way "+ New agent", filing a
   task and "Ask agent to edit" all switch the centre tab themselves. The cost
   is the open file or the board losing its place, which is recoverable by one
   click on the tab it was on.

   Each kind answers for itself, and three of them answer "nothing":

   - an edit, and answering a parked task's questions, open their issue on the
     board's own selection, which is what highlights the card: the panel and the
     board are one selection, so these are the kinds that write
     `project.selectedTask`;
   - a filing opens its draft. It does *not* clear the board's selection, and
     that restraint is the point: `selectedTask` is remembered per project in
     settings.json, so glancing at a filing agent would otherwise forget the
     card somebody had open, permanently, and leave them on a placeholder when
     they left the draft. Nothing is highlighted while the draft is up all the
     same — `highlightedTask` answers that, without writing anything down;
   - a run offers the issues it has taken, and picking one of those opens it;
   - a bare agent, a setup, and a run that has claimed nothing have no work to
     name, so the column and the board keep whatever they were showing. That is
     the whole behaviour, not a gap in it.

   No await — selection is local state, and TerminalView attaches to whatever
   activeId names once it is on screen. */
function selectAgent(id) {
  terminalState.activeId = id
  project.activeTab = 'terminal'
  const row = agentRows.value.find((candidate) => candidate.id === id)
  const work = row?.work
  if (work?.kind === 'editTask' || work?.kind === 'resolveTask') {
    project.selectedTask = work.id
    rightFocus.value = null
  } else if (work?.kind === 'newTask' || row?.claimed?.length) {
    rightFocus.value = id
  }
}

/* The one start this window does not make: a run asks the terminal worker
   itself, so nothing here calls `createSession` and there is no ticket to
   follow — the store recognises the arrival and moves the selection (see
   stores/terminals.js), and what is left is the half every other start does for
   itself, which is to bring the agent forward. Routed through `selectAgent` for
   exactly that reason: picking a row and a run handing over to its next batch
   should land a person in the same place, and two copies of "what follows a
   selection" would be two answers to that within a week.

   Every batch, not only the first: a run is a sequence of sessions and the one
   before has exited by the time the next starts, so staying put would leave
   somebody watching a dead terminal for the rest of the run. `startTheRun` sets
   the same two fields a second earlier for the same reason, and setting them
   again when the session actually lands costs nothing — before that there is no
   agent to select. */
watch(lastRunStart, (id) => {
  if (id == null) return
  project.sideTab = 'agents'
  selectAgent(id)
})

/* The Agent tab is derived from the sessions, so it goes on its own when the
   last agent does — but `project.activeTab` is remembered state and does not,
   and a person left on a tab that is no longer in the row would be looking at a
   centre column with nothing in it. The board is where they land, which is the
   fallback `closeTab` and `closeDiff` already use.

   A watcher here rather than in the store, and that is not a preference:
   `tabs.js` is one half of an import cycle with `settings.js`, so a module-scope
   `watch` in it would read `terminals.js` at evaluation time — the failure
   `notifications.js` carries a note about, which works in the dev server and
   leaves a white window in the built app. The rule itself is in the store
   (`dropAgentTab`), where a test can reach it; this is only the thing that
   notices. */
watch(hasAgentTab, (has) => {
  if (!has) dropAgentTab()
})

/* The shell a terminal tab draws, and `null` for anything else in the centre —
   including an id left over from a tab whose session is gone, which is why this
   hangs off the record rather than off `isTerminalTab`. The same shape
   `activeDiff` has, for the same reason. */
const activeTerminal = computed(() => terminalTab(project.activeTab))

/* The tree and the tabs open together with the project. By this point settings
   have already read the active project — App.vue awaits loadSettings before it
   renders this view at all.

   Moving to another project does exactly the same thing (moveTo in
   projects.js), and it can start while this pass is still in its awaits — with
   a click on a row of the list. There is then nothing left to finish: we check
   against the active project after every await and leave if it changed. The
   move wins, not whoever started earlier. */
onMounted(async () => {
  const opened = activePath.value
  if (!opened) return
  setRoot(opened)
  loadHead(opened)
  /* Not awaited, like its neighbours: the Git tab fills in when git answers,
     and nothing on this pass depends on it. */
  loadRepos(opened)
  loadConfig(opened)
  loadRun(opened)
  /* Not awaited, like the three above it: the bell fills in when the answer
     lands, and nothing on this pass depends on it. */
  measureStorage(opened)
  await loadSessions(opened)
  await listDir('')
  if (activePath.value !== opened) return
  await Promise.all(project.expanded.map((dir) => listDir(dir)))
  if (activePath.value !== opened) return
  await restoreTabs()
})
/* The app exists for the "came back two hours later and I'm looking at what
   changed" scenario — which means the moment focus returns is exactly the
   moment to catch up with the disk. Files deliberately have no watcher: a
   second watcher subsystem in Rust, with its own lifecycle and error reporting,
   costs more than this sweep.

   A clean tab is re-read silently: there is nothing to lose, and showing stale
   text for hours is worse. A dirty one gets a strip and waits for a decision.

   A tab with a read refusal goes through this sweep too rather than being
   skipped: there is no other way out of `error` — the field is locked, writing
   is refused, and the "Reload" button lives under a strip this tab does not
   have. A file that was deleted and put straight back (an ordinary thing next
   to an agent) would otherwise stay dead until a restart. */
const catchUp = async () => {
  if (!activePath.value) return
  refreshDirs(['', ...project.expanded])
  /* Anyone can change the project's branch — an agent in the next tab, a
     person in a terminal — and we learn about it in the same place as about
     files: when focus returns. We do not await it: the bar updates on its own,
     and the pass over the tabs is not tied to it. */
  loadHead(activePath.value)
  /* The working tree is written to by everybody but this window — an agent in
     the next tab, a person in a terminal, a run cutting a worktree — so the Git
     panel catches up exactly where the file tree does. Deliberately no watcher:
     a third watcher subsystem with its own lifecycle would fire on every write
     inside `node_modules` and `target`. The price is named rather than
     discovered: while an agent works, this list is as stale as the tree beside
     it, until focus returns or the refresh button is pressed. */
  loadRepos(activePath.value)
  /* And the one thing in that panel the disk cannot answer: whether anybody
     else has pushed. It goes out from here rather than from the store itself
     because a store that opened sockets on its own would be doing it on every
     repository row somebody clicks — the two moments this app catches up with
     the world are window focus and a project change, and this is the first of
     them.

     Not awaited, and nothing downstream of it: the panel is drawn from what is
     already known, the marks change when the answer lands, and a remote that
     never answers costs this sweep nothing. Whether it goes at all is the
     person's own setting, and the throttle is the store's — both are checked
     there, so this line is a question rather than an order. */
  autoFetch()
  /* The setup session writes .smetana/project.toml from outside this window,
     exactly like an agent changing a branch or a file — window focus is how
     this app learns about all of those. Not awaited, for the same reason
     loadHead above is not: the mark updates on its own. */
  loadConfig(activePath.value)
  /* The attachment store is written to from outside this window too — an agent
     filing a task, the Storage tab's own clean-up button, which lives in the
     other window and leaves this one holding a card about a folder that has
     just been emptied. Focus is when this app learns about all of those. */
  measureStorage(activePath.value)

  const open = [...project.openTabs]
  if (!open.length) return
  /* While the timestamps were travelling, the project may have been switched:
     the buffers belong to somebody else now and must not be touched. The same
     trick as in listDir. */
  const root = filesState.root
  const stats = await statFiles(open)
  if (filesState.root !== root) return

  for (const stat of stats) {
    const buffer = buffers.get(stat.path)
    if (!buffer || buffer.loading) continue
    if (stat.mtime === null) {
      /* The file is gone. Under a dirty tab that is not yet a verdict — what
         was typed is intact and the decision is the person's; under a clean one
         the tab would silently show the contents of something that does not
         exist. */
      if (isDirty(stat.path)) markStale(stat.path)
      else markGone(stat.path)
      continue
    }
    /* The file is there. A buffer with a read refusal is re-read in any case:
       its mtime stayed at zero and there is nothing to compare it with — but
       text typed before the refusal must not be overwritten without asking, so
       a dirty tab gets the strip rather than the disk. */
    if (!buffer.error && stat.mtime === buffer.mtime) continue
    if (isDirty(stat.path)) markStale(stat.path)
    else reloadTab(stat.path)
  }
}

onMounted(() => window.addEventListener('focus', catchUp))
onUnmounted(() => window.removeEventListener('focus', catchUp))

/* Window focus is not enough on its own for the one thing in this window that
   the disk cannot answer.

   Everything else `catchUp` refreshes is local — files, the branch, the
   repositories — and a window left open and untouched cannot have any of it
   change without somebody touching this machine. Whether a colleague pushed is
   the exception: it happens while nobody here does anything at all, and a
   window somebody leaves on the board for an afternoon would go the whole
   afternoon believing a branch is level because the last answer said so. That
   number is now what dims Pull, which makes it a fact somebody reads and
   decides on rather than a decoration.

   A minute is the tick and not the interval. What decides how often a socket
   actually opens is the store's own five-minute throttle per repository, and
   ticking under it is what makes this robust against the two ordinary ways a
   timer goes wrong: a laptop that slept through six of them fetches once on
   waking rather than six times, and a tick that lands a second before the
   throttle expires is followed by another a minute later instead of waiting
   out a whole second interval. The setting is checked in the store too, so
   with `git.autoFetch` off this line costs one early return a minute. */
const SWEEP_EVERY_MS = 60 * 1000
let sweep = null
onMounted(() => {
  sweep = setInterval(autoFetch, SWEEP_EVERY_MS)
})
onUnmounted(() => {
  if (sweep) clearInterval(sweep)
  sweep = null
})

const initing = ref(false)
const initHere = async () => {
  initing.value = true
  try {
    await initActive()
  } finally {
    initing.value = false
  }
}
/* bd gives a new task the one status it has for them — open, which the board
   calls ready. So that column, and only it, carries the "+": a plus over any
   other column would promise a placement the tracker cannot make. */
const ADD_TO = 'ready'
const newTaskOpen = ref(false)
/* The issue the New task dialog was opened from, or null when it was opened
   from "+ New task". `{ id, title }` rather than the issue, and taken from the
   store at the moment the menu was used: the dialog draws the title, and the id
   is what rides to the agent.

   A ref of its own rather than a field on some dialog-state object, because
   `newTaskOpen` is already a bare ref and two halves of one dialog kept in two
   shapes is the drift this file has elsewhere paid for. */
const followUpParent = ref(null)
const creating = ref(false)

/* Where the whole-column press stands. bd's own word, untranslated, because
   `deferred` is not one of the three statuses the tracker store renames and it
   reaches the board exactly as bd spells it.

   That column is where a run files its own findings, and the running-tasks
   skill reserves promoting one of them for a person. This button is the person
   doing it, in one gesture instead of twelve — which is why it moves issues and
   starts nothing: a run still takes only what is already open. */
const PROMOTE_FROM = 'deferred'
const promoteOpen = ref(false)
const promoting = ref(false)
/* The set as it was at the moment of the press, not a live reading of the
   column: the dialog names a count, and what confirming moves has to be the
   same set that count described. The watcher can add a card to that column
   while the dialog is open, and it is not part of what was agreed to. */
const promoteIds = ref([])
const promoted = ref(0)
const promoteFailed = ref(null)

const openPromote = () => {
  if (promoting.value) return
  /* The drawn column, not the whole one: the button a person pressed is
     captioned with the number beside it in the header, which counts the cards
     on screen. Reading the full column here would move a set larger than the
     one the label and the dialog both named. */
  const ids = drawnColumns.value.find((c) => c.status === PROMOTE_FROM)?.tasks.map((t) => t.id)
  if (!ids?.length) return
  promoteIds.value = ids
  promoted.value = 0
  promoteFailed.value = null
  promoteOpen.value = true
}

const closePromote = () => {
  if (promoting.value) return
  promoteOpen.value = false
}

/* One bd call per issue, in sequence — the worker serializes them anyway, and
   firing twenty at once would only bury the order they land in.

   Nothing is rolled back when one of them fails. The failures are counted and
   said out loud instead: the ones that landed are genuinely in ready, the board
   already shows them there, and moving them back would undo a change the person
   asked for over an error that had nothing to do with them.

   The active project is captured the way `startTheRun` captures it. Somebody
   can switch projects mid-way through a minute of writes, and the tracker
   worker points at the new folder from that moment on: every remaining write
   would be aimed at issues bd is no longer looking at. So the loop stops, and
   so does the dialog — it is about a board that is no longer on screen. */
const confirmPromote = async () => {
  if (promoting.value) return
  const path = activePath.value
  promoting.value = true
  promoted.value = 0
  promoteFailed.value = null
  let failed = 0
  try {
    for (const id of promoteIds.value) {
      try {
        await updateIssue(id, { status: 'open' })
        promoted.value += 1
      } catch {
        // the message already sits in trackerState.lastError; the count is what
        // this dialog adds to it
        failed += 1
      }
      if (activePath.value !== path) {
        promoteOpen.value = false
        return
      }
    }
    promoteFailed.value = failed
    if (!failed) promoteOpen.value = false
  } finally {
    promoting.value = false
  }
}

/* What the search may find: every issue in the project, less the merge lock —
   the same exclusion `boardColumns` makes, and made here rather than inside the
   rule because `isLockIssue` is the store's and the rule is deliberately free
   of both Vue and Tauri. Closed issues stay in, and so do the ones the board is
   not drawing today: reaching past the board is the point of searching.

   Cut down to the five fields the palette draws, and the status translated on
   the way through, for the same reason the lock is dropped here: `toUiStatus`
   is the store's vocabulary and the palette is a component, which sees this
   system's statuses and never bd's. */
const searchableIssues = computed(() =>
  [...trackerState.issues.values()]
    .filter((issue) => !isLockIssue(issue))
    .map((issue) => ({
      id: issue.id,
      title: issue.title,
      status: toUiStatus(issue.status),
      parent: issue.parent ?? null,
      updated_at: issue.updated_at
    }))
)

/* The bar's button, so the palette can hand the keyboard back to the thing it
   was opened from. */
const searchButton = ref(null)

const paletteOpen = ref(false)

/* Opening says so out loud, because the bell hangs from the same bar and its
   panel is excluded from its own "anything outside closes it" rule — see
   `onFindKey` below, which has recorded that since the field lived here. */
const openPalette = () => {
  closeNotifications()
  paletteOpen.value = true
}

/* Focus goes back where it came from, which is the one thing a modal owes the
   keyboard: closing with Esc must not drop the person at the top of the
   document. */
const closePalette = () => {
  paletteOpen.value = false
  nextTick(() => searchButton.value?.focus())
}

/* The last three tasks somebody looked at, newest first, without repeats.

   Written from the selection rather than from the palette, and that is what
   makes the word on screen mean what it reads as: five places assign
   `selectedTask` and two of them null it, so a list maintained at each call site
   would be five chances to forget one — and it would also make "recent" mean
   "found by searching" rather than "looked at". */
const RECENT_LIMIT = 3

watch(
  () => project.selectedTask,
  (id) => {
    if (!id) return
    const kept = project.recentTasks.filter((seen) => seen !== id)
    project.recentTasks = [id, ...kept].slice(0, RECENT_LIMIT)
  }
)

/* A click on a card is an explicit request to see that card, and it takes the
   right column back from whatever an agent's row put in it. Without this,
   picking a filing agent would leave the draft standing over every card clicked
   afterwards, with no way back to the board but another agent. */
const selectFromBoard = (id) => {
  project.selectedTask = id
  rightFocus.value = null
}

/* The row the panel is following. It has to be a lookup rather than a stored
   row: `agentRows` is rebuilt on every state event, and a row held from the
   moment it was clicked would keep drawing a session's first second forever. */
const selectedAgent = computed(
  () => agentRows.value.find((row) => row.id === terminalState.activeId) ?? null
)

/* Whether what `rightFocus` names is still the agent the person is looking at.
   Every reader of the focus goes through this: a repair that moved `activeId`
   — a project switch, a removed session — leaves the focus naming an agent that
   is no longer selected, and the column falls back to the board on its own. */
const focusIsLive = computed(
  () => rightFocus.value !== null && rightFocus.value === terminalState.activeId
)

/* The draft, when one is being drawn. Also guarded on the agent still *being* a
   filing one, so a focus that somehow outlived its agent draws nothing rather
   than throwing.

   The placeholder-to-session handover is invisible here, and it takes both
   halves to be: the words survive because the ticket and the session carry the
   same `work` (that is what putting the draft in `SessionWork` bought), and the
   focus survives because the watcher above moves it onto the session's id. The
   `work` half alone is not enough — the id comparison in `focusIsLive` would
   have dropped the row a moment before the identical draft arrived. */
const agentDraft = computed(() =>
  focusIsLive.value && selectedAgent.value?.work?.kind === 'newTask'
    ? selectedAgent.value.work
    : null
)

/* The run's own list, with each issue's title. Every id here is one the tracker
   holds — `claimedBy` in the store built the list out of `trackerState.issues`
   in the first place — so the lookup always finds its issue and a row for an
   issue nobody has heard of cannot happen. The `?? null` is for the other
   thing: an issue that arrived with no title at all, which bd should never
   send and which the list draws around rather than leaving a gap. */
const claimedTasks = computed(() => {
  if (!focusIsLive.value) return []
  return (selectedAgent.value?.claimed ?? []).map((id) => ({
    id,
    title: issueById(id)?.title ?? null
  }))
})

/* Which of the three the column is drawing, derived rather than read straight
   off `rightFocus` for the reason above: the thing focused can vanish, and the
   board is the answer that always exists. */
const rightPanel = computed(() => {
  if (agentDraft.value) return 'draft'
  if (claimedTasks.value.length) return 'claimed'
  return 'board'
})

/* The one answer to "which card is the person looking at", read by the board
   for its highlight and by the inspector for what to draw. Derived, never
   stored: `project.selectedTask` is remembered per project in settings.json, so
   a panel choice that wrote to it would turn a glance at an agent into an edit
   of a preference. Deriving it also stops the two halves drifting — a stored
   version had the run case highlighting a card on the board that the inspector
   then refused to draw.

   A draft has no card, so nothing is highlighted. Under a run's list the
   selection has to be one of the run's own: a card left selected from before
   would sit under "Taken by this agent" reading as part of that run's work. */
const highlightedTask = computed(() => {
  if (rightPanel.value === 'draft') return null
  if (rightPanel.value === 'claimed') {
    return claimedTasks.value.some((task) => task.id === project.selectedTask)
      ? project.selectedTask
      : null
  }
  return project.selectedTask
})

const inspectedIssue = computed(() =>
  highlightedTask.value ? issueById(highlightedTask.value) : null
)

/* Filing a task is an agent's job, not a write from this window: the point of
   the dialog is to hand the work over with enough context, and only something
   that has read the repository can turn four sentences into a task worth
   picking up. The card appears when the agent has run bd create — through the
   watcher, the same as any other change made outside this window. */
const submitNewTask = async ({ brainstorm, spec, plan, ...draft }) => {
  const path = activePath.value
  if (!path) return
  creating.value = true
  project.sideTab = 'agents'
  project.activeTab = 'terminal'
  try {
    /* The three stages ride beside the draft rather than in it: they are the
       agent's briefing about how to work, and nothing on screen draws them —
       the same place `brainstorm` has always had. */
    /* `parent` rides inside `draft` by the rest spread above, the way `images`
       does — only the three stages are named here, because only they are a
       briefing about how to work rather than part of the task. */
    const started = createSession(path, { kind: 'newTask', brainstorm, spec, plan, draft })
    /* Filing a task opens its draft on the right, the same as picking the row
       would: it is the same selection arriving by another route, and the action
       giving two different answers depending on what the column happened to be
       showing was the inconsistency this closes.

       Read before the await, not after, and that is the whole reason this is
       not one line further down. `createSession` picks its start's row
       synchronously — "the row is there and picked before the worker has
       answered", pinned by that name in tests/stores/terminals.test.js — so
       `activeId` is already the ticket, and the draft goes up the moment the
       dialog closes. Waiting for the session would leave the column on the
       board for the second the agent takes to come up, showing somebody the
       board they had just stopped looking at. The handover watcher carries the
       focus onto the session's own id when it lands. */
    rightFocus.value = terminalState.activeId
    await started
    closeNewTask()
  } catch {
    // already reported by the store; the dialog stays open with the text and
    // the thumbnails still in it
  } finally {
    creating.value = false
  }
}

/* Closing is the one event that clears the attachments, and it covers both
   cases that should: cancelling, and a session that actually started. A failed
   create does not reach here, so nobody has to attach four screenshots again
   because the agent was not installed.

   The paths outlive this: the files stay in the app's data directory whether
   the task was filed or not. Forgetting them here is all that happens, which is
   the same bargain the store's own note describes. */
const closeNewTask = () => {
  newTaskOpen.value = false
  clearAttachments()
  /* The same event that clears the attachments clears this, and for the same
     reason: it covers cancelling and a session that actually started, and a
     failed create never reaches here — so the next "+ New task" is never
     silently a follow-up to whatever a menu was last opened on. */
  followUpParent.value = null
}

/* A drop is a window event, not the dialog's — Tauri intercepts file drops
   before the webview sees them — so the subscription lives up here and asks
   whether anything is collecting. */
let stopDrops = null
onMounted(() => {
  stopDrops = watchDrops(() => newTaskOpen.value)
})
onUnmounted(() => stopDrops?.())

/* While the app was closed, the issue may have been closed and removed from
   the tracker. Restoring a selection that no longer exists is not on: the
   inspector would show emptiness while the file kept holding rubbish. We wait
   for the tracker to be ready — before that, "not found" means nothing.

   Readiness alone is not enough: ready only means "a snapshot arrived", and a
   snapshot arrives empty too — when bd was not found, when the folder has no
   .beads, when the first sync failed. In those cases "not found" speaks about
   the tracker rather than the issue, and the selection must not be wiped: the
   debounce would carry a null to disk at once, and one launch with a broken bd
   — from Finder, say — would lose the remembered issue forever. So we ask about
   health too.

   A merge lock counts as gone: the board does not draw it, so a settings file
   written before it was hidden would otherwise leave the inspector showing an
   issue with no card behind it. */
watch(
  () => [trackerState.ready, trackerState.health.state, trackerState.issues.size],
  () => {
    const selected = project.selectedTask ? issueById(project.selectedTask) : null
    if (
      trackerState.ready &&
      trackerState.health.state === 'ok' &&
      project.selectedTask &&
      (!selected || isLockIssue(selected))
    ) {
      project.selectedTask = null
    }
  },
  { immediate: true }
)

/* The status write, tracked by the id it was asked for rather than by a bare
   boolean — the reason `deletingId` below already is one. There are two
   triggers for the same menu now and they disagree about which issue they are
   over: the card's acts on whichever card it was opened from, the Task &
   details header's on the selected one. A bd call takes about two seconds, and
   a flag shared between issues would grey the wrong one of the two for those
   two seconds — which is what makes the id load-bearing rather than tidy. */
const writingId = ref(null)
const setTaskStatus = async (id, status) => {
  writingId.value = id
  try {
    await updateIssue(id, { status })
  } catch {
    // the message already sits in trackerState.lastError
  } finally {
    writingId.value = null
  }
}

/* Deletion is irreversible, so the id it is tracked by is the id it was asked
   for, not a bare boolean: the selection can move while bd is still working,
   and a flag shared between issues would grey out the wrong one's dialog. */
const deletingId = ref(null)

/* Which issue's deletion is being confirmed, or null. An id rather than a
   boolean for the same reason: the dialog names the issue, and the board can
   change under it. The dialog itself lives here rather than in the panel: at
   view level there is no `overflow` box to be clipped by, so it needs no
   `Teleport` either. */
const confirmingDelete = ref(null)
const confirmedIssue = computed(() =>
  confirmingDelete.value ? issueById(confirmingDelete.value) : null
)

const deleteTask = async (id) => {
  deletingId.value = id
  try {
    await deleteIssue(id)
    if (project.selectedTask === id) project.selectedTask = null
    confirmingDelete.value = null
  } catch {
    /* The message already sits in trackerState.lastError — and the dialog stays
       open over it deliberately: closing it would hide the explanation. */
  } finally {
    deletingId.value = null
  }
}

/* A task menu asked for something; this is where it is carried out, for both
   of the menu's triggers — the card's own and the Task & details header's,
   which send the same payload. The issue is resolved from the store rather than
   carried in the payload — the store holds the current title and a card's copy
   may be a delta behind. */
const onTaskAction = ({ kind, id, value }) => {
  if (kind === 'run') return runTask(id)
  if (kind === 'status') {
    /* The one status write that is asked about first. The status comes from the
       store rather than from the menu that sent this: the card's copy may be a
       delta behind, and of the two ways to be wrong here, asking about a task
       somebody has already unparked costs a dialog while writing over a fresh
       parking costs the question. `issueById` hands back bd's own issue, where
       that field is plain `status` — `bdStatus` is the name it takes on a card,
       one layer up in `boardColumns`. */
    const issue = issueById(id)
    if (needsReadyWarning(issue?.status, value)) {
      confirmingReady.value = id
      return
    }
    return setTaskStatus(id, value)
  }
  if (kind === 'delete') {
    confirmingDelete.value = id
    return
  }
  if (kind === 'resolve') {
    const issue = issueById(id)
    if (issue) askAgentToResolve(issue)
    return
  }
  if (kind === 'ask-agent') {
    const issue = issueById(id)
    if (issue) askAgentToEdit(issue)
    return
  }
  if (kind === 'follow-up') {
    /* From the store and not from the menu's payload, for the reason the status
       branch above spells out: a card's copy may be a delta behind, and the
       dialog is about to put this title in front of somebody.

       Nothing else is prefilled from the parent. The person is filing a
       different task, and the agent reads the parent itself. */
    const issue = issueById(id)
    if (!issue) return
    followUpParent.value = { id: issue.id, title: issue.title }
    newTaskOpen.value = true
  }
}

/* Which issue's move to Ready is being asked about, or null. An id rather than
   the issue, for the reason `confirmingDelete` above already carries: the
   dialog names what the board holds now, not what it held when the menu was
   opened. */
const confirmingReady = ref(null)
const readyIssue = computed(() =>
  confirmingReady.value ? issueById(confirmingReady.value) : null
)
/* What is still unanswered, drawn in the dialog. A parked task with no note is
   an ordinary outcome — somebody can park one by hand — and the dialog says so
   in prose rather than drawing an empty list. */
const readyQuestions = computed(() => openQuestions(readyIssue.value?.notes))

const moveToReadyAnyway = () => {
  const id = confirmingReady.value
  confirmingReady.value = null
  if (id) setTaskStatus(id, READY)
}

const resolveFromDialog = () => {
  const issue = readyIssue.value
  confirmingReady.value = null
  if (issue) askAgentToResolve(issue)
}

const askAgentToEdit = async (issue) => {
  const path = activePath.value
  if (!path) return
  project.sideTab = 'agents'
  project.activeTab = 'terminal'
  try {
    await createSession(path, { kind: 'editTask', id: issue.id, title: issue.title })
  } catch {
    // already reported — see newAgent above
  }
}

/* A session that asks the person what the run could not settle, writes the
   answers into the issue and unparks it. Started exactly the way editing is,
   and the questions are deliberately not carried in the payload: they are in
   the issue's own notes, the agent reads the issue anyway, and a copy sent from
   here would be the board as it was when a menu opened. */
const askAgentToResolve = async (issue) => {
  const path = activePath.value
  if (!path) return
  project.sideTab = 'agents'
  project.activeTab = 'terminal'
  try {
    await createSession(path, { kind: 'resolveTask', id: issue.id, title: issue.title })
  } catch {
    // already reported — see newAgent above
  }
}

/* What the tracker's health means where the board would be. The generic
   "No board yet — connect a tracker" is wrong for a folder without .beads:
   there is nothing to connect to and creating a task there fails. Each state
   says what it is and what to do about it, and all of them stay quiet — this
   is information, not an emergency, and the loud budget belongs to the card
   that is waiting on you. The diagnostic text from Rust goes to the console,
   not here. */
const HEALTH_NOTICE = {
  'no-project': {
    icon: 'folder-git-2',
    title: 'No project open',
    description: 'Add a folder that bd tracks — or one you want it to.'
  },
  'not-a-beads-repo': {
    icon: 'folder-git-2',
    title: 'No tracker here',
    description:
      'No .beads directory in this folder or any folder above it. Initialize bd to start tracking tasks in it.'
  },
  'bd-version-mismatch': {
    icon: 'info',
    title: 'Unexpected bd version',
    description:
      'The bundled bd is not the version this build was checked against. Tasks may be read or written incorrectly.'
  },
  error: {
    icon: 'triangle-alert',
    title: 'bd is failing',
    description:
      'The tracker command keeps returning errors — see the console for what it said. The board recovers on its own once it succeeds.'
  }
}

/* bd owns which columns exist; the settings own only their sequence, and the
   two meet in orderColumns. The stored order is per project, because the set of
   statuses is: a custom status of one repository has no place in another one's
   order. Writing `project.columnOrder` is the whole of saving it — the settings
   store debounces it to disk and loadProjectLayout brings it back, on a restart
   and on a switch alike. */
const orderedColumns = computed(() =>
  orderColumns(boardColumns.value, project.columnOrder).map((column) => ({
    ...column,
    /* `runnable` rides in the task object, the way every other thing a card is
       drawn from does — the column v-binds the whole of it, and a second
       channel for one flag would put the decision in two places. The per-card
       `runBlockedReason` rides beside it for the same reason, and it is per
       card because the refusal is per scope now: the card's own play is greyed
       only over a live run on this very task or epic, never over the queue's
       or a neighbour's. */
    tasks: column.tasks.map((task) => {
      const runnable = runnableTask(task)
      return {
        ...task,
        runnable,
        /* A write in flight on this very issue, which greys its whole menu —
           per id, since the menu belongs to the card rather than to the
           selection. */
        busy: writingId.value === task.id || deletingId.value === task.id,
        runBlockedReason: runnable ? scopeBusyReason(cardScope(task.id), runsState.runs) : ''
      }
    })
  }))
)

/* The card behind whatever the Task & details panel is drawing, or null. The
   panel's own menu button is built from it, which is the whole of "the same
   menu the card has": the very values the card is drawn from, worked out once
   in `orderedColumns` above and read here rather than worked out again.

   Recomputing them from `issueById` was considered and refused, because it
   would have drifted from the board in silence. Blocked is worked out from
   unfinished blockers while bd keeps such an issue at `open`
   (`stores/tracker.js`), so `runnableTask` on the issue would say a run is
   available where the card on the board says it is not, and the panel would
   offer a run the board refuses.

   Every issue is in here bar the merge lock, and a lock issue can be selected
   neither from the board nor from a run's claimed list — so `null` here means
   there is nothing to act on. */
const inspectedCard = computed(() => {
  const id = highlightedTask.value
  if (!id) return null
  for (const column of orderedColumns.value) {
    const task = column.tasks.find((t) => t.id === id)
    if (task) return task
  }
  return null
})

const inspectedMenu = computed(() =>
  inspectedCard.value
    ? taskMenuItems({
        bdStatus: inspectedCard.value.bdStatus,
        runnable: inspectedCard.value.runnable,
        runBlockedReason: inspectedCard.value.runBlockedReason,
        busy: inspectedCard.value.busy
      })
    : []
)

/* What is actually drawn: the whole ordered board put through the two view
   settings (`components/kanban/boardView.js`). After `orderColumns` and never
   before — the sequence is a property of the whole board and must not depend on
   which columns happen to be on screen today.

   `Date.now()` is read here rather than kept in a ticking ref: this recomputes
   on every tracker delta and on every change to the settings, which is enough.
   A card is not obliged to vanish on the minute, and a timer would repaint the
   board for nobody. */
const drawnColumns = computed(() =>
  visibleColumns(orderedColumns.value, settings.kanban, Date.now())
)

/* Which columns this project's board has, for the settings window's Kanban tab
   — the full set, never the drawn one: a column hidden by a setting is exactly
   the column somebody goes there to pin, and a list that dropped it would take
   the way back with it. `blocked` is in it, which is why this comes from here
   and not from Rust — see `stores/app.js`.

   Watched over the joined names rather than the array: `orderedColumns` is
   rebuilt on every delta, so an identity watch would announce on every card
   that moved. */
const projectColumns = computed(() => orderedColumns.value.map((column) => column.status))
watch(
  () => projectColumns.value.join('\n'),
  () => announceBoardColumns(projectColumns.value),
  { immediate: true }
)

/* Only when there is nothing else to show: a failing bd is no reason to hide
   the tasks that were already read. */
const healthNotice = computed(() => {
  if (trackerState.health.state === 'ok') return null
  if (boardColumns.value.some((column) => column.tasks.length)) return null
  return HEALTH_NOTICE[trackerState.health.state] ?? HEALTH_NOTICE.error
})

/* A run's document, drawn as the page it is rather than as its source. It is an
   ordinary path in `openTabs` — no storage of its own, and it closes and comes
   back after a restart like every other tab — so where it sits is the whole of
   what makes it one, and that rule is `reportTab.js`. */
const reportTabActive = computed(() => isReportPath(project.activeTab))

/* A changed file open as a diff. The record is the store's — the two texts and
   whatever refused to be read — and `null` is both "not a diff tab" and "a diff
   tab that is no longer there", which is why the branch below hangs off it
   rather than off `isDiffTab`: an id left over from a restart draws the board,
   not two empty columns. */
const activeDiff = computed(() => diffTab(project.activeTab))

/* A file tab is anything that isn't terminal or kanban. There is no closed
   list in the centre and there won't be: the project brings the tabs.

   Minus the reports, which are a third kind of tab: the two computeds are
   never both true, and a report opened in CodeMirror would show a person the
   markup of a document written for them to read. This is also what keeps Cmd+S
   off it below — there is nothing to save on a tab nobody can type into. And
   minus the diffs, which are a fourth: they name no file in `openTabs`, so a
   `FileEditor` on one would ask the disk for a path built out of a tab id. */
const fileTabActive = computed(
  () =>
    project.activeTab !== 'terminal' &&
    project.activeTab !== 'kanban' &&
    !reportTabActive.value &&
    !isDiffTab(project.activeTab) &&
    /* And minus the terminals, which are a fifth kind, and for the same reason
       the diffs are excluded: a shell's tab names no file in `openTabs`, so a
       `FileEditor` on one would ask the disk for a path built out of a tab id.
       This computed is what stands between that and the editor, since the
       editor's branch comes first in the template. */
    !isTerminalTab(project.activeTab)
)

/* The text a diff refuses with. `fileErrorText` is the editor's own table and
   the kinds are the same on both sides of the wire — `VcsError::kind` carries
   `FilesError`'s strings deliberately — so a binary file says the same thing
   whichever way it was opened. */
const diffNotice = computed(() =>
  activeDiff.value?.error ? fileErrorText(activeDiff.value.error) : ''
)

/* Cmd+S is caught by the window, not by the editor field. A click on a tab, on
   a tree row or on any button takes focus out of the field, and a handler on
   the field itself would silently stop working — precisely when a person
   reaches for Cmd+S. The view knows whether there is anything to save, not the
   field: there is something to save only on a file tab.

   The key is checked through `event.code` — that is the physical key, and it
   depends neither on the keyboard layout nor on Caps Lock. `event.key` is a
   non-Latin character under a non-Latin layout and 'S' under Caps Lock, and a
   comparison with 's' would miss in both cases: there simply would be no
   save. */
const onSaveKey = (event) => {
  if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) return
  if (event.code !== 'KeyS') return
  /* We cancel in any case: the webview's "save page" is out of place here,
     both on the board and in the chat. */
  event.preventDefault()
  if (fileTabActive.value) saveTab(project.activeTab)
}

/* Cmd+F / Ctrl+F opens the command palette. `event.code` and not `event.key`,
   the same discipline `onSaveKey` above records: `event.key` is a non-Latin
   character under a non-Latin layout and 'F' under Caps Lock, and the shortcut
   would simply not fire in either case.

   Cancelled in any case, like the save: the webview's own find bar is the
   platform's rather than this product's, and it would search the rendered board
   instead of the project.

   **Except inside the editor, where this key is already spoken for.**
   `@codemirror/search`'s `searchKeymap` binds Mod-f to `openSearchPanel` and
   cancels the default without stopping propagation, so the event arrives here
   as well: unguarded, ⌘F in a file opened the editor's find panel and then took
   the keyboard out of it a tick later, which left find-in-file unreachable from
   the keyboard altogether — in an editor whose panel `files/editor/extensions.js`
   installs and `theme.js` themes on purpose. The spec's "from anywhere in the
   window" was written about the board, not about taking a key from the one
   thing on screen that already had it. Checked before `preventDefault`, so the
   editor's own binding is left entirely alone.

   Closing the bell is the same "anything else closes it" rule the panel already
   follows, applied to the one thing inside the scope bar that now opens a
   surface of its own: both hang from the same corner at the same width and the
   same z, and the panel is later in the template, so a surface opened under an
   open bell would be drawn underneath it with nothing on screen saying why.

   It keeps this key even though ⌘K below is the palette's own: cancelling is
   mandatory here whatever happens next, since the webview's find bar would
   search the rendered board, and a key that is intercepted, cancelled and then
   does nothing is worse than a second door to the same room. */
const onFindKey = (event) => {
  if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) return
  if (event.code !== 'KeyF') return
  if (event.target?.closest?.('.cm-editor')) return
  event.preventDefault()
  openPalette()
}

/* Cmd+K / Ctrl+K opens the same palette, and is the key the panel itself
   advertises on the bar's button. `event.code` again, for the reason the two
   above give.

   **Except where this key is already spoken for, which is two places and not
   one.** `Ctrl+K` in a shell is kill-to-end-of-line, and taking it would cost
   somebody a readline binding they use every day in exchange for a second door
   to a palette ⌘F already opens. The editor is the same case and was missed at
   first: CodeMirror's `defaultKeymap` folds in `emacsStyleKeymap` on macOS, so
   `Ctrl-k` is `deleteToLineEnd` inside `.cm-editor` — and it cancels the default
   without stopping propagation, exactly as its Mod-f binding does, so this
   listener ran anyway and killed to end of line *and* opened a modal over the
   file. Both checks come before `preventDefault`, so both keys are left entirely
   alone; the classes are the ones xterm.js and CodeMirror put on the elements
   they render into, which are the only marks those hosts carry. */
const onPaletteKey = (event) => {
  if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) return
  if (event.code !== 'KeyK') return
  if (event.target?.closest?.('.xterm')) return
  if (event.target?.closest?.('.cm-editor')) return
  event.preventDefault()
  openPalette()
}

onMounted(() => window.addEventListener('keydown', onSaveKey))
onUnmounted(() => window.removeEventListener('keydown', onSaveKey))
onMounted(() => window.addEventListener('keydown', onFindKey))
onUnmounted(() => window.removeEventListener('keydown', onFindKey))
onMounted(() => window.addEventListener('keydown', onPaletteKey))
onUnmounted(() => window.removeEventListener('keydown', onPaletteKey))

/* The order of the branches runs from "the file does not exist as text" to
   "the file is there but something happened to it": `error` locks the field and
   explains the emptiness, `stale` asks for a decision and is therefore the only
   one carrying buttons, `saveError` merely reports. The tone of a write refusal
   is as quiet as the others': the edits are intact, the field stayed editable,
   and the next Cmd+S is an ordinary attempt rather than a recovery. */
const editorNotice = computed(() => {
  const buffer = activeBuffer.value
  if (!buffer) return null
  if (buffer.error) return { tone: 'blocked', text: fileErrorText(buffer.error) }
  if (buffer.stale) {
    return { tone: 'stale', text: 'This file changed on disk since it was opened.' }
  }
  if (buffer.saveError) return { tone: 'blocked', text: saveErrorText(buffer.saveError) }
  return null
})

/* Expanding reads a directory if it has not been read yet. Collapsing does not
   forget what was read: expanding it back has to be instant, and the focus
   sweep brings the freshness. */
const toggleDir = (path) => {
  const at = project.expanded.indexOf(path)
  if (at === -1) {
    project.expanded.push(path)
    if (!filesState.dirs.has(path)) listDir(path)
  } else {
    project.expanded.splice(at, 1)
  }
}

/* The "…N more" row is not a file: there is no path on disk behind it, and a
   tab opened from it would reach settings and stay there forever. We filter it
   out here, in both handlers: the tree knows nothing about the stub and should
   not. */
const onSelectFile = (path) => {
  if (isStubPath(path)) return
  project.selectedPath = path
  openFile(path)
}
const onOpenFile = (path) => {
  if (isStubPath(path)) return
  project.selectedPath = path
  openFile(path, { permanent: true })
}

/* The file tree's context menu: which verb does what. The rows themselves are
   `components/files/fileMenu.js`'s, and the pair is joined by hand — a `kind`
   renamed on one side draws perfectly and does nothing at all when pressed, the
   same seam `newTabMenu.js` and `onNewTab` above have. The tree emits rather
   than acting because the stores live here: a component that imported one would
   be the second exception to a rule with exactly one.

   Nothing in here moves the selection or opens a preview. A secondary click is
   a question about a row, not a visit to it, and the row under the panel says
   so with a highlight of its own. */
const fileMenuToast = ref(null)
let fileMenuToastTimer = null

/* Success goes away on its own and a refusal does not, which is the split the
   rest of the app already keeps: every other toast in this corner is an error
   held in a store until somebody dismisses it. A copy has nothing on screen to
   show for itself — an empty clipboard and one holding the path look exactly
   alike until somebody pastes — so it says so, briefly, and then stops
   occupying the corner. */
const SAID_TOAST_MS = 3000

function sayFileMenu(toast) {
  clearTimeout(fileMenuToastTimer)
  fileMenuToast.value = toast
  if (toast?.tone === 'success') {
    fileMenuToastTimer = setTimeout(() => {
      fileMenuToast.value = null
    }, SAID_TOAST_MS)
  }
}

onUnmounted(() => clearTimeout(fileMenuToastTimer))

/* Which agent a path lands in: the selected one, and only ever the selected
   one, when it is an agent that can be typed into.

   The whole safety of this gesture is that the text is *typed* rather than sent,
   so the person sees it land and writes around it. That only holds if it lands
   where they are looking, and the centre draws `terminalState.activeId` — so
   this is that id or nothing. There used to be a fall-back to the newest live
   row, and it broke exactly that: a finished agent stays in the list and stays
   selectable, and `createSession` parks a *string* ticket in `activeId` for the
   second a spawn takes, so in both cases the path went into a session the tab
   was not showing and sat there in somebody else's half-written prompt, with
   nothing on screen to say it had.

   Narrowing rather than moving the selection is the other half of the choice.
   `selectAgent` sets `activeId` and the tab together and is welcome to; this
   menu is built on not moving anything — a secondary click is a question about
   a row, not a visit to it — and an item that quietly repointed the agents panel
   would be the same surprise one panel over.

   Exited rows are excluded because there is nothing behind them to write to, and
   start tickets because a ticket is not a session and has no id the worker would
   accept. */
const liveAgentRows = computed(() =>
  agentRows.value.filter(
    (row) => !row.starting && row.state !== 'done' && row.state !== 'failed'
  )
)

const attachTarget = computed(
  () => liveAgentRows.value.find((row) => row.id === terminalState.activeId)?.id ?? null
)

/* Whether there is an agent here to pick at all — the same population, before
   the selection narrows it. It decides nothing about whether the item is off:
   it decides which reason the off row gives, because "no agent to type into" is
   plainly false with one running one column over, and that is the ordinary
   state rather than a corner. Nothing moves `activeId` when a session ends —
   `finish` leaves it alone and the repair in `loadSessions` treats an exited row
   as a live selection — so an agent finishing while another runs leaves the
   selection on the finished one until somebody moves it. */
const hasLiveAgent = computed(() => liveAgentRows.value.length > 0)

/* A file handed to an agent is the drag-and-drop gesture by another route, so
   it is the same bytes through the same function: `dropText` quotes the path,
   ends it in one space and refuses outright a name carrying a control character
   — see `terminal/dropPaths.js` for why that last one is a refusal rather than
   a repair. A second way to write a path into a prompt would be a second quoting
   rule to keep correct. Return stays with the person either way. */
async function attachToAgent(path) {
  const id = attachTarget.value
  const text = dropText([path])
  if (!id || !text) {
    sayFileMenu({
      tone: 'error',
      title: 'Nothing was attached',
      /* The second branch is a race and nothing else: the row was drawn live,
         so there was an agent selected when the menu opened, and it stopped
         being one before the pick. The label's own two sentences are about the
         menu; this is about the moment after it. */
      description: !text
        ? 'That name carries a character that would press Return in an agent.'
        : 'The selected agent went away before the path could reach it.'
    })
    return
  }
  /* The tab and nothing else: `id` is already `terminalState.activeId`, which is
     what this tab draws, so the path lands in the session that comes up. */
  project.activeTab = 'terminal'
  await send(id, text)
}

async function copyPath(text, what) {
  const ok = await copyText(text)
  sayFileMenu(
    ok
      ? { tone: 'success', title: `Copied the ${what}`, description: text }
      : {
          tone: 'error',
          title: `Could not copy the ${what}`,
          description: 'The clipboard refused it. The path is in the console.'
        }
  )
}

/* Making the entry the draft row was typed into, and the whole of what happens
   on screen after it exists.

   The rule about the name is `newEntry.js`'s and is applied here rather than in
   the tree, because two of its three answers are a toast's business: an empty
   field is somebody who changed their mind and is answered with silence, and a
   name no entry can carry is answered with the sentence the back end would have
   used for it — one wording for the refusal, wherever it is decided. Neither
   goes near Rust.

   A file opens as a permanent tab and not a preview: it was asked for by name a
   moment ago, and a tab the next click would replace is not what somebody who
   just made a file wants. A folder is expanded instead, which is the same
   answer in the tree's own terms — the thing you made, open, with its contents
   (none yet) under it. */
async function makeEntry(kind, dir, typed) {
  const { verdict, name } = checkNewName(typed)
  if (verdict === 'nothing') return
  if (verdict === 'refused') {
    sayFileMenu({
      tone: 'error',
      title: 'Nothing was created',
      description: makeErrorText({ kind: 'badName' })
    })
    return
  }
  try {
    const path = kind === 'dir' ? await createDir(dir, name) : await createFile(dir, name)
    /* The tree has no watcher — freshness is the focus sweep — so the folder
       that just gained an entry is re-read here, by the one who knows it
       changed. */
    await listDir(dir)
    if (kind === 'dir') {
      if (!project.expanded.includes(path)) project.expanded.push(path)
      await listDir(path)
    } else {
      project.selectedPath = path
      openFile(path, { permanent: true })
    }
  } catch (error) {
    sayFileMenu({
      tone: 'error',
      title: 'Nothing was created',
      description: makeErrorText(error)
    })
  }
}

/* Into the system trash, and then the tidying no one else will do.

   Every tab over what is gone is closed rather than left: `stale` in the editor
   is about a file that changed, not one that stopped existing, and a buffer
   over nothing has nowhere to save itself — a dirty tab kept open would be an
   offer this app cannot honour. A folder takes everything under it, which is
   what a trash means. The list is filtered by `kind` and never by the shape of
   an id: the Agent tab and the shell tabs come from `terminals.js`, they are
   not paths, and a `startsWith` over the whole row would be closing sessions.

   A diff tab is one of the tabs over that path and closes too, but it has to be
   found through `diffTabs`, and in the tree's own space rather than its record's
   two fields. Its id is a repository and a path with a zero byte between them,
   so no test against the id could ever match; and `repo` is a repository's
   **absolute** path with `path` relative to *it*, which is only the same string
   as the tree's in a project that is one repository. `vcs/repos.rs` finds the
   root and one level down, so in a project of several, matching the bare
   `tab.path` would leave `sub`'s diff of `src/a.js` open when `sub/src/a.js`
   went, and close it when the root's own `src/a.js` went — a tab about a file
   that is still there. `relativeTo` over the joined pair is the conversion
   `loadDiff` already makes for the same reason, and it answers `null` for a
   repository outside this project, which `under` then never matches.

   The expanded list and the selection are settings, and both would otherwise
   keep naming a folder that is not there — harmless on screen and permanent in
   `settings.json`. */
async function deleteEntry(path) {
  try {
    await trashPath(path)
  } catch (error) {
    sayFileMenu({
      tone: 'error',
      title: 'Nothing was deleted',
      description: trashErrorText(error)
    })
    return
  }
  const under = (other) => other === path || other.startsWith(`${path}/`)
  const closing = tabList.value
    .filter((tab) => (tab.kind === 'file' || tab.kind === 'preview') && under(tab.id))
    .map((tab) => tab.id)
  /* The ids are taken before anything closes: `tabList` is computed off the
     very list `closeTab` splices, and `diffTabs` is the list `closeDiff`
     splices. */
  const closingDiffs = diffTabs
    .filter((tab) => {
      const rel = relativeTo(filesState.root, `${tab.repo}/${tab.path}`)
      return rel !== null && under(rel)
    })
    .map((tab) => tab.id)
  for (const id of closing) closeTab(id)
  for (const id of closingDiffs) closeDiff(id)
  for (let i = project.expanded.length - 1; i >= 0; i -= 1) {
    if (under(project.expanded[i])) project.expanded.splice(i, 1)
  }
  if (project.selectedPath && under(project.selectedPath)) project.selectedPath = null
  await listDir(parentOf(path))
}

const onFileAction = async ({ kind, path, target, name }) => {
  const root = filesState.root
  if (kind === 'create-file' || kind === 'create-dir') {
    /* `path` is the folder the draft row sat in — the tree worked that out when
       it opened the field, since that is where the row was drawn. */
    await makeEntry(kind === 'create-dir' ? 'dir' : 'file', path, name)
  } else if (kind === 'delete') {
    await deleteEntry(path)
  } else if (kind === 'open-terminal') {
    await newTerminal(folderOf({ path, target }))
  } else if (kind === 'reveal') {
    const ok = await revealInFileManager(absolutePath(root, path))
    if (!ok) {
      sayFileMenu({
        tone: 'error',
        title: 'Could not show it',
        description: 'This one needs the desktop app — a browser has no file manager to ask.'
      })
    }
  } else if (kind === 'copy-path') {
    await copyPath(absolutePath(root, path), 'path')
  } else if (kind === 'copy-relative-path') {
    await copyPath(relativePath(path), 'relative path')
  } else if (kind === 'attach') {
    await attachToAgent(absolutePath(root, path))
  }
  /* `new-file` and `new-folder` never arrive here: they put a field in the tree
     and come back later as `create-file` or `create-dir` with a name. */
}

/* One question for all unsaved tabs. It comes up in three places and the
   answer is the same in all three, hence a single modal. */
const unsaved = ref(null)

onMounted(() =>
  onUnsaved(
    (paths) =>
      new Promise((resolve) => {
        /* A second question has no right to orphan the first: whoever is
           awaiting an answer gets a "no" and winds their work down cleanly,
           clearing their own flags. Otherwise `moving` in projects.js would stay
           raised forever. */
        if (unsaved.value) unsaved.value.resolve(false)
        unsaved.value = { paths, resolve }
      })
  )
)

/* The answer has to resolve the promise whatever the outcome: the modal is
   already off the screen, while whoever is waiting keeps their flag raised —
   `closing` in settings, `moving` in projects. An unresolved promise here means
   a window that cannot be closed and a project list that cannot be switched. */
const answerUnsaved = async (answer) => {
  const pending = unsaved.value
  unsaved.value = null
  if (!pending) return
  try {
    if (answer === 'cancel') return pending.resolve(false)
    if (answer === 'save') {
      await saveTabs(pending.paths)
      /* The write may have failed — the tabs are then still dirty, and letting
         things through is not on: closing would destroy the text we were asked
         to save. The strip with the reason for the refusal is already on
         screen; the person will decide. */
      if (pending.paths.some(isDirty)) return pending.resolve(false)
    } else {
      discardTabs(pending.paths)
    }
    pending.resolve(true)
  } catch (err) {
    /* A repeat resolve is harmless: a promise resolves once. */
    console.error('[desktop] the unsaved-work answer did not work out:', err)
    pending.resolve(false)
  }
}

const onCloseTab = async (id) => {
  /* A diff holds nothing of anybody's, so there is nothing to ask about and
     nothing in `openTabs` to take out. */
  if (isDiffTab(id)) return closeDiff(id)
  /* A terminal is the other way round: there is nothing unsaved in it either,
     but closing it kills the shell behind it — the tab is only a view of a
     session, and one that merely hid a live shell would leave a process nobody
     can see. The store owns both halves of that. */
  if (isTerminalTab(id)) return closeTerminalTab(id)
  if (isDirty(id) && !(await confirmUnsaved([id]))) return
  closeTab(id)
}

/* editor/states.js keys state by path, and a tab's path is relative (tabs.js
   keeps it that way so a project move does not turn the list into rubbish). The
   same README.md, package.json or CLAUDE.md can be open in two different
   projects at once, and their relative path is identical — without the root
   that would be one key for two different files: one repository's edit history
   would quietly flow into another's on a plain Cmd+Z. absoluteEditorPath joins
   the root to the relative path, and that makes different projects' keys
   distinct without an explicit purge on a move — the old root simply stops
   appearing in the live set below.

   The root comes from filesState.root, not from activePath
   (stores/projects.js): during a move activeProject changes at the very start
   of moveTo, before the first await, while filesState.root changes only after
   it, synchronously together with project.activeTab and tabList
   (resetTabs/setRoot come right after applySection). Had we taken activePath,
   an extra reactivity pass would happen between those two moments with the new
   root and the still-old activeTab — the composite path would point nowhere for
   an instant. This way both change in one go, and the watcher below and the
   watcher inside FileEditor see them only together. */
const absoluteEditorPath = (relPath) => (filesState.root ? `${filesState.root}/${relPath}` : relPath)

/* Editor state lives exactly as long as the tab. Cleanup follows the tab
   list, not the close button: the same watcher covers switching projects and
   a path that fell out because the file stopped being readable. Everything that
   is not a file is filtered out — the pinned tabs (Kanban, Agent), the diffs and
   the terminals: none has a file on disk, and there is no reason to build a
   composite path for one — nothing is ever saved under its id.

   flush: 'post' is required, not cosmetic. closeTab trims openTabs; this
   watcher at the default (flush: 'pre') would fire before FileEditor gets a
   chance to react to the new props.path in its onBeforeUnmount or its own
   watcher — keepOnly would clear the path before FileEditor saves its state
   through putState, and the very next putState would immediately resurrect
   the entry just cleared. post waits for the whole patch — including
   FileEditor's reaction — to finish, so cleanup sees the already-saved
   state. */
watch(
  tabList,
  (tabs) =>
    keepOnly(
      tabs
        .filter((tab) => tab.kind === 'file' || tab.kind === 'preview')
        .map((tab) => absoluteEditorPath(tab.id))
    ),
  { flush: 'post' }
)

/* Sessions belong to the project. Someone else's keep running in the
   background and are not killed — a project switch that killed another
   project's work would be the same class of loss that `stale` guards
   against in the files layer. */
watch(activePath, (path) => {
  loadSessions(path)
  loadHead(path)
  loadConfig(path)
  loadRun(path)
})

/* The other half of the background fetch, and it is keyed on the repository the
   Git panel settled on rather than on the project path beside it.

   The path is what changes first and it is the wrong moment to ask: which
   repository a project shows is decided an invoke later — `loadRepos` reads the
   list, `selectRepo` picks the remembered one — so a fetch fired on the path
   would go out for the repository being left, against a remote nobody is
   looking at any more, and stamp that one's throttle. Waiting for the selection
   asks about what is on screen.

   It also covers a person picking another repository inside one project, which
   is the same question one row further along: the panel is now drawing marks
   for a repository this session has never asked the remote about. The store
   holds the cost down — one call in flight per repository and one every five
   minutes — and the setting decides whether any of it happens at all. */
watch(
  () => vcsState.selected,
  (repo) => {
    if (repo) autoFetch()
  }
)

/* What stands in the scope bar: the chosen project's name and its branch. A
   detached HEAD is not a branch, and it is labelled as plainly as it looks: a
   short hash with a word. A dash means "there is nothing to show" — a folder
   without git, an unreadable .git, a HEAD in an unfamiliar shape: all of them
   ordinary states rather than failures, and what explains them is the absence
   of a branch, not an error message. */
const branchLabel = computed(() => {
  if (gitState.branch) return gitState.branch
  if (gitState.detached) return `${gitState.detached} (detached)`
  return '—'
})

/* The left panel belongs to the selected project, so its header says what that
   project is called and, under the name, where it stands. With no project open
   at all there is no name to say: the panel falls back to the section label it
   used to carry, which is also what `Panel` draws whenever there is no subtitle
   beside it.

   The summary's branch is `gitState.branch` rather than `branchLabel` above: a
   dash stands for "nothing to show" in the scope bar, where it holds a column
   open, and in a sentence it would be a word that means nothing. An empty
   branch is dropped by `projectSummary` instead. */
const activeProjectName = computed(() => (activePath.value ? basename(activePath.value) : 'Projects'))
const panelSummary = computed(() =>
  activePath.value ? projectSummary(gitState.branch, projectStates.value[activePath.value]) : ''
)
/* The missing-tracker mark, for the selected project. Every other project says
   it in its tile's tooltip, which is the only room a 28px tile has for it. */
const activeProjectTracked = computed(
  () => projectRows.value.find((row) => row.path === activePath.value)?.tracked !== false
)

/* An explicit refresh of the tree. Files deliberately have no watcher (see the
   spec), and this is the second half of the answer to "what is on disk right
   now" — the first half fires on its own when focus returns to the window. */
const refreshTree = () => refreshDirs(['', ...project.expanded])

/* ---- the bell -------------------------------------------------------------

   The panel hangs under the bell in the top right corner. It is placed here
   rather than by the component, the same split `ContextMenu` and `MenuButton`
   keep: the component draws a list or says it is empty, and knows nothing about
   where on the screen it is — which is what lets it be looked at in the gallery
   in an ordinary column.

   Whether the panel opens is not a preference and is deliberately not stored:
   it is a glance, and a window that reopened its notifications on every launch
   would be the fixture bell's other failure said differently. */
const notificationsOpen = ref(false)
const notificationsBox = ref(null)
const scopeBar = ref(null)

/* Anything outside closes it, with the whole scope bar excluded rather than the
   bell alone. The bell has to be excluded — pointerdown closing the panel and
   the click after it reopening would make a second press do nothing — and it is
   inside `ScopeIndicator`, which draws its own button and hands out no ref to
   it. The cost is that a press on the project name or a run's stop button
   leaves the panel open, which is a glance staying open beside a bar that has
   not moved. */
const onNotificationsPointerdown = (event) => {
  if (notificationsBox.value?.contains(event.target)) return
  if (scopeBar.value?.$el?.contains(event.target)) return
  closeNotifications()
}
const onNotificationsKeydown = (event) => {
  if (event.key === 'Escape') closeNotifications()
}
function closeNotifications() {
  if (!notificationsOpen.value) return
  notificationsOpen.value = false
  document.removeEventListener('pointerdown', onNotificationsPointerdown, true)
  window.removeEventListener('keydown', onNotificationsKeydown)
}
const toggleNotifications = () => {
  if (notificationsOpen.value) {
    closeNotifications()
    return
  }
  notificationsOpen.value = true
  document.addEventListener('pointerdown', onNotificationsPointerdown, true)
  window.addEventListener('keydown', onNotificationsKeydown)
}
onUnmounted(closeNotifications)

/* A card's own button. The switch is on the source rather than on the card's
   label: the label is prose and changes with the copy, while the source is what
   the card came from. A source added to `notifications.js` and forgotten here
   gets a button that does nothing, which is why the three places that name a
   source — the card's own `source`, `SOURCES` in the store, and this — move
   together.

   The update card leads to About rather than installing from here, and that is
   deliberate: installing restarts the app over unsaved editor buffers and live
   terminals, so the press that does it belongs beside the sentence naming the
   version and the refusal that may come back — which is the same bargain the
   storage card makes with Clean up. */
const actOnNotification = (notification) => {
  if (notification.source === 'storage') openSettingsWindow('storage')
  if (notification.source === 'run') showReport(notification.report)
  if (notification.source === 'update') openSettingsWindow('about')
  closeNotifications()
}

/* A run's report opened from its card. The document is an ordinary file under
   the project root, so this is the same call the file tree makes and there is
   no second way of opening a tab to keep in step with the first — `openFile`
   puts it in `openTabs`, makes it active and reads it, and `reportTabActive`
   above decides that what the centre draws is a report rather than an editor.
   Permanent rather than a preview: this is a document somebody asked for by
   name, and the next click in the tree must not evict it.

   The path arrives absolute and the tabs are project-relative; `reportTabPath`
   is that rule and the whole of it. It declines rather than guesses, and the
   refusal is logged rather than shown: it means a card outlived the project it
   was made in, which the store already prevents, so anybody meeting it is
   looking at a defect and not at something to act on. */
const showReport = (report) => {
  const path = reportTabPath(report, activePath.value)
  if (!path) {
    console.warn('[app] this run report is not in the open project:', report)
    return false
  }
  openFile(path, { permanent: true })
  return true
}

/* Endings this window has already dealt with, by token, in memory and nowhere
   else — the same reasoning `deliveredRuns` carries one file over, and the same
   token that is issued once per app process and never reused.

   It holds every ending this watcher has answered for — the ones left to the
   bell and the ones shown nothing at all, as well as the ones opened in a tab —
   and that width is the point: what is remembered is that the decision was
   *made*, not which way it went. Without it, an ending that went to the bell
   would be asked about again on the next `loadRun` — every window focus, every
   project switch — and an ending answered while the switch was off would be
   answered again the next time any run stopped, opening its tab if the switch
   had been turned back on in between. */
const decidedRuns = new Set()

/* The scope bar's one sentence about this project. Derived rather than stored,
   like everything else in this bar: the rule is components/shell/headline.js and
   both of its inputs are already reactive here.

   The agents come from `agentCounts` and not from the rail's `projectStates`,
   which is the map that knows about every project at once. That map counted a
   person's own shells when this was written, so a shell that rang the bell
   would have had this bar announce an agent waiting on somebody in a project
   holding no agent at all; the mark carries a work kind now and the map drops
   them, but the source is unchanged — the counter beside this sentence is built
   from the sessions and the two have to agree. The store comment beside
   `agentCounts` has the whole of it. This is the active project's bar, so the
   active project's own list is the right source anyway. */
const scopeHeadline = computed(() =>
  headline({ row: agentCounts.value, runs: runsState.runs })
)

/* Which runs have stopped, as a value that changes exactly when one does —
   `configFreshness.js`'s shape, and for the same reason: `upsert` writes a run
   back into the list in place, so a watcher over the array itself would need
   `deep` and would then wake on every field of every live run. */
const stoppedRuns = computed(() =>
  runsState.runs
    .filter((run) => run?.state?.kind === 'stopped')
    .map((run) => run.token)
    .join(' ')
)

/* Where a finished run's account goes, decided by `reportDelivery.js` and
   carried out here, because opening a tab is the one thing no store can do.

   The default `pre` flush is what keeps two deliveries from both being seen,
   and it is what keeps the switched-off case showing nothing at all:
   `syncRunCards` makes the card inside `upsert`, so for the moment between that
   and this the bell holds a card we are about to take back — and a `pre`
   watcher runs before this component's own render in the same tick, so the
   badge never paints the number. A `post` flush would show it for a frame.

   `markRunDelivered` is called when the tab actually opened, and when the
   answer was `none`. It is not called for `bell`: that card is the delivery and
   it stands until somebody takes it. `showReport` declines a document that is
   not in this project, and suppressing the card on the strength of a tab that
   never appeared would leave the person with neither — which is the same reason
   the answer `none` has to take the card back by hand rather than by never
   having made one, since `syncRunCards` made it before this watcher ran. */
watch(stoppedRuns, () => {
  for (const run of runsState.runs) {
    const where = deliveryFor(run, settings.notifications.showReport, decidedRuns)
    if (!where) continue
    decidedRuns.add(run.token)
    if (where === 'none') markRunDelivered(run.token)
    else if (where === 'tab' && showReport(run.summary.report)) markRunDelivered(run.token)
  }
})

/* When the store is weighed: at start once the project is resolved, on a
   switch (`projects.js`, after the new project's layout has landed), when focus
   returns — the same sweep the file tree and the branch ride on — and after an
   attachment is saved, which is the one moment the app knows for certain that
   the number changed. No watcher over the app's own data directory: reading a
   directory costs milliseconds and guards no state, the same reasoning `files/`
   and `git.rs` are built on.

   The list's length is what `attachmentsState.items` is watched for. Taking a
   thumbnail back out never deletes anything, so a shrinking list is a
   measurement that will find nothing changed — cheap, and cheaper than a second
   signal to keep in step with the store. */
watch(
  () => attachmentsState.items.length,
  () => measureStorage(activePath.value)
)

/* ---- styles ---------------------------------------------------------- */
const rootStyle = {
  display: 'flex',
  flexDirection: 'column',
  height: '100vh',
  background: 'var(--canvas)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  fontSize: 'var(--text-md)',
  overflow: 'hidden'
}
const bodyStyle = { flex: 1, minHeight: 0, display: 'flex', alignItems: 'stretch' }

/* Either side folds away to a 32px rail so the board gets the width; the rail
   keeps the panel's name and the button that brings it back. Open, the width is
   the clamped one — what was stored is what a person dragged to, not what fits
   the window they are in now. */
const leftStyle = computed(() => ({
  flex: '0 0 auto',
  /* The rail sits beside the panel inside this one column, so the column is
     both of them wide. Folded, there is no rail: a 44px strip of projects
     beside a 32px strip of nothing is two rails. */
  width: layout.leftCollapsed
    ? `${RAIL}px`
    : `${leftWidth.value + (railOpen.value ? PROJECT_RAIL : 0)}px`,
  display: 'flex',
  minWidth: 0
}))
/* Panel scrolls its slot as one block; the worktree line above and the tab row
   below have to stay put, so only what is between them scrolls. */
const sidebarStyle = {
  display: 'flex',
  flexDirection: 'column',
  height: '100%',
  minHeight: 0
}
/* The sidebar's own tab row: micro type, because these are section names and
   not open files. It sits directly under the panel header now, at the top of
   what it scopes rather than at the far end of it, and it is drawn as three
   segments instead of three full-height tabs.

   The inset rule and the raised fill the foot row used are gone with the
   position: a rule under a tab was that row's answer to sitting against the
   column's edge, and a segmented row marks its active segment by fill. The
   focus ring stays — it was kept explicitly at the design review, and these are
   the one control in the new left column that has one. */
const sideTabBar = {
  display: 'flex',
  alignItems: 'stretch',
  gap: 'var(--space-1)',
  flex: '0 0 auto',
  padding: 'var(--space-2) var(--space-3)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const sideTabStyle = (tab) => {
  const active = project.sideTab === tab.id
  return {
    flex: 1,
    minWidth: 0,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    /* The handoff draws a 22px segment; this is `--control-h-sm`, which is 24
       comfortable and 20 compact. The token rather than the number, because a
       literal would be the one height in the left column that neither density
       nor the app-wide font size reaches — and 22 is inside the two anyway. */
    height: 'var(--control-h-sm)',
    borderRadius: 'var(--radius-2)',
    font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
    letterSpacing: 'var(--tracking-caps)',
    textTransform: 'uppercase',
    color: active ? 'var(--text-primary)' : 'var(--text-muted)',
    background: active
      ? 'var(--surface-selected)'
      : hoveredSideTab.value === tab.id
        ? 'var(--surface-hover)'
        : 'transparent',
    cursor: 'default',
    transition: 'var(--transition-control)'
  }
}
/* The mark and the button that clears it are one hover target, tied together by
   a gap narrower than the header's own: the triangle is what a person sees
   without touching anything, the gear is what they press. The tooltip wraps the
   glyph alone rather than the pair — a panel centred over both would hang off to
   one side of whichever of them the pointer is on, and the button already
   carries its own label. */
const setupMarkStyle = { display: 'inline-flex', alignItems: 'center', gap: 'var(--space-2)' }
/* The absolute path of the project this panel belongs to. Broken anywhere
   rather than ellipsised: a path is read from both ends, and a panel 236px wide
   would otherwise show the first three segments of every one of them. */
const panelFootStyle = {
  font: 'var(--weight-regular) var(--text-2xs)/var(--leading-snug) var(--font-mono)',
  color: 'var(--text-muted)',
  wordBreak: 'break-all'
}
const centerStyle = { flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }

/* Collapsed, the column is the same rail AppShell reserves for one. */
const rightStyle = computed(() => ({
  flex: '0 0 auto',
  width: layout.rightCollapsed ? `${RAIL}px` : `${rightWidth.value}px`,
  display: 'flex',
  minWidth: 0
}))
/* Panel already owns the scroll container; this is only the layout inside it. */
const inspectorBody = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-5)',
  padding: 'var(--panel-pad)',
  minWidth: 0
}
/* The issue's title inside the delete dialog, in the same words the panel's
   own dialog drew it in — this is what a person reads to check they are about
   to delete the thing they meant. */
const deleteTitleStyle = {
  font: 'var(--weight-medium) var(--text-md)/var(--leading-snug) var(--font-sans)',
  color: 'var(--text-primary)',
  textWrap: 'pretty'
}
/* The parked questions, quoted verbatim in the Ready dialog. Prose rather than
   a table for the reason the inspector's own notes section carries: a note is
   somebody's sentence, and a row would promise a field it is not. The triangle
   beside each is `status/status.js`'s glyph for parked, so the dialog and the
   card the person came from say the same thing the same way. */
const questionListStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  marginTop: 'var(--space-4)'
}
const questionStyle = {
  display: 'flex',
  gap: 'var(--space-4)',
  alignItems: 'flex-start',
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-primary)',
  overflowWrap: 'anywhere'
}
const questionGlyphStyle = {
  flex: 'none',
  display: 'flex',
  marginTop: '2px',
  color: 'var(--attn-loud)'
}

/* Where the bell's panel sits: under the bar, against the right edge, clear of
   the gear by the same gutter the bar's own padding uses. Above everything, at
   the popover level, since it is opened over whatever is on screen. */
const notificationsBoxStyle = {
  position: 'fixed',
  top: 'calc(var(--scope-bar-h) + var(--space-2))',
  right: 'var(--space-5)',
  zIndex: 'var(--z-popover)'
}

/* The column of toasts in the corner. When empty it takes up nothing and
   intercepts nothing: with no children its size is zero. */
const toastStackStyle = {
  position: 'fixed',
  right: 'var(--space-6)',
  bottom: 'var(--space-6)',
  zIndex: 'var(--z-toast)',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'flex-end',
  gap: 'var(--space-4)'
}
</script>

<template>
  <div :style="rootStyle">
    <!-- The project's name and the branch it is on, both live. `worktree` is
         left empty on purpose: the component shows worktree-or-branch in that
         slot and appends "@branch" only when both are set, so passing the
         branch alone is what puts it there once, undecorated.

         Both counters are the stores' own computeds and neither is counted
         here: the files are the Git panel's selected repository, so the number
         is the length of the list that panel draws, and the agents are the
         left column's rows minus the ones that have finished. The two rules
         live in vcs.js and terminals.js because a rule in this file is a rule
         no test can reach. Note that with several repositories in one project
         the branch beside them is the project root's while the count is the
         selected repository's — the panel is where a person is looking at that
         list, and this is the number they can check against it. -->
    <ScopeIndicator
      ref="scopeBar"
      :repo="activePath ? basename(activePath) : '—'"
      worktree=""
      :branch="branchLabel"
      :dirty-count="dirtyCount"
      :agents-active="liveAgentCount"
      :headline="scopeHeadline.text"
      :headline-level="scopeHeadline.level"
      :notifications="notificationsState.items.length"
      @notifications="toggleNotifications"
      @settings="openSettingsWindow()"
    >
      <template #status>
        <!-- One segment per run, oldest first — a project holds several now,
             and each segment's stop names its own run by token. The scope
             bar's own gap spaces the segments; RunBar draws nothing for a
             run it was not given, so an empty list costs no width. `busy` is
             deliberately not bound: the run a confirm is starting has no
             segment until the worker answers, so `runStarting` is about none
             of these, and passing it disabled the other live runs' stop
             buttons over a start that never touches them. -->
        <RunBar v-for="r in runsState.runs" :key="r.token" :run="r" @stop="stopTheRun(r.token)" />
      </template>

      <!-- All the bar keeps of the search: the door, and the key that opens it.
           The palette itself is below, outside this bar. -->
      <template #search>
        <TaskSearchButton ref="searchButton" @open="openPalette" />
      </template>
    </ScopeIndicator>

    <!-- Every task in the project, searchable from the middle of the window.
         What it finds is the one thing on screen that reaches past the board: a
         closed task, and one in a column the board is not drawing today, are
         both findable here and nowhere else.

         Outside `ScopeIndicator` for the reason the bell's panel below is: the
         bar is a flex item in a column that clips, and an overlay positioned
         inside it would be cut off at its own first row. -->
    <CommandPalette
      :open="paletteOpen"
      :issues="searchableIssues"
      :edges="dependencyEdges"
      :recent="project.recentTasks"
      :pending="searchState.pending"
      :error="searchState.error ?? ''"
      :semantic-ids="searchState.ids"
      :answered="searchState.answered"
      @close="closePalette"
      @select="selectFromBoard"
      @semantic="searchSemantic"
      @reset="clearSemantic"
    />

    <!-- The bell's panel, hung under the corner it was opened from. Fixed
         rather than absolute: the bar is a flex item in a column that clips,
         and a panel positioned inside it would be cut off at its own first
         card. -->
    <div v-if="notificationsOpen" ref="notificationsBox" :style="notificationsBoxStyle">
      <NotificationPanel
        :items="notificationsState.items"
        @action="actOnNotification"
        @dismiss="dismissNotification($event.id)"
      />
    </div>

    <div :style="bodyStyle">
      <!-- left: worktree files and the agents working in it -->
      <div :style="leftStyle">
        <!-- One tile per project, and not while the column is folded: a 44px
             rail of projects beside the 32px rail a folded panel becomes is two
             rails, and neither would say which of them the button belongs to. -->
        <ProjectRail
          v-if="railOpen"
          :projects="projectRows"
          :active-path="activePath"
          :states="projectStates"
          :branches="activePath ? { [activePath]: gitState.branch } : {}"
          :can-add-agent="project.sideTab === 'agents'"
          :configured="configured"
          :config-broken="configBroken"
          @select="switchTo"
          @remove="removeProject"
          @add-agent="newAgent"
          @setup="openSetup"
          @add-project="onAddProject"
        />
        <!-- The header button closes the column one step at a time: it hides the
             project rail first and folds the whole column on the next press,
             and the button inside the folded rail brings both back. The steps
             are `leftChrome.js`'s, including the words, so a button cannot end
             up saying one thing and doing another. Dragging the separator past
             the panel's minimum still folds the column on its own, and the same
             rail button opens that too — which is why `Panel` keeps the two
             events apart. -->
        <Panel
          :title="activeProjectName"
          :subtitle="panelSummary"
          side="left"
          :collapsed="layout.leftCollapsed"
          :toggle-label="headerLabel(layout)"
          :expand-label="RAIL_EXPAND"
          :style="{ flex: 1, minWidth: 0 }"
          @toggle="applyLeftChrome(nextFromHeader(layout))"
          @expand="applyLeftChrome(nextFromRail())"
        >
          <!-- The three marks the project row used to carry, for the selected
               project alone, with the glyphs, colours and words they had there.
               None of them is told from the others by hue: the missing tracker
               is a lone muted triangle with nothing beside it that fixes it; the
               missing run configuration is a red triangle bonded to the gear
               that opens the setup it is asking for; and a configuration that
               cannot be parsed is a red page-with-a-cross, standing alone. The
               last needs its own glyph precisely because it stands alone —
               beside the tracker's lone triangle the two would differ in nothing
               but colour. It offers no button on purpose: a file that exists and
               cannot be read must not be answered by a button that starts an
               agent writing over it, and the way out is the tile's menu, whose
               setup item is live over a damaged file. -->
          <template #marks>
            <Tooltip v-if="activePath && !activeProjectTracked" label="No bd tracker here">
              <Icon name="triangle-alert" :size="12" :style="{ color: 'var(--text-muted)' }" />
            </Tooltip>
            <span v-if="needsSetup" :style="setupMarkStyle">
              <Tooltip label="Not set up for runs">
                <Icon name="triangle-alert" :size="12" :style="{ color: 'var(--status-failed-fg)' }" />
              </Tooltip>
              <IconButton
                icon="settings-2"
                label="Set up for runs"
                size="sm"
                @click="openSetup(activePath, false)"
              />
            </span>
            <Tooltip v-if="configBroken" label="Run configuration cannot be read">
              <Icon name="file-x" :size="12" :style="{ color: 'var(--status-failed-fg)' }" />
            </Tooltip>
          </template>
          <template #actions>
            <!-- There is nothing to refresh on the Agents tab: the sessions
                 announce their own state, and the button would promise work
                 that does not exist. The other two both read the disk on window
                 focus and both offer the same button for the times a person
                 does not want to wait for it. -->
            <IconButton
              v-if="project.sideTab === 'files'"
              icon="refresh-cw"
              label="Refresh files"
              size="sm"
              @click="refreshTree"
            />
            <IconButton
              v-else-if="project.sideTab === 'git'"
              icon="refresh-cw"
              label="Refresh git"
              size="sm"
              @click="refreshGit"
            />
          </template>
          <div :style="sidebarStyle">
            <!-- Above what it scopes rather than at the foot of the column. The
                 roving tabindex, the roles and the focus ring are the ones the
                 foot row had; only the position and the fill changed. -->
            <div role="tablist" :style="sideTabBar">
              <div
                v-for="t in SIDE_TABS"
                :key="t.id"
                role="tab"
                :aria-selected="project.sideTab === t.id"
                :tabindex="project.sideTab === t.id ? 0 : -1"
                :style="sideTabStyle(t)"
                @click="project.sideTab = t.id"
                @mouseenter="hoveredSideTab = t.id"
                @mouseleave="hoveredSideTab = null"
              >
                {{ t.label }}
              </div>
            </div>
            <div :style="{ flex: 1, minHeight: 0, overflow: 'auto' }">
              <!-- `filesState.root` and not `activePath`: every path this
                   panel produces is relative to that root and its menu's verbs
                   join the two, and `moveTo` sets the active project one await
                   before it sets the root. With nothing to hang a path off,
                   there is no tree to draw and — the point of the guard — no
                   menu to open over the empty panel offering to copy `''`. -->
              <FileTree
                v-if="project.sideTab === 'files' && filesState.root"
                :nodes="tree"
                :expanded="expanded"
                :selected-path="project.selectedPath ?? undefined"
                :can-attach="attachTarget !== null"
                :has-live-agent="hasLiveAgent"
                @toggle="toggleDir"
                @select="onSelectFile"
                @open="onOpenFile"
                @action="onFileAction"
              />
              <GitPanel
                v-else-if="project.sideTab === 'git'"
                :repos="vcsState.repos"
                :selected="vcsState.selected"
                :tree="vcsState.tree"
                :branches="vcsState.branches"
                :tracking="vcsState.tracking"
                :actions="gitWrites"
                :busy="vcsState.busy"
                :fetching="vcsState.fetching"
                :write-error="vcsState.writeError"
                :error="vcsState.error"
                :loading="vcsState.loading"
                :open-path="activeDiff?.repo === vcsState.selected ? activeDiff.path : null"
                :sections="resolvedGitSections"
                :branch-folders="project.branchFolders"
                :message="draftMessage()"
                :suggesting="vcsState.suggesting"
                :suggest-error="vcsState.suggestError"
                @toggle="toggleGitSection"
                @toggle-folder="toggleBranchFolders"
                @resize="resizeGitSection"
                @select="selectRepo"
                @checkout="checkout"
                @merge="merge"
                @rebase="rebase"
                @pull="pull"
                @push="push"
                @fetch="fetchNow"
                @new-branch="newBranchFrom = $event"
                @message="setMessage"
                @commit="commit"
                @suggest="suggestMessage"
                @open="openDiff(vcsState.selected, $event.path)"
              />
              <AgentList
                v-else
                :rows="agentRows"
                :active-id="terminalState.activeId"
                @select="selectAgent"
                @remove="removeSession"
              />
            </div>
          </div>
          <template v-if="activePath" #footer>
            <div :style="panelFootStyle">{{ activePath }}</div>
          </template>
        </Panel>
      </div>

      <Resizer
        label="Resize left panel"
        :step="STEP"
        @dragstart="startDrag('left')"
        @drag="onDrag('left', $event)"
        @reset="resetWidth('left')"
      />

      <!-- centre: tabs over the board -->
      <div :style="centerStyle">
        <!-- The dragged order is written straight through, with no `mergeOrder`
             beside it as the board has. The board needs one because a hidden
             column would otherwise be struck out of the stored order by the
             first drag; the row draws every tab it has — `overflowCount` is a
             prop nobody connects — so what comes back is the whole of the row
             and there is nothing to merge it with. Connect an overflow menu one
             day and that stops being true, and a tab that was not drawn will
             want its place kept exactly the way `boardView.js:mergeOrder` keeps
             a column's. -->
        <TabBar
          :tabs="tabList"
          :active-id="project.activeTab"
          @select="project.activeTab = $event"
          @close="onCloseTab"
          @promote="promote"
          @reorder="project.tabOrder = $event"
        >
          <!-- Beside the pinned block rather than at the far right of the row:
               it is about those first two tabs, and past the strut it would
               drift away from them. The block is what it names and not the board
               within it, since which of the pair the button ends up against is
               the order's to decide. Disabled with no project open, where
               no row has anywhere to start anything — the two that open a tab
               have no project root to open it in, and the third would file a
               task against no tracker. -->
          <template #afterPinned>
            <MenuButton
              icon="plus"
              label="New agent, terminal or task"
              :items="NEW_TAB_ITEMS"
              :width="180"
              :disabled="!activePath"
              @select="onNewTab"
            />
          </template>
        </TabBar>
        <NewTaskModal
          :open="newTaskOpen"
          :busy="creating"
          :status="ADD_TO"
          :parent="followUpParent"
          :attachments="attachmentsState.items"
          :dragging="attachmentsState.dragging"
          :error="attachmentsState.lastError ?? ''"
          @close="closeNewTask"
          @submit="submitNewTask"
          @attach="pickImages"
          @files="attachFiles"
          @remove="removeAttachment"
        />
        <PromoteColumnModal
          :open="promoteOpen"
          :count="promoteIds.length"
          :busy="promoting"
          :moved="promoted"
          :failed="promoteFailed"
          @close="closePromote"
          @confirm="confirmPromote"
        />
        <RunModal
          :open="runOpen"
          :scope="runScope"
          :count="runCount"
          :part-of="runParent"
          :branches="gitState.branches"
          :default-branch="runConfig?.defaults?.target_branch ?? branchLabel"
          :default-priority="runConfig?.defaults?.min_priority ?? 2"
          :default-parallel="runConfig?.defaults?.max_parallel_tasks ?? 3"
          :remembered="project.runSettings"
          :live-check-available="runConfig?.live_check?.mode !== 'none'"
          :live-check-blocked="liveCheckBlocked"
          :config-error="configErrorText"
          :error="runError"
          :busy="runStarting"
          @close="runOpen = false"
          @confirm="startTheRun"
          @rescope="runTheEpicInstead"
        />
        <SetupProjectModal
          :open="!!setupFor"
          :name="setupFor ? basenameOf(setupFor) : ''"
          :existing="setupExisting"
          :busy="settingUp"
          @close="closeSetup"
          @confirm="startSetup"
        />
        <!-- Delete, asked for from a card's own menu. It used to live inside
             TaskInspector and had to be teleported out of Panel's scroll
             container to be drawn at all; here there is no `overflow` box over
             it, so it is written plainly like every other dialog in this view.
             The issue is read from the store by id rather than held, so the
             dialog names what the board holds now. -->
        <Modal
          :open="!!confirmedIssue"
          :closable="!deletingId"
          :title="`Delete ${confirmedIssue?.id}?`"
          description="bd deletes the issue outright and rewrites references to it in whatever was linked to it. Anything that depended on this issue is left without the dependency. There is no undo."
          @close="confirmingDelete = null"
        >
          <div :style="deleteTitleStyle">{{ confirmedIssue?.title }}</div>
          <template #footer>
            <Button variant="ghost" :disabled="!!deletingId" @click="confirmingDelete = null">Cancel</Button>
            <Button variant="danger" :disabled="!!deletingId" @click="deleteTask(confirmedIssue.id)">
              {{ deletingId ? 'Deleting…' : 'Delete' }}
            </Button>
          </template>
        </Modal>
        <!-- Parked, on its way back to Ready. The questions themselves are
             quoted rather than summarised: this is the one moment somebody
             decides whether they matter, and a dialog that only said "there are
             questions" would send them to the card to find out. Three ways out
             and the recommended one last, where every other dialog here puts
             the action it expects. -->
        <Modal
          :open="!!readyIssue"
          :title="`Move ${readyIssue?.id} to ready with the question unanswered?`"
          :description="readyQuestions.length
            ? 'An agent parked this because it could not settle something on its own. Moving it to ready puts it back in the queue, and whoever takes it next meets the same question.'
            : 'An agent parked this and left no note saying why. Moving it to ready puts it back in the queue, and whatever stopped the last agent is still there.'"
          @close="confirmingReady = null"
        >
          <div :style="deleteTitleStyle">{{ readyIssue?.title }}</div>
          <div v-if="readyQuestions.length" :style="questionListStyle">
            <div v-for="(question, i) in readyQuestions" :key="i" :style="questionStyle">
              <span :style="questionGlyphStyle"><Icon name="triangle-alert" :size="14" /></span>
              <span>{{ question }}</span>
            </div>
          </div>
          <template #footer>
            <Button variant="ghost" @click="confirmingReady = null">Cancel</Button>
            <Button variant="secondary" @click="moveToReadyAnyway">Move anyway</Button>
            <Button variant="primary" @click="resolveFromDialog">Answer questions</Button>
          </template>
        </Modal>
        <!-- A merge or a rebase that stopped on conflicts. It has no dismiss
             and takes no `close`: the two doors are the only ways out, because
             a conflicted tree behind a closed dialog is a state the panel
             promises to show and has nothing to draw it with. Everything in it
             comes from the record the store made when git answered, including
             which repository — the panel's selection can have moved since. -->
        <!-- Cutting a branch, from the row the menu was opened on. It lives here
             beside the conflict dialog rather than inside `GitPanel` for the
             reason that one does: a modal belongs to the window, and a panel
             that is 252px wide and scrolls is no place to hang one from. The
             verdict and `busy` go in live, because a run can start while the
             dialog is open and the button has to go dead when it does. -->
        <NewBranchModal
          :open="newBranchFrom !== null"
          :from="newBranchFrom"
          :branches="vcsState.branches"
          :actions="gitWrites"
          :busy="Boolean(vcsState.busy)"
          @close="newBranchFrom = null"
          @create="cutBranch"
        />
        <ConflictModal
          v-if="vcsState.conflict"
          :open="true"
          :op="vcsState.conflict.op"
          :repo="vcsState.conflict.repo"
          :ours="vcsState.conflict.ours"
          :theirs="vcsState.conflict.theirs"
          :files="vcsState.conflict.files"
          :busy="vcsState.busy?.op === 'abort'"
          :error="vcsState.conflictError"
          @resolve="resolveConflictWithAgent"
          @abort="abortConflict"
        />
        <Modal
          v-if="unsaved"
          :open="true"
          title="Save changes?"
          :description="unsaved.paths.length === 1
            ? `${basenameOf(unsaved.paths[0])} has unsaved changes.`
            : `${unsaved.paths.length} files have unsaved changes.`"
          :closable="false"
        >
          <template #footer>
            <Button variant="secondary" size="sm" @click="answerUnsaved('cancel')">Cancel</Button>
            <Button variant="secondary" size="sm" @click="answerUnsaved('discard')">Don't save</Button>
            <Button variant="primary" size="sm" @click="answerUnsaved('save')">Save</Button>
          </template>
        </Modal>
        <!-- A run's report, before the editor branch and not beside it: the
             buffer is the one tabs.js already loads for any open path, so the
             document needs no second read path and inherits the same loading
             and error handling every other tab has. What it does not inherit is
             the field — a report is read, never edited, which is why this is a
             branch of its own rather than a mode of FileEditor. -->
        <ReportView v-if="reportTabActive" :html="activeBuffer?.text ?? ''" :theme="theme" />
        <!-- A changed file, HEAD against the working tree. Before the editor
             branch for the same reason the report is: it is a tab of its own
             kind, with no buffer behind it and nothing to save. -->
        <DiffView
          v-else-if="activeDiff"
          :path="activeDiff.path"
          :head="activeDiff.head"
          :work="activeDiff.work"
          :missing-at-head="activeDiff.missingAtHead"
          :notice="diffNotice"
        />
        <!-- A file tab: the board and the chat have nothing to do with it. -->
        <!-- There is no :key here any more: the field survives a tab switch
             deliberately. editor/states.js keeps the caret, the scroll position
             and the edit history per tab, and FileEditor switches state by
             :path. -->
        <FileEditor
          v-else-if="fileTabActive"
          :path="absoluteEditorPath(project.activeTab)"
          :model-value="activeBuffer?.text ?? ''"
          :read-only="!!activeBuffer?.error || !!activeBuffer?.loading"
          :word-wrap="settings.editor.wordWrap"
          :notice="editorNotice"
          @update:model-value="setText(project.activeTab, $event)"
          @reload="reloadTab(project.activeTab)"
          @keep-mine="keepMine(project.activeTab)"
        />
        <!-- The Agent tab shows the agent a person picked; a terminal tab shows
             its own shell. Two branches over one component, and the session is a
             prop rather than something the pane reads for itself: see the note
             on `sessionId` in TerminalView.vue. -->
        <TerminalView
          v-else-if="project.activeTab === 'terminal'"
          :session-id="terminalState.activeId"
        />
        <TerminalView v-else-if="activeTerminal" :session-id="activeTerminal.session" />
        <!-- bd init is the one wait that keeps its EmptyState: the skeleton
             would replace the very sentence that explains what is happening,
             and the busy button says it better than six grey lines. Every
             other switch shows the skeleton — there the board is what is
             being replaced. -->
        <div v-else-if="trackerState.switching && !initing" :style="{ padding: 'var(--panel-pad)' }">
          <Skeleton :lines="6" :height="12" />
        </div>
        <EmptyState v-else-if="healthNotice" v-bind="healthNotice">
          <template v-if="trackerState.health.state === 'not-a-beads-repo'" #action>
            <Button variant="primary" size="sm" :disabled="initing" @click="initHere">
              {{ initing ? 'Initializing…' : 'Initialize bd' }}
            </Button>
          </template>
          <template v-else-if="trackerState.health.state === 'no-project'" #action>
            <Button variant="primary" size="sm" @click="onAddProject">Add project…</Button>
          </template>
        </EmptyState>
        <KanbanBoard
          v-else
          :columns="drawnColumns"
          :filtered="orderedColumns.length > 0"
          :selected-id="highlightedTask"
          :add-to="ADD_TO"
          :run-from="runOffered ? ADD_TO : null"
          :run-blocked-reason="runBlockedReason"
          :promote-from="PROMOTE_FROM"
          @select="selectFromBoard"
          @add="newTaskOpen = true"
          @run="openRun({ kind: 'queue' })"
          @promote="openPromote"
          @task-action="onTaskAction"
          @reorder="project.columnOrder = mergeOrder($event, projectColumns)"
        />
      </div>

      <Resizer
        label="Resize task panel"
        :step="STEP"
        @dragstart="startDrag('right')"
        @drag="onDrag('right', $event)"
        @reset="resetWidth('right')"
      />

      <!-- right: the task that is waiting on you, and everything known about it -->
      <div :style="rightStyle">
        <Panel
          title="Task &amp; details"
          side="right"
          :collapsed="layout.rightCollapsed"
          :style="{ flex: 1, minWidth: 0 }"
          @toggle="layout.rightCollapsed = !layout.rightCollapsed"
          @expand="layout.rightCollapsed = false"
        >
          <!-- The second way to the card's menu, and the same menu: the items
               come from the card this panel is drawing, so the two cannot say
               different things. In the panel's header rather than inside
               `TaskInspector`, so it stays in the corner while the issue
               scrolls under it, and so the component that draws an issue keeps
               knowing nothing about runs or writes.

               Drawn whenever there is an issue in the panel — from the board or
               from a run's claimed list — and absent over a draft and over the
               empty state, where there is no issue and nothing to act on. The
               menu is wider than this column ever gets, which costs nothing:
               `MenuButton` is fixed-position, right-aligned to the trigger and
               clamped to the window, so it opens leftwards over the board. -->
          <template #actions>
            <MenuButton
              v-if="inspectedIssue"
              :items="inspectedMenu"
              :label="`Actions for ${inspectedIssue.id}`"
              :width="MENU_W"
              icon="ellipsis"
              size="sm"
              @select="onTaskAction({ kind: $event.kind, id: inspectedIssue.id, value: $event.value })"
            />
          </template>

          <div :style="inspectorBody">
            <!-- A task still being filed: the person's own words, read-only,
                 with no issue behind them. Alone in the column — there is no
                 card selected on the board while this is up. -->
            <DraftInspector v-if="rightPanel === 'draft'" :draft="agentDraft" />

            <!-- What a run has taken, and the card for whichever of them is
                 picked. The list stays above the card: the choice between them
                 is the point, and a card that replaced the list would take the
                 way back with it. -->
            <ClaimedTasks
              v-else-if="rightPanel === 'claimed'"
              :tasks="claimedTasks"
              :selected-id="highlightedTask"
              @select="project.selectedTask = $event"
            />

            <!-- A link in one of the issue's prose fields goes to the person's
                 own browser: the panel raises it, and this is where the app's
                 one link-opening path is bound to it. -->
            <TaskInspector
              v-if="inspectedIssue"
              :issue="inspectedIssue"
              :ui-status="toUiStatus(inspectedIssue.status)"
              @open="openExternal"
            />

            <!-- Nothing picked on the board, which is where a project opens.
                 The app's own answer to "nothing here" rather than a blank
                 column: a silent panel beside a full board reads as a
                 rendering failure. Only the board case — under a run's
                 claimed list the list itself is the content, and an empty
                 state beneath it would say the panel was empty when it is
                 not. -->
            <EmptyState
              v-else-if="rightPanel === 'board'"
              compact
              icon="inbox"
              title="No task selected"
              description="Pick a card on the board to see it here."
            />
          </div>
        </Panel>
      </div>
    </div>

    <!-- The toasts live in one column: two fixed corners would overlap each
         other, and a tracker failure would hide a disk failure exactly when
         both are broken. -->
    <div :style="toastStackStyle">
      <Toast
        v-if="trackerState.lastError"
        tone="error"
        :title="trackerState.lastError.title"
        :description="trackerState.lastError.description"
        @close="trackerState.lastError = null"
      />
      <Toast
        v-if="filesState.lastError"
        tone="error"
        title="Could not read the file tree"
        :description="filesState.lastError"
        @close="filesState.lastError = null"
      />
      <Toast
        v-if="terminalState.lastError"
        tone="error"
        :title="terminalState.lastError.title"
        :description="terminalState.lastError.description"
        @close="terminalState.lastError = null"
      />
      <!-- The file tree's menu, which is the one thing in this column that has
           a success to report: a copy leaves nothing on screen behind it. -->
      <Toast
        v-if="fileMenuToast"
        :tone="fileMenuToast.tone"
        :title="fileMenuToast.title"
        :description="fileMenuToast.description"
        @close="fileMenuToast = null"
      />
    </div>
  </div>
</template>
