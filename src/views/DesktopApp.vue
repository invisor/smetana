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
import SegmentedTabs from '../components/shell/SegmentedTabs.vue'
import TabBar from '../components/shell/TabBar.vue'
import { NEW_TAB_ITEMS } from '../components/shell/newTabMenu.js'
import { headline } from '../components/shell/headline.js'
import { CHROME_NONE, CHROME_STATES, chromeInFullscreen } from '../components/shell/windowChrome.js'
import FileTree from '../components/files/FileTree.vue'
import ConflictModal from '../components/git/ConflictModal.vue'
import GitPanel from '../components/git/GitPanel.vue'
import { gitActions } from '../components/git/gitActions.js'
/* What the branch-review window's table is built out of, and what a press of
   Review turns it into. Pure and outside the component that draws it, for the
   reason that whole family is: a `.vue` file is the one thing no test in this
   repository can reach. */
import {
  fetchFailures,
  fetchTargets,
  reportPath,
  reviewPairs,
  reviewRows
} from '../components/git/reviewRows.js'
import {
  NO_VISIT,
  answeredCount,
  changesVisible,
  enterGitTab,
  gitAnswered,
  toggleChanges
} from '../components/git/changesFold.js'
import KanbanBoard from '../components/kanban/KanbanBoard.vue'
import { pickBranch } from '../components/run/branchChoice.js'
import { orderColumns } from '../components/kanban/columnOrder.js'
import { mergeOrder, visibleColumns } from '../components/kanban/boardView.js'
import { isParked, needsReadyWarning, openQuestions, READY } from '../components/kanban/parked.js'
import { MENU_W, taskMenuItems } from '../components/kanban/taskMenu.js'
/* The whole of what happens on screen after a copy — the board's id and a
   session row's menu both, one policy and one duration for the two of them, and
   the same one the gallery answers with. */
import { useCopyFeedback } from '../components/core/copyFeedback.js'
import { promoteTitle, taskCount } from '../components/kanban/promoteTitle.js'
import Button from '../components/core/Button.vue'
import RunBar from '../components/run/RunBar.vue'
import ReportView from '../components/run/ReportView.vue'
import { isReportPath, reportTabPath, reviewReportTabs } from '../components/run/reportTab.js'
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
import StatusFooter from '../components/shell/StatusFooter.vue'
import Skeleton from '../components/core/Skeleton.vue'
import Icon from '../components/core/Icon.vue'
import Tooltip from '../components/core/Tooltip.vue'
import IconButton from '../components/core/IconButton.vue'
import { CommandPalette, TaskSearchButton, TerminalView } from '../components/index.js'
import AgentList from '../components/agent/AgentList.vue'
import SessionRow from '../components/agent/SessionRow.vue'
import {
  DELETE_SESSION_TITLE,
  FORK_KIND,
  RESUME_KIND,
  copyNoun as copyVerbNoun,
  copyPayload,
  isCopyKind,
  resumeAvailability
} from '../components/agent/sessionMenu.js'
/* The right column's Sessions tab, and a different subject from the store
   below it: this one is Claude Code's own transcripts on disk, that one is the
   live PTY sessions of this run of the app. The two lists never mix. */
import {
  deleteSessionTranscript,
  initSessions,
  loadSessionHistory,
  openSessionDirectory,
  openSessionLog,
  revealSessionLog,
  sessionsState
} from '../stores/sessions.js'
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
  lastDiagnosticLine,
  repairTracker,
  resetFolderAccess,
  searchSemantic,
  searchState,
  toUiStatus,
  trackerFailure,
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
  announceDialogProps,
  closeDialogWindow,
  closeWindow,
  copyText,
  isWindowMaximized,
  minimizeWindow,
  openDialogWindow,
  openExternal,
  openSettingsWindow,
  readAgentUsage,
  revealInFileManager,
  toggleMaximizeWindow,
  watchBoardHello,
  watchDialogHello,
  watchDialogResult,
  watchFullscreen
} from '../stores/app.js'
import { dialogWidth, stalenessMessage, stalenessOf } from './dialogRegistry.js'
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
/* The compare window, which is a window rather than a panel: this view opens it
   and hears nothing back. Its state lives in that window's own webview
   (`stores/compare.js`), which is why nothing else of it is imported here. It
   is opened on `vcsState.selected` — the repository whose branches the panel is
   listing, which is the repository the row belongs to, so a project made of
   several compares the one on screen. */
import { openCompareWindow } from '../stores/compare.js'
/* The Git panel's own state, beside git.js rather than inside it: that store is
   the branch in the scope bar and spawns no process, this one runs git. */
import {
  abortConflict,
  autoFetch,
  fetchNow,
  checkout,
  commit,
  createBranch,
  deleteBranch,
  dirtyCount,
  dismissConflict,
  draftMessage,
  fetchIn,
  loadRemoteBranches,
  loadRepos,
  merge,
  openConflict,
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
  configError,
  initRuns,
  loadBrowserTools,
  loadConfig,
  loadRun,
  needsSetup,
  runsState,
  saveDefaults,
  startRun,
  stopRun
} from '../stores/runs.js'
import { initUpdates } from '../stores/updates.js'
import { liveCheckBlock } from '../components/run/browserTools.js'
import { folderOf, parentOf, relativePath } from '../components/files/fileMenu.js'
import { pasteSource } from '../components/files/fileClipboard.js'
import { checkNewName } from '../components/files/newEntry.js'
/* Two imports straight from `src/paths.js` rather than through a store: both
   are conversions between two path spaces and belong to neither of the stores
   that hold them. `tabs.js` reaches for `relativeTo` for the same join, and
   `absolutePath` sat in `fileMenu.js` until the system clipboard wanted it
   too. */
import { absolutePath, relativeTo } from '../paths.js'
import { dropText } from '../components/terminal/dropPaths.js'
import { workingKey } from '../components/run/configFreshness.js'
import { runTitle, scopeBusyReason } from '../components/run/runScopes.js'
import { DEFAULTS_FALLBACK, draftFrom } from '../components/run/projectDefaults.js'
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
import { folderRefusedHasReset, folderRefusedNotice } from './folderAccess.js'
import {
  basenameOf,
  copyEntry,
  copyErrorText,
  copyExternalEntry,
  createDir,
  createFile,
  fileErrorText,
  filesState,
  isStubPath,
  listDir,
  makeErrorText,
  moveEntry,
  readSystemClipboard,
  refreshDirs,
  renameEntry,
  renameErrorText,
  saveErrorText,
  setClipboard,
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
  renameTab,
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
  density: { type: String, default: 'comfortable' },
  /* Which chrome the window around this view has, from
     `components/shell/windowChrome.js`. A prop rather than a question asked
     here, because it has to be settled before this view's first paint — see
     `App.vue`, which resolves it in the same wait as the settings file. */
  windowChrome: {
    type: String,
    default: CHROME_NONE,
    validator: (value) => CHROME_STATES.includes(value)
  }
})

/* The two halves of the window's state that do change while it is open. The
   chrome itself does not — it is what the platform gave us, and it arrives as a
   prop already settled. */
const fullscreen = ref(false)
const maximized = ref(false)

/* What the bar actually draws: the window's chrome, minus the traffic lights
   while a fullscreen window has moved them into its own auto-hiding bar. */
const barChrome = computed(() => chromeInFullscreen(props.windowChrome, fullscreen.value))

/* Held here and torn down synchronously, the way `stopBoardHello` below is: the
   subscription is only reached after an await, by which point there is no
   active component instance left for an `onUnmounted` inside the callback to
   register against. */
let stopFullscreen = null
onMounted(async () => {
  /* Nothing to watch in a browser, and asking would cost a second line in a
     console this view keeps quiet on purpose: `none` is the answer precisely
     when there is no window behind the page. */
  if (props.windowChrome === CHROME_NONE) return
  maximized.value = await isWindowMaximized()
  stopFullscreen = await watchFullscreen(async (value) => {
    fullscreen.value = value
    maximized.value = await isWindowMaximized()
  })
})
onUnmounted(() => stopFullscreen?.())

/* Both switches live on the document root: every token is defined against them,
   and the chrome above rides beside them for the same reason. So does the type
   scale, which the settings window's app-wide font size rewrites there token by
   token — that way no component knows about it and the editor and the terminal
   come along for free (see `useAppearance.js`). The theme arrives already
   resolved: `system` is App.vue's to answer, since it is the machine's answer
   and not a stored one. */
watchEffect(() =>
  paintRoot(document.documentElement, {
    theme: props.theme,
    density: props.density,
    uiFontSize: settings.appearance.uiFontSize,
    editorFontSize: settings.editor.fontSize,
    windowChrome: barChrome.value
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

/* The right column's own two: the task the person is looking at, and the agent
   sessions running in this project. Both halves of one panel rather than two
   panels, since a column has room for one thing at a time and which of the two
   somebody wants is a choice they make rather than a state to derive.

   The same doubling `SIDE_TABS` carries, and the same warning: these two ids
   are the closed list in `src-tauri/src/settings/model.rs` (`RIGHT_TABS`) as
   well. A third tab added only here would work all session and come back as
   Task after a restart, with no error anywhere. */
const RIGHT_TABS = [
  { id: 'task', label: 'Task' },
  { id: 'sessions', label: 'Sessions' }
]
onMounted(initTracker)
onMounted(adoptInitialProject)
onMounted(initTerminals)
/* Starts the clock the session rows' "18h ago" is measured against, and nothing
   else: the list itself is read when the tab is opened, not here. */
onMounted(initSessions)
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
   dialog, the same window the `+` above the `ready` column opens. */
const onNewTab = (item) => {
  if (item.kind === 'agent') newAgent()
  else if (item.kind === 'terminal') newTerminal()
  else if (item.kind === 'task') openNewTask()
  /* The fourth row, and the second door into the branch-review window: no name
     to start from, so the table opens as one empty row on the repository the
     Git panel is showing. */
  else if (item.kind === 'review') openReviewChanges()
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

/* The app's dialogs are windows of their own, and this is the app window's half
   of that: what it opens, what it keeps telling them, and what it does when the
   ground one of them stands on goes.

   Which dialogs there are, how wide each is and what each stands on is
   `views/dialogRegistry.js`, which is pure and has the tests. What is here is
   the wiring only.

   Each open dialog is served rather than rendered: a `watchEffect` announces its
   props now and again on every change, and that is what makes a live value —
   `busy` while a run starts, a branch list that has just landed — reach a window
   that is already up. Nothing about a window is stored in `settings.json`: a
   dialog somebody has half filled in does not survive a project switch, and the
   store is where the things that do survive live. */
const openDialogs = new Map()

/* Deliberately a plain `Map` and not a `reactive` one. Nothing draws from it —
   the only reader is the watcher below, and it reads inside its callback rather
   than in its source — so reactivity here would buy a proxy around a structure
   holding stop functions and nothing else. */
function serveDialog(kind, { ground, props: propsFor, onResult, forget = null }) {
  /* Reopening a kind that is already open: the window is brought forward rather
     than made twice, so the service has to be replaced rather than stacked, or
     the same props would be announced by two watchers.

     The old service's state is deliberately **not** forgotten on the way
     through. Every caller here sets what its dialog is about and then opens —
     the branch a cut starts from, the issue a delete names, the ids a promote
     took — so a `forget` here would clear the very thing the line above it just
     wrote, and the window that came forward would be about nothing: "Cut from"
     with no branch, `Delete undefined?`, a count of zero. The state belongs to
     whichever opening is current, and that is this one. */
  if (openDialogs.has(kind)) stopServing(kind, { forget: false })

  const service = { ground, forget, stops: [], closed: false }
  /* A subscription that arrives after the dialog has already closed is stopped
     on the spot rather than kept: the two `listen` calls are promises, and a
     window closed inside that gap would otherwise leave a listener nobody holds
     the way to stop. */
  const collect = (stop) => {
    if (service.closed) stop()
    else service.stops.push(stop)
  }

  collect(watchEffect(() => announceDialogProps(kind, propsFor())))
  const failed = (err) => console.warn('[app] the dialog window will not be answered:', err)
  watchDialogHello(kind, () => announceDialogProps(kind, propsFor())).then(collect, failed)
  watchDialogResult(kind, onResult).then(collect, failed)

  openDialogs.set(kind, service)
  openDialogWindow(kind, dialogWidth(kind))
}

/* The ground under a dialog window that is already up, changed while it stands.

   **One caller and one reason**, and it is worth reading before adding a
   second. Pressing Delete in the `delete-branch` window is the moment that
   window stops standing on the branch existing: it is the thing about to make
   it not exist. Left as it was, the refresh that follows a successful delete
   takes the branch out of `vcsState.branches`, the ground watcher at the foot
   of this file finds a window standing over nothing, and the person is told
   "the branch it was about is gone" — a notice written for a board that moved
   under somebody, raised over the very act they just performed. The branch goes
   back on if git refuses, since the window is then standing over a branch that
   still exists and that somebody else can still delete from a terminal.

   `null` is what `stalenessOf` reads as "this window does not stand on one",
   which is that rule's own documented shape and not a new one — the project and
   the repository still hold it up in the meantime. */
function reground(kind, ground) {
  const service = openDialogs.get(kind)
  if (service) service.ground = ground
}

/* Everything this window was doing for one dialog, undone — but nothing said to
   the window itself. Split from `closeDialog` because the window closing is one
   of the two ways this is reached: a person pressing the frame's own cross is
   the window telling us, and asking it to close again would be an answer to a
   question it already answered. */
function stopServing(kind, { forget = true } = {}) {
  const service = openDialogs.get(kind)
  if (!service) return
  service.closed = true
  for (const stop of service.stops) stop()
  openDialogs.delete(kind)
  if (forget) service.forget?.()
}

/* The app window closing a dialog window: the ground went, or the guest asked.
   `closeDialogWindow` is safe to call on a window that is not there — the person
   may have closed it a moment earlier — which is what lets this be the one path
   for both. */
function closeDialog(kind) {
  stopServing(kind)
  closeDialogWindow(kind)
}

/* Nothing of this window survives it. A dialog window is fed entirely from here,
   so one left standing would be a question nothing can answer — Rust closes them
   with the main window for that reason, and this is the same tidying on the
   front end's side of the wire. */
onUnmounted(() => {
  for (const kind of [...openDialogs.keys()]) stopServing(kind)
})

/* Which branch the new-branch dialog was opened from, and null for closed. The
   branch is the state rather than a boolean beside it: what the dialog is about
   is entirely the row somebody right-clicked, and a flag with the name kept
   somewhere else is two things to clear instead of one.

   It is still here now that the dialog is a window, because it is what the
   announcement is built out of: the window draws what this ref says, and the
   ground the window stands on is that same branch. */
const newBranchFrom = ref(null)

/* Cutting a branch, in a window of its own rather than a modal over the board —
   so the list of branches it is a question about stays readable beside it. The
   verdict and `busy` are announced live, because a run can start while the
   window is open and the button has to go dead when it does. */
function openNewBranch(from) {
  newBranchFrom.value = from
  serveDialog('new-branch', {
    /* The repository is captured here and is part of the ground, because
       `createBranch` resolves it from `vcsState.selected` when Create is
       pressed rather than when this window opened. With no scrim to stop them,
       somebody can click another repository row in the panel while the window
       stands — and `main` exists in both, so nothing about the branch name would
       have noticed. The window closes instead, before Create can be pressed. */
    ground: { project: activePath.value, repo: vcsState.selected, branch: from },
    props: () => ({
      title: 'New branch',
      from: newBranchFrom.value,
      branches: vcsState.branches,
      actions: gitWrites.value,
      busy: Boolean(vcsState.busy)
    }),
    forget: () => {
      newBranchFrom.value = null
    },
    onResult: (name, payload) => {
      if (name === 'close') closeDialog('new-branch')
      if (name === 'create') cutBranch(payload)
    }
  })
}

/* The dialog closes first and git runs after, which is the shape of every write
   in this panel: the spinner lands on the row the branch is cut from, and a
   refusal is drawn where the panel draws the rest of git's refusals. A dialog
   held open over that spinner would be a second place saying the same thing. */
const cutBranch = (ask) => {
  closeDialog('new-branch')
  createBranch(ask)
}

/* Which branch the delete-branch window is asking about, and null for closed —
   the branch is the state for `newBranchFrom`'s reason, since what the window
   is about is entirely the row somebody right-clicked.

   Beside it the two fields that make this the one confirm in the app that asks
   twice: whether git has already declined the plain delete because the branch
   holds commits of its own, and git's own words for a refusal forcing would not
   fix. Both are cleared when the window is opened and when it is forgotten, so
   a second delete never opens on the last one's answer. */
const deletingBranch = ref(null)
const deleteBranchNotMerged = ref(false)
const deleteBranchRefusal = ref('')

/* Deleting a branch, in a window of its own rather than a modal over the board.
   The same ground as cutting one and for the same two reasons `openNewBranch`
   records: `deleteBranch` resolves the repository from `vcsState.selected` when
   the button is pressed, and this window is about a branch that exists. */
function openDeleteBranch(branch) {
  deletingBranch.value = branch
  deleteBranchNotMerged.value = false
  deleteBranchRefusal.value = ''
  serveDialog('delete-branch', {
    ground: { project: activePath.value, repo: vcsState.selected, branch },
    props: () => ({
      /* The frame's caption, in `DeleteBranchModal`'s own words — see the
         comment beside its `title`. */
      title: `Delete ${deletingBranch.value}?`,
      branch: deletingBranch.value ?? '',
      notMerged: deleteBranchNotMerged.value,
      refusal: deleteBranchRefusal.value,
      busy: vcsState.busy?.op === 'delete'
    }),
    forget: () => {
      deletingBranch.value = null
      deleteBranchNotMerged.value = false
      deleteBranchRefusal.value = ''
    },
    onResult: (name, payload) => {
      if (name === 'close') closeDialog('delete-branch')
      if (name === 'confirm') removeBranch(Boolean(payload?.force))
    }
  })
}

/* The one write behind a dialog in this view that does **not** close the window
   first, and the exception is the whole feature: git's refusal is the second
   question, so the window that asked the first one has to still be there to ask
   it. `cutBranch` and `deleteTask` close first because nothing they hear back
   changes what the window would say.

   What that costs is the ground, and `reground` above is what pays it: the
   branch is let go of before git is asked, so the successful case closes this
   window from here — quietly — instead of having it pulled out from under the
   person with a notice about a branch they just deleted. */
async function removeBranch(force) {
  const branch = deletingBranch.value
  if (!branch) return
  /* A second press while the first is still out, and it is not a hypothetical:
     `busy` reaches the window's button through an announcement, so the guest's
     `:disabled` is one IPC hop behind the flag. `write()` in the store already
     refuses the second call — but by then this function has nulled the ground
     twice and the second refusal's `else` would hand the branch **back** while
     the first call is still in flight. The refresh that follows the first would
     then find the window standing over a branch that has gone, and close it
     with the very notice `reground` exists to prevent. The guard is the same
     one `write()` keeps, one layer earlier, so nothing here runs twice.

     It does **not** stand in the way of the second question, and that rests on
     an ordering worth naming: `write()` clears `busy` in its `finally`, before
     `deleteBranch` throws and long before the `catch` below sets `notMerged`, so
     by the time `Delete anyway` is on screen there is nothing in flight to
     refuse it. `vcs.test.js` pins that half — a refused delete leaves `busy`
     null. */
  if (vcsState.busy) return
  const standing = { project: activePath.value, repo: vcsState.selected, branch }
  reground('delete-branch', { ...standing, branch: null })
  try {
    const gone = await deleteBranch(branch, { force })
    /* `false` with nothing thrown is git already busy, or the project or the
       repository having moved while the call was out — in every one of which
       the window is either about to be closed by the ground watcher or was
       never going to write anything. Nothing to say and nothing to redraw. */
    if (gone) closeDialog('delete-branch')
    else reground('delete-branch', standing)
  } catch (refused) {
    reground('delete-branch', standing)
    if (refused?.kind === 'notMerged') deleteBranchNotMerged.value = true
    /* Anything else — a branch held by another worktree above all — is a
       refusal `-D` would repeat, so what the window draws is git's own words
       and one way out. The panel behind it draws the same refusal under its own
       title, since `writeError` is set whatever this window does with it. */
    else deleteBranchRefusal.value = refused?.message ?? ''
  }
}

/* Reviewing what a branch changed, in a window of its own.

   **Two doors and one window.** A branch row's menu opens it knowing the name,
   and the table is then every repository of the project that has a branch of
   that name; the `+` menu's `New review` opens it knowing nothing, and the
   table is one row on the repository the Git panel is showing with its checked
   side empty. Picking a name on that lone row calls the very same rule again
   and builds the rest — so the two doors differ in what they start with and in
   nothing else.

   Which repositories a name is in, and which are short of it, is
   `reviewRows.js` over `target_branches`' answer. That command already walks
   every repository `[project].repos` names and says which of them each branch
   is missing from, so nothing here walks a project a second time.

   The ground is the project alone (`dialogRegistry.js` carries why): this
   window is about a set of repositories rather than about the one the panel has
   selected, so a repository going is a row leaving the table rather than a
   window that has lost its subject. */
const reviewTable = ref([])
const reviewBranch = ref('')
const reviewWithout = ref([])
/* What `origin` has, per repository. The store holds one list at a time — it
   was written for a caller that asks about one repository — so the answers are
   copied out of it as they land and kept here, keyed by path. */
const reviewRemote = ref({})
const reviewFetching = ref([])
const reviewFetchFailed = ref([])
const reviewStarting = ref(false)

/* The two terms `pickBranch` takes before it falls back to the top of the list,
   in **one expression** rather than one per caller. Both the rule's own fill and
   a row added by hand read it, and they used to differ by a term: the hand-added
   row borrowed the run dialog's `?? branchLabel`, so on a project with no
   `[defaults].target_branch` and nothing remembered it started at the
   checked-out branch while a rule-built row started at the top of the list.
   `branchLabel` is a fourth term the spec's order does not have, and it can be
   `'—'` or `'abc123 (detached)'` besides. */
const reviewDefaults = () => ({
  remembered: project.runSettings?.targetBranch ?? null,
  configured: runConfig.value?.defaults?.target_branch ?? null
})

/* What a row added by hand starts its reference side at: the run dialog's own
   order, through the run dialog's own rule, because the two windows are asking
   the same question about the same project and a second order would be a second
   answer to it. */
const reviewBase = computed(() => {
  const { remembered, configured } = reviewDefaults()
  return pickBranch(gitState.branches, remembered, configured)
})

/* The table, rebuilt from a name — or from no name at all, which is the
   `New review` door. Called on opening and again whenever somebody picks a
   branch on the checked side of a lone row, which is the one edit the window
   does not make for itself.

   `chosenBase` is what that row already had on its reference side, and it rides
   in front of the remembered branch rather than beside it. The `New review`
   door opens with the base filled and the head empty, and the table's columns
   run Repository, Base, To check — so the ordinary way through this window is
   to set the base and *then* the branch, and a rebuild that ignored the first
   would put the default back over it in the moment somebody's eye was on the
   second. It goes in as `remembered` because that is `pickBranch`'s first term
   and because a name that has since left the list is skipped there already. */
function buildReviewTable(branch, chosenBase = null) {
  const defaults = reviewDefaults()
  const { rows, without } = reviewRows(vcsState.repos, branch, {
    branches: gitState.branches,
    ...defaults,
    remembered: chosenBase || defaults.remembered,
    selected: vcsState.selected
  })
  reviewTable.value = rows
  reviewWithout.value = without
  reviewBranch.value = typeof branch === 'string' ? branch : ''
}

/* What `origin` is known to have in each of this project's repositories.

   One repository at a time and copied the moment each answer lands, because
   `loadRemoteBranches` writes into a single field of the store: it was written
   for a caller looking at one repository, and this window is looking at all of
   them. Sequential rather than parallel for exactly that reason — two calls in
   flight would leave one repository's list under another's name. It costs
   nothing worth parallelising: `vcs_remote_branches` spawns no process at all,
   it reads `refs/remotes/origin/` and `packed-refs` off the disk.

   Nothing waits for it. The window is already up and drawing local branches;
   the origin lists arrive underneath, which is the freshness this whole panel
   promises everywhere else.

   **The counter is what makes the snapshotting safe, and it is not the same
   question as whether the window is open.** `serveDialog` allows a kind that is
   already open to be reopened: it replaces the service and brings the window
   forward, and it stops nothing this function started — so a second opening
   leaves two loops alive, `openDialogs.has(...)` answers true for both, and two
   `vcs_remote_branches` calls are in flight against a store field that holds one
   list. The write from one and the snapshot from the other can then interleave,
   which puts one repository's origin branches under another repository's key.
   A loop belongs to the opening that started it, and this is what says so. */
let reviewRemoteRun = 0

async function loadReviewRemotes(path) {
  const mine = (reviewRemoteRun += 1)
  const answers = {}
  for (const repo of vcsState.repos) {
    if (!repo?.path) continue
    await loadRemoteBranches(repo.path)
    /* A later opening of this window, a project switched under the read, or the
       window closed: none of the three is worth filling a list about. */
    if (mine !== reviewRemoteRun) return
    if (activePath.value !== path || !openDialogs.has('review-changes')) return
    answers[repo.path] = [...vcsState.remoteBranches]
    reviewRemote.value = { ...answers }
  }
}

async function openReviewChanges(branch = null) {
  const path = activePath.value
  if (!path) return
  reviewRemote.value = {}
  reviewFetching.value = []
  reviewFetchFailed.value = []
  reviewStarting.value = false
  /* **Before the table and not after it**, unlike the run dialog's own late
     `loadBranches`. That window fills a field from the list and can do it when
     the list arrives; this one *is* the list — a table built before
     `target_branches` has answered reads as a branch no repository has, which
     is an empty window with a sentence under it saying something untrue. The
     call is git file reads and no process, so the wait is the one the branch
     list already costs everywhere else in this panel. */
  await loadBranches(path)
  if (activePath.value !== path) return
  buildReviewTable(branch)
  serveDialog('review-changes', {
    ground: { project: path },
    props: () => ({
      /* The frame's caption and the dialog's own heading, one literal in both
         places — and a third in `mockBackend.js` for the browser. */
      title: 'Review changes',
      rows: reviewTable.value,
      branch: reviewBranch.value,
      repos: vcsState.repos,
      branches: gitState.branches,
      remote: reviewRemote.value,
      without: reviewWithout.value,
      defaultBase: reviewBase.value,
      fetching: reviewFetching.value,
      fetchFailed: reviewFetchFailed.value,
      busy: reviewStarting.value
    }),
    forget: () => {
      reviewTable.value = []
      reviewWithout.value = []
      reviewBranch.value = ''
      reviewRemote.value = {}
      reviewFetching.value = []
      reviewFetchFailed.value = []
    },
    onResult: (name, payload) => {
      if (name === 'close') closeDialog('review-changes')
      /* The one message from this window that is not somebody finishing with
         it: a branch picked on the checked side of a lone row, with the base
         that row already carried. The rule builds the rest of the table around
         both and it comes back as a prop. */
      if (name === 'branch') buildReviewTable(payload?.name, payload?.base)
      if (name === 'submit') startReview(payload?.rows)
    }
  })
  await loadReviewRemotes(path)
}

/* What a press of Review does, in the order it has to happen in.

   The fetch comes first and is not optional: a side reading `origin/main` is
   only as current as the last fetch, so without one the report would be about a
   commit nobody asked about, and nothing on screen would have said so. **A
   fetch that fails does not call the review off** — what `origin` holds on this
   machine is still readable, merely older — so the repositories that could not
   be reached are named instead, in the window while it is still up and in a
   toast that outlives it.

   The window closes last rather than first, which is the opposite of every
   other write behind a dialog in this view. The reason is the same one that
   keeps `delete-branch` open: what the window is drawing while this runs — the
   fetch, and then which repositories it could not reach — is not said anywhere
   else, so closing first would be closing over the only report of it. */
async function startReview(rows) {
  const path = activePath.value
  const list = Array.isArray(rows) ? rows : []
  if (!path || !list.length || reviewStarting.value) return
  reviewStarting.value = true
  reviewFetchFailed.value = []
  /* The repositories origin could not be reached in, by path, and the one
     answer three readers are drawn from: the sentence under the table, the
     toast that outlives the window, and the intent. It is declared out here
     rather than inside the branch below because the intent is built after it
     — a review where nothing had to be fetched simply carries none. */
  let missed = []
  try {
    const targets = fetchTargets(list)
    if (targets.length) {
      reviewFetching.value = targets
      /* Together rather than one after another, unlike the origin lists above:
         these are network calls of up to a minute each and none of them writes
         anything the next one reads. The store keeps one in flight per
         repository, which is the only sharing there is between them. */
      const reached = await Promise.all(targets.map((repo) => fetchIn(repo)))
      reviewFetching.value = []
      missed = fetchFailures(targets, reached)
      /* The window and the toast speak in names — they are what
         `[project].repos` holds and what the table draws — and the intent
         speaks in paths, because that is how the prompt lists a pair's
         repository. Both are this one list, so there is no arrangement of
         them in which the person and the agent are told about different
         repositories. */
      reviewFetchFailed.value = missed.map(reviewRepoName)
      if (missed.length) {
        sayFileMenu({
          tone: 'info',
          title: 'Origin was not reached everywhere',
          description: `The review reads what origin was last known to have in ${reviewFetchFailed.value.join(', ')}.`
        })
      }
    }
    /* The name of the branch under review rather than the ref it resolved to:
       the path is what a person reads afterwards, and `origin-feature-x` would
       be naming a remote in a filename. */
    const report = reportPath(list[0]?.head, new Date())
    /* Both switches the way `newAgent` sets them — this is the same act by
       another door, and a tab that appears a second later leaves the button a
       person pressed looking as though it did nothing — but **only while this is
       still the project on screen**. `project` is `settings.project`, the active
       project's own section, and the fetch above it is up to a minute of
       network: somebody who moved on during it would have another project's
       tabs flipped by a review they started somewhere else. The session is
       unaffected either way, since `createSession` is given the path this
       started from rather than whatever is selected now. */
    if (activePath.value === path) {
      project.sideTab = 'agents'
      project.activeTab = 'terminal'
    }
    /* `fetchFailed` rides beside the pairs so that the report can say so about
       itself. Without it the sentence lived only on screen and in a toast, and
       somebody who saw neither had nothing to learn it from — an `origin/main`
       a week old reads exactly like one a minute old. */
    await createSession(path, {
      kind: 'reviewBranch',
      pairs: reviewPairs(list),
      report,
      fetchFailed: missed
    })
    closeDialog('review-changes')
  } catch {
    /* Already reported by `createSession`, which sets `terminalState.lastError`
       and draws it as a toast. The window stays open over the failure, the way
       every other dialog behind a failed start does. */
  } finally {
    reviewStarting.value = false
  }
}

/* A repository's name for a sentence, from the path a row carries. The panel
   speaks in names — they are what `[project].repos` holds and what the table
   draws — and a path in a toast would be a line of chrome nobody reads. */
const reviewRepoName = (path) =>
  vcsState.repos.find((repo) => repo.path === path)?.name ?? path

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
/* The commit box's field is in here beside the two sections, and it is a count
   of rows for the same reason they are — it survives a change of density or of
   the app-wide font size, which a pixel height would not. What it does not
   share is `null`: those two mean "never dragged, follow the content" and this
   one has a shipped height instead. */
const ROWS_KEY = { repos: 'reposRows', branches: 'branchRows', commit: 'commitRows' }

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

/* Which branches are pinned above the tree, and this one is under the project
   beside the folders for the same argument: which names are worth keeping in
   reach is a fact about a repository and its naming convention, not a habit of
   reading. What arrives is the whole new list, already resolved by
   `branchTree.js` — the panel is presentational on this as on the folds. */
const setFavoriteBranches = (favorites) => {
  project.favoriteBranches = favorites
}

/* The second door out of a conflict, and the one this view has to carry: the
   store cannot open a tab or move a side tab, and everything else about the
   conflict is already in `vcsState`.

   The dialog is closed first and the tree is left exactly as git left it —
   that is the whole of what this door does to the repository. **Closed and not
   forgotten**: the record stays, so the panel goes on drawing `Resolve
   conflicts` for as long as the tree is conflicted, which is the way back in
   for a session that stopped early or resolved the wrong way. Then it is the
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
      /* An unknown branch crosses as the empty string, the way `ours` above
         already does: `Intent::ResolveConflict` takes two `String`s, and
         `prompt.rs::resolve_conflict` has an arm for each of them being empty.
         A rebase this app did not start has no `theirs` at all — the branch it
         is going onto is readable nowhere a git process can see. */
      theirs: conflict.theirs ?? '',
      files: conflict.files
    })
  } catch {
    // already reported — see newAgent above
  }
}

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
  /* A window of its own rather than a modal over the board, so the card, the
     column or the queue this run is aimed at stays readable beside the question
     about it. Everything below is announced live: the branch list lands after
     the window is up (see the note above), the machine's answer about a browser
     lands after that, and `busy` and the refusal arrive while the run is
     starting — a window that had been handed its props once would show none of
     it.

     The ground is the project and nothing else, deliberately. A run aimed at a
     task whose card somebody deletes meanwhile is a question about an id bd no
     longer holds, and the worker refuses it in the same words it would use for
     any other missing task — which is a better answer than a window vanishing
     mid-sentence. The project moving is different in kind: every id on screen
     belongs to another tracker after it. */
  serveDialog('run', {
    ground: { project: activePath.value },
    props: () => ({
      /* The frame's caption and the dialog's own heading, from one rule in
         `runScopes.js`. `rescope` changes the scope under an open window, so
         this is not a string settled at opening: the frame has to follow the
         heading from "Run this task" to "Run these tasks". */
      title: runTitle(runScope.value),
      scope: runScope.value,
      count: runCount.value,
      partOf: runParent.value,
      branches: gitState.branches,
      defaultBranch: runConfig.value?.defaults?.target_branch ?? branchLabel.value,
      defaultPriority: runConfig.value?.defaults?.min_priority ?? DEFAULTS_FALLBACK.min_priority,
      defaultParallel:
        runConfig.value?.defaults?.max_parallel_tasks ?? DEFAULTS_FALLBACK.max_parallel_tasks,
      remembered: project.runSettings,
      liveCheckAvailable: runConfig.value?.live_check?.mode !== 'none',
      liveCheckBlocked: liveCheckBlocked.value,
      configError: configErrorText.value,
      error: runError.value,
      busy: runStarting.value
    }),
    onResult: (name, payload) => {
      if (name === 'close') closeDialog('run')
      if (name === 'confirm') startTheRun(payload)
      if (name === 'rescope') runTheEpicInstead()
    }
  })
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
    closeDialog('run')
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
  /* The path this is about is the ground, and it is the active project on both
     routes in: adding a project makes it active before this is reached, and the
     row menu's setup item is dead on any row that is not the active one
     (`shell/projectMenu.js`). Written as the path rather than as `activePath`
     so that a third route opening this over another project fails loudly — the
     window would close on the spot — instead of quietly setting up the wrong
     folder. */
  serveDialog('setup-project', {
    ground: { project: path },
    props: () => ({
      /* The frame's caption, in `SetupProjectModal`'s own words. It owns
         them; this repeats them because the frame is drawn by the OS and
         nothing on the window's side knows what this dialog is called. */
      title: setupExisting.value ? 'Set this project up again?' : 'Set this project up?',
      name: setupFor.value ? basenameOf(setupFor.value) : '',
      existing: setupExisting.value,
      busy: settingUp.value
    }),
    /* Both fields, because the comment over `setupExisting` promises they do
       not drift and a reader will trust it. Nothing today opens this without
       setting both — that is what the promise rests on — and a half-cleared
       pair is exactly the state that would quietly break it. */
    forget: () => {
      setupFor.value = null
      setupExisting.value = false
    },
    onResult: (name) => {
      if (name === 'close') closeSetup()
      if (name === 'confirm') startSetup()
    }
  })
}

const closeSetup = () => {
  closeDialog('setup-project')
}

/* The other window about `.smetana/project.toml`, and the one that changes it
   without starting anything: `[defaults]`, four scalars, edited in a form.
   Modelled on `openSetup` above down to holding the path this is about rather
   than reading `activePath` at render time — a window that outlived a project
   switch must fail loudly instead of quietly writing four numbers into another
   repository.

   The two are refused in different states and that is the whole reason there
   are two: the setup runs over a damaged file, this one cannot, because there
   are no parsed values to put in its fields. `shell/projectMenu.js` carries
   that rule and the captions that explain it. */
const settingsFor = ref(null)
const savingSettings = ref(false)
const settingsError = ref('')

const openProjectSettings = async (path) => {
  settingsFor.value = path
  settingsError.value = ''
  serveDialog('project-settings', {
    ground: { project: path },
    props: () => ({
      /* The frame's caption, in the component's own words, for the reason
         `openSetup` above repeats its dialog's: the OS draws the frame, and
         nothing on the window's side knows what this dialog is called. */
      title: 'Project settings',
      /* Announced on every change, like everything else here, but read **once**
         at the other end: `ProjectSettingsModal` seeds its draft in a watcher on
         `open`, and `DialogWindow.vue` holds `open` true for the life of the
         window, so what this value is at the moment the window mounts is what
         the fields get and a later announcement never reaches them. That is
         what stops a prop arriving mid-edit from taking away what somebody is
         typing, and it is safe here because the menu item is greyed until
         `configured` — which is exactly when `runConfig` is non-null. */
      defaults: draftFrom(runConfig.value),
      branches: gitState.branches,
      busy: savingSettings.value,
      error: settingsError.value
    }),
    forget: () => {
      settingsFor.value = null
      settingsError.value = ''
    },
    onResult: (name, payload) => {
      if (name === 'close') closeDialog('project-settings')
      if (name === 'save') saveProjectSettings(payload)
    }
  })
  /* After the dialog is up, for the run dialog's reason: the list is a git read
     per repository and nobody needs it until they are looking at the field. The
     same `loadBranches` and the same `gitState.branches` the run dialog's own
     branch field is filled from — asking twice for one list would be two
     sources to keep in step. */
  await loadBranches(path)
}

const saveProjectSettings = async (draft) => {
  const project = settingsFor.value
  if (!project || savingSettings.value) return
  savingSettings.value = true
  settingsError.value = ''
  try {
    /* An empty branch is no branch: the file's `target_branch` is an
       `Option<String>`, and `Select` has no way to hand back `null`. Sending
       `""` would write a branch name of length zero. */
    await saveDefaults(project, { ...draft, target_branch: draft.target_branch || null })
    closeDialog('project-settings')
  } catch (err) {
    /* Shown in the window rather than swallowed: both refusals the command has
       — no file, and a file that will not parse — are sentences the person has
       to read to know what to do next. */
    settingsError.value = String(err?.message ?? err)
  } finally {
    savingSettings.value = false
  }
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

   - an edit, answering a parked task's questions, and fixing work behind a
     closed one all open their issue on the board's own selection, which is what
     highlights the card: the panel and the board are one selection, so these
     are the kinds that write `project.selectedTask`;
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
  if (work?.kind === 'editTask' || work?.kind === 'resolveTask' || work?.kind === 'fixTask') {
    project.selectedTask = work.id
    rightFocus.value = null
    /* Written here rather than left to the watch on `rightPanel`, and this is
       the one branch that has to: it opens an issue on the board's own
       selection, so `rightPanel` never leaves `'board'` and the watch has
       nothing to fire on. The filing and claimed branch below moves the panel
       by moving `rightPanel`, and without this line two rows of one session
       list would answer the same click oppositely — the row above showing its
       work at once, this one showing nothing at all while quietly changing a
       remembered preference under somebody standing on Sessions. */
    project.rightTab = 'task'
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
  /* And the machine's clipboard, which is written to from outside this window
     more often than anything else here: copying a file in Finder means leaving
     this app and coming back, and coming back is this event. Not awaited, like
     its neighbours — the tree's Paste row un-greys when the answer lands — and
     it cannot fail in a way anybody sees (`stores/files.js`). */
  readSystemClipboard()

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

/* What is left of the agent's subscription, for the strip along the bottom of
   this window.

   The timer and the last answer live here rather than in a store of their own:
   `app.js` deliberately holds no state, and this reading belongs to one window
   and does not outlive it — which is that store's own argument about where the
   question goes, applied to where the answer sits.

   Ten minutes is `runs::usage::POLL`, chosen there because a session limit
   resets in hours and a weekly one in days, so asking oftener only spends the
   machine. There is no re-read when the window regains focus: a laptop that
   slept shows a stale figure for up to one interval, and the timer is the whole
   policy.

   **The agent is named in the question** rather than left to Rust to read out
   of `settings.json`, exactly as the settings window names it: this window owns
   that field and the file is a debounce behind it, so a read that let Rust
   consult the file could be answered about the agent somebody has just left.

   A probe is somebody else's CLI with a sixty-second ceiling over it, so a tick
   that lands while one is still out does nothing, and so does a second press on
   the strip. The numbers already on screen stay where they are meanwhile —
   unlike the settings block, which clears its rows before every read because a
   block sitting there showing the previous answer would be claiming a reading
   that is being replaced as it is read. The strip never labels its figure
   fresh, so it claims nothing by keeping it, and blanking a permanent strip
   every ten minutes is a flicker in the corner of somebody's eye.

   A refusal clears the reading and is kept beside it: `invoke` failing is the
   channel rather than an answer, so two dashes is all the strip can show — but
   the hint has to say the reading failed rather than that nobody has asked yet,
   which is the sentence an empty reading alone would draw over an attempt that
   happened. */
const USAGE_EVERY_MS = 10 * 60 * 1000
const usageReading = ref(null)
const usageBusy = ref(false)
const usageError = ref(null)
let usagePoll = null

const readUsage = async () => {
  if (usageBusy.value) return
  usageBusy.value = true
  usageError.value = null
  try {
    usageReading.value = await readAgentUsage(settings.agent)
  } catch (err) {
    usageReading.value = null
    usageError.value = err.message
  } finally {
    usageBusy.value = false
  }
}

onMounted(() => {
  readUsage()
  usagePoll = setInterval(readUsage, USAGE_EVERY_MS)
})
onUnmounted(() => {
  if (usagePoll) clearInterval(usagePoll)
  usagePoll = null
})

/* Whoever would answer has changed, so whatever is on the strip is about
   somebody else. The settings window is where that edit is made, and it reaches
   this window through the bridge above. */
watch(() => settings.agent, () => readUsage())

const initing = ref(false)
const initHere = async () => {
  initing.value = true
  try {
    await initActive()
  } finally {
    initing.value = false
  }
}

/* The deterministic door out of a failing tracker, and the same shape `initing`
   above has: the empty state stays where it is and the button says what is
   happening, because the sentence explaining the situation is the thing a
   skeleton would replace.

   The refusal is swallowed here and not rethrown, exactly as `initActive`
   swallows `initBd`'s: the store has already put it in `trackerState.lastError`
   for the toast, and health is still `error`, so the screen keeps the whole
   notice with both buttons on it. Nothing removes the copy the repair took, on
   either outcome — a migration is the one irreversible thing this app does to
   somebody's tracker, and the way back has to outlive the attempt.

   Success says where the copy is, and that is not a courtesy. The board comes
   back and takes the whole notice with it, so without this the one durable
   thing the press left behind — a directory beside `.beads`, holding a copy of
   a database, that nothing in this app will ever remove — would be something a
   person next met while wondering what it was. The toast is held until it is
   dismissed rather than timed out like the file tree's: where a copy of one's
   tracker went is not a fact that has finished being useful after three
   seconds.

   **The name, not the path.** A `Toast` is 320px with an icon and a dismiss
   button beside its text, and nothing in it or in `tokens/base.css` sets
   `overflow-wrap` — so an absolute path is one unbreakable ~70-character word
   that runs past the toast's own border, and the path is the entire reason
   this toast exists. `.beads.backup-<UTC>` beside `.beads` locates the copy
   completely, because it is always beside the `.beads` of the project that was
   just repaired, and it breaks nowhere it has to. */
const repairing = ref(false)
const repairNote = ref(null)
const repairHere = async () => {
  repairing.value = true
  try {
    const { backup } = await repairTracker()
    repairNote.value = {
      title: 'Tracker repaired',
      description: `A copy of .beads was taken first, as ${basename(backup)} beside it. Nothing removes it.`
    }
  } catch {
    /* the message already sits in trackerState.lastError */
  } finally {
    repairing.value = false
  }
}

/* The other door, for the failure nothing here can classify — and there is
   deliberately no classifier: bd offers no structured verdict about its own
   database, so the app repairs without diagnosing and hands over the whole of
   the failure when that was not enough.

   The briefing is fetched at the moment of the press rather than assembled from
   what the store happens to hold, and that is the point: the tracker is what is
   broken, so the agent cannot ask it anything afterwards, and one answer from
   the worker cannot describe two different moments the way two reads could. */
const askAgentAboutTracker = async () => {
  const path = activePath.value
  if (!path) return
  project.sideTab = 'agents'
  project.activeTab = 'terminal'
  try {
    const failure = await trackerFailure()
    await createSession(path, {
      kind: 'repairTracker',
      dir: failure.dir || path,
      bdVersion: failure.bdVersion,
      command: failure.command,
      stderr: failure.stderr
    })
  } catch {
    /* Both awaits report their own refusal before rejecting — `createSession`
       into `terminalState.lastError`, `trackerFailure` into
       `trackerState.lastError` — and both draw as a toast. That second half is
       why the store wraps its `invoke` rather than handing the promise
       straight over: the tabs have already moved by the time either is
       awaited, so a silent rejection is a button that takes somebody somewhere
       else and then does nothing. */
  }
}

/* The third door under the board, and the one that is not about bd at all: the
   operating system has a stored refusal for this folder and nothing else will
   get past it. `tccutil reset` makes macOS forget the decision; only a process
   that has not already been refused in this launch is asked again, so Rust
   restarts the app itself once the reset lands.

   Which means success never comes back here — the window is on its way out
   before the promise could settle — and the flag is therefore cleared in the
   catch rather than in a `finally`. A `finally` would be the tidier shape and
   the wrong one: it would put the button back to "Reset and restart" in the
   last moments of a window that is closing, which reads as nothing having
   happened.

   The likeliest refusal is not a failure at all but the run gate: a restart
   kills every PTY child, so Rust refuses while any run is going anywhere, and
   that run is by definition in another project — this one's folder cannot be
   read, so nothing here is running. The store puts Rust's sentence, which names
   the projects, in front of the caption.

   No confirmation dialog, like the repair beside it. What stands in for one is
   the notice above the button, which says the app restarts. */
const resettingAccess = ref(false)
const resetAccessHere = async () => {
  resettingAccess.value = true
  try {
    await resetFolderAccess()
  } catch {
    /* The caption is already in trackerState.lastError and draws as a toast.
       The app is still standing, so the button has to come back. */
    resettingAccess.value = false
  }
}

/* bd gives a new task the one status it has for them — open, which the board
   calls ready. So that column, and only it, carries the "+": a plus over any
   other column would promise a placement the tracker cannot make. */
const ADD_TO = 'ready'
/* The issue the New task dialog was opened from, or null when it was opened
   from "+ New task". `{ id, title }` rather than the issue, and taken from the
   store at the moment the menu was used: the dialog draws the title, and the id
   is what rides to the agent.

   There is no `newTaskOpen` beside it any more: whether the dialog is open is
   whether its window is being served, which `openDialogs` already knows, and a
   flag kept beside that is two halves of one dialog free to disagree. */
const followUpParent = ref(null)
const creating = ref(false)

/* Filing a task, in a window of its own. Three doors reach it — the "+" over
   the ready column, the tab bar's menu, and a card's own "follow-up" — and the
   parent is this function's argument rather than something a caller sets first.
   That is not style: `serveDialog` replaces an open service before it starts
   the new one, so anything the caller had just written into a ref that the old
   service forgets on the way out would be wiped between the two.

   The images are not announced and do not come back one by one: the store that
   holds them lives in that window, because a file dropped on it is that
   window's event and nothing here can hear it (`stores/attachments.js`). What
   this window is handed, in `submit`, is the list of paths.

   The ground carries the column as well as the project. There is exactly one —
   bd files a new task as open, which the board draws as ready — so a board that
   no longer has it is a board this dialog could not place a card on. */
const openNewTask = (parent = null) => {
  followUpParent.value = parent
  serveDialog('new-task', {
    ground: { project: activePath.value, column: ADD_TO },
    props: () => ({
      title: 'New task',
      busy: creating.value,
      status: ADD_TO,
      parent: followUpParent.value
    }),
    /* The one moment this window can be sure the attachment folder may have
       grown: the window that writes into it has just stopped being served. It
       used to be a watcher over the store's own list, which this window no
       longer holds — and `forget` covers every way out, the frame's own cross
       and a project switch included. */
    forget: () => measureStorage(activePath.value),
    onResult: (name, payload) => {
      if (name === 'close') closeNewTask()
      if (name === 'submit') submitNewTask(payload)
    }
  })
}

/* Where the whole-column press stands. bd's own word, untranslated, because
   `deferred` is not one of the three statuses the tracker store renames and it
   reaches the board exactly as bd spells it.

   That column is where a run files its own findings, and the running-tasks
   skill reserves promoting one of them for a person. This button is the person
   doing it, in one gesture instead of twelve — which is why it moves issues and
   starts nothing: a run still takes only what is already open. */
const PROMOTE_FROM = 'deferred'
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
  /* A window of its own, and this is the one of the four where that is worth
     more than the room: the count climbs inside it for most of a minute while
     the board fills in behind, and with no scrim over the board both are
     readable at once.

     The live ground is the project, and effectively only the project. The
     column rides along because that is what this dialog is about, but the
     clause behind it can only fire if bd's own status set stops holding
     `deferred` — `boardColumns` seeds a bucket per declared column, so promoting
     every last card out of it empties the column without removing it, and that
     is deliberate: a window closing at the moment its work finished would read
     as the work having failed. What could genuinely go out from under this
     window is the set of ids it took, and the ground vocabulary has no word for
     that (`views/dialogRegistry.js`). It is not a hole worth filling here: an id
     that vanishes mid-run is one failed write among the others, counted and
     reported below like the rest. */
  serveDialog('promote-column', {
    ground: { project: activePath.value, column: PROMOTE_FROM },
    props: () => ({
      /* The frame's caption. The rule is `kanban/promoteTitle.js`, which the
         dialog itself draws its heading from: nothing on the window's side of
         the wire knows what a dialog is called, so the sentence has to be said
         from here too, and a pluralisation written out twice is the half that
         would go quietly wrong. */
      title: promoteTitle({
        count: promoteIds.value.length,
        moved: promoted.value,
        failed: promoteFailed.value
      }),
      count: promoteIds.value.length,
      busy: promoting.value,
      moved: promoted.value,
      failed: promoteFailed.value
    }),
    forget: () => {
      promoteIds.value = []
      promoted.value = 0
      promoteFailed.value = null
    },
    onResult: (name) => {
      if (name === 'close') closePromote()
      if (name === 'confirm') confirmPromote()
    }
  })
}

/* Unconditional, where the modal refused while writing. The window's own frame
   carries a cross the app cannot disable, and by the time this is reached the
   window is already gone — refusing here would leave this view serving props to
   nothing. The writes already asked for run on to the end regardless; nothing
   was ever rolled back. */
const closePromote = () => {
  closeDialog('promote-column')
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
  const total = promoteIds.value.length
  promoting.value = true
  promoted.value = 0
  promoteFailed.value = null
  /* Counted here as well as in the two refs beside them, and that is not
     duplication for its own sake: this window's frame carries a cross the app
     cannot disable, so somebody can close it while the loop runs, and closing
     forgets the refs. The writes go on regardless — the `for…of` below holds
     the array it started with — so the report has to be kept somewhere nothing
     can clear. */
  let moved = 0
  let failed = 0
  try {
    for (const id of promoteIds.value) {
      try {
        await updateIssue(id, { status: 'open' })
        moved += 1
        promoted.value = moved
      } catch {
        // the message already sits in trackerState.lastError; the count is what
        // this dialog adds to it
        failed += 1
      }
      if (activePath.value !== path) {
        closeDialog('promote-column')
        return
      }
    }
    /* The outcome, said in whichever of the two places is still there to say it
       in. While the window stands, it says it itself: `failed` turns its title
       into a report and its footer into one button. If the person closed it
       mid-write, the app window says it instead — dropping it would leave a
       promote of twelve with three refusals saying nothing at all about the
       three, since `trackerState.lastError` keeps only the newest of them and
       none of them carries a count. The count is the whole of what this dialog
       is for, which is why the modal used to refuse to close over it. */
    if (openDialogs.has('promote-column')) {
      promoteFailed.value = failed
      if (!failed) closeDialog('promote-column')
      return
    }
    sayFileMenu({
      tone: failed ? 'error' : 'success',
      title: promoteTitle({ count: total, moved, failed }),
      description: failed
        ? `${taskCount(failed)} could not be moved. The board shows the ones that did — nothing was rolled back.`
        : 'They are in ready. The window was closed while they were still moving.'
    })
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
   afterwards, with no way back to the board but another agent.

   It takes the column back from the Sessions tab for the same reason: somebody
   standing on Sessions would otherwise press a card and see nothing move, the
   panel answering them one tab away. One of the three places that move that tab
   — the branch in `selectAgent` above and the watch on `rightPanel` below are
   the others — and the reason all three are spelled out separately rather than
   collapsed into one watch on `selectedTask` is that `loadProjectLayout` writes
   that field too, when a project is opened, in the same tick as the `rightTab`
   beside it. Such a watch would restore a remembered Sessions and overwrite it
   a microtask later with Task, and the tab would never survive a restart. */
const selectFromBoard = (id) => {
  project.selectedTask = id
  rightFocus.value = null
  project.rightTab = 'task'
}

/* Copying a task's id, for the card on the board and for the inspector's
   header both. It lives here rather than in either component because exactly
   one file under `src/components/` imports a store, and it is `TerminalView`:
   the two of them raise `copy-id` and take back a word for their tooltip, and
   neither knows a clipboard exists.

   The confirmation is that tooltip rather than a toast, and this is the one
   place in the app where a copy is answered without `sayFileMenu`. A toast
   goes to the corner of the screen; what is being confirmed here is a click
   somebody is still looking at, on a card among dozens of identical-looking
   ones, and the answer belongs on the id itself. A refusal goes the same way,
   so this feature has one channel rather than two.

   One id at a time, deliberately: the state is a single id and a single
   outcome, so copying a second one takes the confirmation off the first — two
   cards both reading `Copied` would be a claim about a clipboard that only
   holds one thing.

   All of which is `useCopyFeedback` now and not written out here: how long the
   confirmation stands, which press owns it when two race, and the second clear
   that keeps a stranded timer from putting out the later one. That policy had
   been written out four times over — twice here and twice in the gallery — and
   had already cost once, when the stranded timer sat in both copies of it and
   had to be found and fixed in each. Two lines of wiring is what is left: which
   id is being claimed, and what text goes on the clipboard. */
const {
  target: copiedTaskId,
  state: taskIdCopyState,
  /* What each of the two components gets: its own outcome, and nothing for
     anybody else's id. */
  stateFor: copyStateFor,
  copy: copyIdFeedback
} = useCopyFeedback(copyText)

/* The id is both the thing claimed and the thing written — a card is told apart
   by exactly what it puts on the clipboard, which is what makes this the
   simpler of this window's two callers. */
const copyTaskId = (id) => copyIdFeedback(id, id)

/* ---- the Sessions tab's cards, and the menu on one ----------------------- */

/* Which cards are open, by session id. A list rather than a `Set` for one
   reason: this is read in the template once per row, and a `Set` behind a
   `ref` is a proxy whose `has` is not tracked the way an array's `includes`
   is — the same trap `expanded` in the file tree avoids by being an array in
   `settings.json`.

   Several at once, which is the design: comparing two sessions is what
   somebody opens a second card for. And nothing about it is written down —
   `settings.json` is where the things that survive a restart live, and opening
   a card is a gesture inside one look at a list. */
const expandedSessions = ref([])

const isSessionOpen = (id) => expandedSessions.value.includes(id)

const toggleSession = (id) => {
  const at = expandedSessions.value.indexOf(id)
  if (at >= 0) expandedSessions.value.splice(at, 1)
  else expandedSessions.value.push(id)
}

/* A project switch closes everything. The ids of another project's sessions
   would never match a row again, so nothing would be drawn open — but the list
   would go on growing for as long as the window is up, and a session id is a
   UUID: this is the one place that would quietly keep every card anybody had
   ever opened. */
watch(() => sessionsState.project, () => {
  expandedSessions.value = []
})

/* Copying one of the three things a session row offers — its resume command,
   its id, the path to its transcript — and saying so afterwards.

   The same policy as the id above, which is now the same code as the id above,
   and for its own stated reason: a copy is the one action with nothing on
   screen to show for it, so the answer belongs on the control somebody is
   still looking at rather than in the corner of the screen. What differs is
   where it lands.
   A task's id is a control of its own; a session's copy is picked from a menu
   that closes on the way out, so the trigger the menu hung from is what is left
   to answer on — it draws a tick and names what was copied, then goes back to
   being a menu button.

   One session at a time, deliberately, exactly as one id at a time up there:
   two rows both claiming to have been copied would be a claim about a clipboard
   that holds one thing. */
const {
  stateFor: sessionCopyStateFor,
  /* Which of the three it was, for the sentence. */
  nounFor: sessionCopyNounFor,
  copy: sessionCopyFeedback
} = useCopyFeedback(copyText)

/* Three verbs into one call: which row is claimed, what the verb puts on the
   clipboard — `''` when there is nothing, which the composable answers as a
   refusal rather than as an emptied clipboard — and what to call it in the
   sentence. */
const copyFromSession = (kind, session) =>
  sessionCopyFeedback(session?.id ?? null, copyPayload(kind, session), copyVerbNoun(kind))

/* Which transcript is being deleted, by path — the field the row's menu is
   greyed from. A path and not a boolean, so a second row's menu is live while
   the first one's is not. */
const deletingSessionPath = ref(null)

/* And which one a person is being asked about, or null. The whole record rather
   than an id, unlike `confirmingDelete` for a task, and the difference is where
   the truth lives: a task is in `trackerState` and can be read again by id at
   the moment the dialog is announced, while the sessions list is read off disk
   when the tab is opened and never watched — so the row the menu was opened
   over is the whole of what this app knows about that session. */
const confirmingSession = ref(null)

/* The confirmation, in a window of its own. Its ground is the project alone;
   `dialogRegistry.js` records why there is no sort of ground for a session and
   why that is right rather than missing. */
const openDeleteSession = (session) => {
  confirmingSession.value = session
  serveDialog('delete-session', {
    ground: { project: activePath.value },
    props: () => ({
      /* The frame's caption, and the same one copy the component's own heading
         calls — see `DELETE_SESSION_TITLE`. */
      title: DELETE_SESSION_TITLE,
      session: confirmingSession.value,
      /* Announced because it is the component's prop, not because it is ever
         seen here: the delete closes this window before it starts, for the
         reason written over `deleteSession`. The state itself is looked at in
         `?view=gallery`. */
      busy: Boolean(deletingSessionPath.value)
    }),
    forget: () => {
      confirmingSession.value = null
    },
    onResult: (name) => {
      if (name === 'close') closeDialog('delete-session')
      if (name === 'confirm') deleteSession(confirmingSession.value)
    }
  })
}

/* The dialog closes first and the file goes after, which is the shape every
   write behind a dialog in this view keeps (`cutBranch` records it). Here it
   costs nothing to observe: the answer arrives in a few milliseconds, the row
   leaves the list on its own, and a refusal has a toast of its own — which is
   the thing that would otherwise be hidden behind a window somebody is still
   looking at.

   The row's own menu is greyed while it runs, which is what `deletingSessionPath`
   is for. It is a short window and it is not decoration: pressing Delete twice
   would put the second press against a file the first one has already taken. */
async function deleteSession(session) {
  const path = session?.path
  closeDialog('delete-session')
  if (!path) return
  deletingSessionPath.value = path
  try {
    const failure = await deleteSessionTranscript(path)
    if (failure) {
      sayFileMenu({ tone: 'error', title: 'Nothing was deleted', description: failure })
      return
    }
    /* The store has taken the row out. Its card goes with it, or the id would
       sit in that list for the life of the window naming nothing. */
    const at = expandedSessions.value.indexOf(session.id)
    if (at >= 0) expandedSessions.value.splice(at, 1)
    sayFileMenu({
      tone: 'success',
      title: 'The transcript was deleted',
      description: path
    })
  } finally {
    deletingSessionPath.value = null
  }
}

/* A session read off disk, brought back as a live agent.

   **The same road every other agent in this app takes**, and that is the whole
   design of it rather than a detail: `createSession` with an intent, which is
   `terminal_create`, which is a profile's own command line plus `--resume <id>`
   and `Pty::spawn`. A second way to start an agent is the place two ways
   silently diverge. What the session gets that others do not is the directory
   it is resumed in — the one its transcript recorded, which for a worktree
   session is a path under `.worktrees/` and is never quietly replaced by the
   project root.

   `fork` is the whole of the difference between the Sessions tab's two
   launching verbs, and it rides in the intent rather than forking this
   function: Resume in worktree carries on writing into the transcript it
   opened, Continue in a new session leaves that file exactly as it was and
   starts a second one beside it from the same history. Everything else — the
   directory, the id, the guard, the row that appears — is one path, because two
   would be the place two paths silently diverge. The second card that turns up
   in this tab afterwards is that fork's own transcript and an expected outcome
   rather than a duplicate.

   The two lines before the await are `newAgent`'s, for its stated reason: a
   spawn takes about a second, and a person who pressed this must see the row
   they asked for rather than nothing at all. What is deliberately *not* here is
   the third line — `project.rightTab` stays where it is. Somebody standing in
   the Sessions tab is standing there on purpose, possibly to bring up a second
   one, and a resume that swung the column onto Task would be the app deciding
   what they came for. The row does appear in the left column and the terminal
   comes forward in the centre, which is where a person who pressed this is
   looking.

   The availability is asked again here even though the row that raised this is
   already greyed, and it is not belt and braces about the menu: the card's
   button and the menu row are two doors onto one verb, and the list they are
   drawn from was read when the tab was opened. What this cannot catch — a
   worktree removed in the meantime — is refused by the worker itself, which is
   the guard standing next to the spawn.

   The catch swallows the rejection for `newAgent`'s reason: `createSession` has
   already reported it, and this exists only to stop Vue repeating what the
   store said. */
async function resumeSession(session, { fork = false } = {}) {
  const path = activePath.value
  if (!path) return
  if (!resumeAvailability(session, { agent: settings.agent, fork }).available) return
  try {
    project.sideTab = 'agents'
    project.activeTab = 'terminal'
    await createSession(path, {
      kind: 'resumeSession',
      id: session.id,
      cwd: session.cwd,
      /* Absence travels as absence: a transcript nobody typed in has no title,
         and the row says what it is rather than inventing a name for it. */
      title: session.title ?? null,
      fork
    })
  } catch {
    // already reported — see comment above
  }
}

/* The session menu's verbs: which one does what. The rows themselves are
   `components/agent/sessionMenu.js`'s, and the pair is joined by hand — a
   `kind` renamed on one side draws perfectly and does nothing at all when
   pressed, the same seam `fileMenu.js`/`onFileAction` has. The test pins the
   producing side.

   Every failure says so. That is the whole of one acceptance criterion and it
   is not a formality: this list is read when the tab is opened and never
   watched, so a transcript deleted from somewhere else — or a worktree removed
   after the session that ran in it — leaves a row whose every verb is about a
   file that has gone, and a menu item that did nothing and said nothing would
   read as a broken app rather than as a stale row. */
const onSessionAction = async ({ kind, session }) => {
  /* Three verbs and one shape: a sentence from the worker, or null. Written
     here rather than three times over so that the rows below stay a list of
     what each verb is, which is the half a person reads this chain for. The
     two launching verbs are not among them — they start a session rather than
     reaching a file, and a failed spawn is already a toast of the store's. */
  const say = (failure, title) => {
    if (failure) sayFileMenu({ tone: 'error', title, description: failure })
  }
  if (isCopyKind(kind)) {
    await copyFromSession(kind, session)
  } else if (kind === RESUME_KIND || kind === FORK_KIND) {
    await resumeSession(session, { fork: kind === FORK_KIND })
  } else if (kind === 'open-log') {
    say(await openSessionLog(session?.path), 'Could not open the log')
  } else if (kind === 'open-cwd') {
    say(await openSessionDirectory(session?.cwd), 'Could not open the working directory')
  } else if (kind === 'reveal-log') {
    /* Deliberately **not** `revealInFileManager`, which every other reveal in
       this window uses. That one is the file tree's: it answers a boolean, so
       the only sentence its callers have for a refusal is the one about a
       browser having no file manager — and in the built app the commonest way
       this fails is a transcript that has gone since the list was read, which
       that sentence would have described as a missing desktop app. The store's
       own verb answers with words. */
    say(await revealSessionLog(session?.path), 'Could not show the log')
  } else if (kind === 'delete') {
    openDeleteSession(session)
  }
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

/* The third place that moves the tab, and the only one of the three that is a
   watch: a draft, or a run's claimed list, arriving in this column has to be
   seen, and somebody standing on Sessions would be looking at the wrong tab for
   it. A watch works here because this is the one case that *does* move
   `rightPanel` — the two branches that open an issue on the board's own
   selection leave it reading `'board'`, which is why they write the tab
   themselves.

   Safe as a watch where `selectedTask` is not, and for a plain reason: nothing
   this reads comes off the disk. `rightPanel` is derived from live sessions,
   which do not survive a restart, so opening a project cannot move it and the
   remembered tab is left alone.

   One direction only. Nothing anywhere switches somebody **to** Sessions: an
   agent that needs an answer already has the bell, the status footer and the
   left column, and taking the panel out from under a person reading a task is
   not on that list. */
watch(rightPanel, (panel) => {
  if (panel !== 'board') project.rightTab = 'task'
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

/* Closing covers both cases that should: cancelling, and a session that
   actually started. A failed create does not reach here, so nobody has to type
   their four sentences again because the agent was not installed — the window
   stays up with the text and the thumbnails still in it.

   Nothing is cleared from this side any more. The text and the images were the
   window's own state and go when the window does; the paths outlive both, since
   the files stay in the app's data directory whether the task was filed or not.
   `openNewTask` above is what makes sure the next one is never silently a
   follow-up to whatever a menu was last opened on. */
const closeNewTask = () => closeDialog('new-task')

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

/* Which issue bd is being asked to delete, or null. An id and not a bare
   boolean, matching `writingId` above: the selection can move while bd is still
   working, and a flag shared between issues would answer for the wrong one.

   It is read in `orderedColumns` beside `writingId` — one rule about a write in
   flight on a card — and announced to the dialog as `busy`. Neither is visible
   for a delete, and that is a fact about `deleteIssue` rather than about this
   line: it removes the card in the same synchronous block that sets this, so
   there is no card left to grey and no window left to tell. See `deleteTask`
   below, which spends a paragraph on it. */
const deletingId = ref(null)

/* Which issue's deletion is being confirmed, or null. An id rather than a
   boolean for the same reason: the dialog names the issue, and the board can
   change under it. */
const confirmingDelete = ref(null)
const confirmedIssue = computed(() =>
  confirmingDelete.value ? issueById(confirmingDelete.value) : null
)

/* The confirm, in a window of its own. Its ground is the issue, which is the
   whole point of standing this one outside the app window: with no scrim, bd
   can delete the task from a terminal while somebody is reading about deleting
   it, and the window goes with it rather than offering a button that would now
   fail. */
const openDeleteTask = (id) => {
  confirmingDelete.value = id
  serveDialog('delete-task', {
    ground: { project: activePath.value, issue: id },
    props: () => ({
      /* The frame's caption, in `DeleteTaskModal`'s own words — see the
         comment beside its `title`. */
      title: `Delete ${confirmingDelete.value}?`,
      id: confirmingDelete.value ?? '',
      /* Read from the store by id at the moment it is announced, never
         carried in from the menu that asked: the store holds the current
         title and a card's copy may be a delta behind. */
      taskTitle: confirmedIssue.value?.title ?? '',
      /* Announced because it is the component's prop and this is a mirror of
         what the app holds, not because it is ever seen here: `deleteTask`
         closes this window before it sets the flag, for the reason written
         over it. The state itself is looked at in `?view=gallery`. */
      busy: Boolean(deletingId.value)
    }),
    forget: () => {
      confirmingDelete.value = null
    },
    onResult: (name) => {
      if (name === 'close') closeDialog('delete-task')
      if (name === 'confirm') deleteTask(confirmingDelete.value)
    }
  })
}

/* The dialog closes first and bd runs after — `cutBranch` above records that
   shape for every write behind a dialog in this view, and here it is
   load-bearing rather than a habit.

   `deleteIssue` takes the issue out of `trackerState.issues` **synchronously**,
   before it awaits bd, so with the write first the ground watcher below would
   run on the very microtask this function's `await` yields to and find the
   window standing over an issue that has gone. It would close the window itself
   and raise the notice meant for a board that moved under somebody — over the
   deletion they had just asked for, and held on screen, since only a `success`
   toast is given a timer. That signal is worth having for the dialogs that use
   it honestly, and this is not one of them.

   Nothing is lost by closing early, and there is no spinner anywhere in this
   because there is nothing left to put one on. `deleteIssue` is optimistic in
   the direction opposite to every other write in the store — its own header
   says the card goes at once and comes back if bd refused — and the removal is
   synchronous, in the same block as the line below, so no paint falls between
   them. The card is simply gone from the board by the first frame after Delete
   is pressed, and that is the feedback. The analogy to a cut branch does not
   carry: a cut *adds* a row, and so leaves one to grey.

   `deletingId` therefore has no observable consumer left. It is kept because it
   is the flag the announced `busy` mirrors and because `orderedColumns` reads
   it beside `writingId`, where the pair is one rule rather than two — not
   because anything greys. On the refusal path the issue is put back and the
   flag cleared in the same synchronous continuation, so it is unobservable
   there too.

   What a refusal does get is the place every other refusal from the tracker in
   this view gets — `trackerState.lastError`, as a toast at the foot of this
   file — and the card reappears under it. */
const deleteTask = async (id) => {
  if (!id) return
  closeDialog('delete-task')
  deletingId.value = id
  try {
    await deleteIssue(id)
    if (project.selectedTask === id) project.selectedTask = null
  } catch {
    // the message already sits in trackerState.lastError and draws as a toast
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
      openReadyTask(id)
      return
    }
    return setTaskStatus(id, value)
  }
  if (kind === 'delete') {
    openDeleteTask(id)
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
  if (kind === 'fix') {
    /* From the store rather than from the menu's payload, the reason the status
       branch above spells out: a card's copy may be a delta behind. */
    const issue = issueById(id)
    if (issue) askAgentToFix(issue)
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
    openNewTask({ id: issue.id, title: issue.title })
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

/* The warning, in a window of its own. Its ground is the issue, like the
   delete's: the questions it quotes are the issue's own notes, and an issue that
   has gone leaves the window quoting a card nobody can reach. Three doors, and
   the third — `resolve` — is the one this view had to teach the host to forward
   by name; the other two every dialog has. */
const openReadyTask = (id) => {
  confirmingReady.value = id
  serveDialog('ready-task', {
    ground: { project: activePath.value, issue: id },
    props: () => ({
      /* The frame's caption, in `ReadyTaskModal`'s own words. */
      title: `Move ${confirmingReady.value} to ready with the question unanswered?`,
      id: confirmingReady.value ?? '',
      taskTitle: readyIssue.value?.title ?? '',
      /* Live, not a copy taken at the press: an agent can answer a question
         while the window stands, and the list is the content of this dialog. */
      questions: readyQuestions.value
    }),
    forget: () => {
      confirmingReady.value = null
    },
    onResult: (name) => {
      if (name === 'close') closeDialog('ready-task')
      if (name === 'confirm') moveToReadyAnyway()
      if (name === 'resolve') resolveFromDialog()
    }
  })
}

const moveToReadyAnyway = () => {
  const id = confirmingReady.value
  closeDialog('ready-task')
  if (id) setTaskStatus(id, READY)
}

const resolveFromDialog = () => {
  const issue = readyIssue.value
  closeDialog('ready-task')
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

/* A session that corrects work already finished and merged. Started exactly the
   way editing and answering are, and carrying the same two fields: the agent
   reads the issue itself, and what is wrong with the work is what the person is
   about to say in the terminal. */
const askAgentToFix = async (issue) => {
  const path = activePath.value
  if (!path) return
  project.sideTab = 'agents'
  project.activeTab = 'terminal'
  try {
    await createSession(path, { kind: 'fixTask', id: issue.id, title: issue.title })
  } catch {
    // already reported — see newAgent above
  }
}

/* What the tracker's health means where the board would be. The generic
   "No board yet — connect a tracker" is wrong for a folder without .beads:
   there is nothing to connect to and creating a task there fails. Each state
   says what it is and what to do about it, and all of them stay quiet — this
   is information, not an emergency, and the loud budget belongs to the card
   that is waiting on you.

   The diagnostic text from Rust used to go to the console and nowhere else,
   which is the decision smetana-j7o overturned: a person whose board would not
   come up was told to open developer tools to find out why. It is here now, in
   `healthSaid` below and the `detail` slot it feeds — the last non-empty line
   of the health message, in mono, under the sentence about it. The line is a
   hint and not the payload: the whole of the failure, the failed command and
   the bd version with it, is what "Ask an agent" hands over. */
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
    /* Names the ordinary cause and what the button will do before it does
       anything else. "See the console" used to stand here, which was an
       instruction for whoever wrote the app rather than for whoever uses it —
       and the diagnostic it pointed at is now on the screen, in the `detail`
       slot below. The copy is mentioned because it is what makes pressing the
       button a small decision: there is no confirmation dialog in front of it,
       and the reason there is none is that nothing is lost either way. */
    description:
      'Most often the tracker was made by an older bd than this build ships. Repairing runs bd\'s own migrations, and takes a copy of .beads beside it first.'
  }
}

/* Whatever the health line says, cut to the one line worth putting on the
   screen. The rule is the store's (`lastDiagnosticLine`) because the toast a
   failed repair raises wants the same cut, and a copy of it here would be a
   rule living in the one kind of file no test in this repository can reach.

   It was called `bdSaid` while `error` was the only state that drew it, and the
   name stopped being true when `folder-refused` split off: there the line is a
   path the app could not read and bd never said a word about it, since bd never
   got that far. */
const healthSaid = computed(() => lastDiagnosticLine(trackerState.health.message ?? ''))

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
  /* The one state whose notice is not a constant: what it says depends on
     whether there is a button under it, and that depends on the platform. The
     rule is `folderAccess.js`'s rather than a ternary here, because it is copy
     and copy is the whole of this state — see the note there. */
  if (trackerState.health.state === 'folder-refused') {
    return folderRefusedNotice(trackerState.folderAccessRepair)
  }
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
const onSelectFile = (path, kind) => {
  if (isStubPath(path)) return
  project.selectedPath = path
  /* A folder is selected and never opened. The click that selects it also
     toggles it, which is the whole of what a folder row does, and `openFile` on
     one would open a tab that reads `notAFile` — the selection is what the
     keyboard's verbs are about, and that is the only reason a folder is
     selected at all. */
  if (kind === 'dir') return
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

/* The ground watcher for every dialog window this view has open.

   It lives down here rather than beside `serveDialog` because a `watch` runs
   its source the moment it is created, and the source reads `projectColumns`
   and this file's toast — both of which are declared above this line and below
   that one.

   A dialog window has no scrim, so the board can move underneath it. When what
   a window stands on goes, the window goes with it, and the notice lands here
   rather than there — the person is in this window, since this is where they did
   the thing that moved the ground.

   One watcher for every kind rather than one per dialog: the rule is the
   registry's, and a kind added there is covered by this without a line changing
   here. */
watch(
  () => ({
    project: activePath.value,
    /* The repository the Git panel has selected, and not the list of them. A
       write in that panel resolves which repository it runs in at the moment it
       is pressed, so what a dialog about a repository stands on is the selection
       — see `stalenessOf`, which spends a paragraph on why. */
    repo: vcsState.selected,
    issues: new Set(trackerState.issues.keys()),
    columns: new Set(projectColumns.value),
    branches: new Set(vcsState.branches.map((branch) => branch.name))
  }),
  (world) => {
    if (!openDialogs.size) return
    for (const [kind, { ground }] of [...openDialogs]) {
      const reason = stalenessOf(kind, ground, world)
      if (!reason) continue
      closeDialog(kind)
      /* This view's one ad-hoc toast, whose name records the first thing that
         used it rather than what it is for. Held rather than timed: the window
         vanished while the person was looking somewhere else, so the sentence
         explaining it has to still be there when they look back. */
      sayFileMenu({ tone: 'info', title: stalenessMessage(kind, reason) })
    }
  }
)

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

/* What an entry that has just appeared earns on screen, for the two verbs that
   produce one out of another: the folder it landed in is re-read — the tree has
   no watcher, so whoever changed a folder re-reads it — the row is selected,
   and a file opens as a permanent tab rather than a preview, since it was asked
   for a moment ago and a tab the next click would replace is not what somebody
   wants. A folder is expanded instead, which is the same answer in the tree's
   own terms.

   Deliberately not shared with the tail of `makeEntry`, which reads almost the
   same and differs in the one thing that matters: that verb *knows* which of
   the two it made and this one has to ask. A copy answers with a path and
   nothing else, so which it is comes off the listing just read — and a folder
   made by hand is expanded without being selected, where one pasted is both,
   because a paste is a destination somebody chose and a new folder is a place
   they are about to fill. */
async function revealMade(path, dir) {
  await listDir(dir)
  project.selectedPath = path
  const made = filesState.dirs.get(dir)?.entries.find((entry) => entry.path === path)
  /* Nothing at all when the listing did not come back — a refused read, or a
     read of that folder already in flight, which `listDir` declines to start a
     second time. `openFile` on a guess would open a tab over a folder and show
     `notAFile` in it; the row is selected either way, and the next window focus
     brings the listing. */
  if (!made) return
  if (made.kind === 'dir') {
    if (!project.expanded.includes(path)) project.expanded.push(path)
    await listDir(path)
  } else {
    openFile(path, { permanent: true })
  }
}

/* The tidying a move or a rename owes, and the whole of what makes it different
   from a delete: the file is still there.

   Open tabs follow it. `renameTab` changes the id and moves the buffer whole —
   unsaved text, `mtime` and dirtiness included — because the file has not
   changed and neither has its timestamp, so the buffer is as valid as it was a
   moment ago. Closing them the way `deleteEntry` closes them was the
   alternative and it throws away somebody's place in a file for nothing.

   Diff tabs are the one thing closed rather than carried. Their left-hand side
   is `vcs_file_at_head`, where the file is still under the name it had, so a
   diff that followed the move would be a diff against the wrong thing. They are
   found exactly as `deleteEntry` finds them — through `diffTabs`, converted
   into the tree's path space with `relativeTo`, never against the bare
   `tab.path`, which is relative to a repository and not to the project — and
   the reasoning there covers this line too.

   `expanded` and `selectedPath` are settings, and both would otherwise keep
   naming a path that is not there: harmless on screen and permanent in
   `settings.json`. They are rewritten rather than dropped, since the folder
   they named is open and still is.

   What is handed back is those rewritten folders, and the caller owes every one
   of them a `listDir`. Nothing else will ever read them: `filesState.dirs` is
   keyed by path, so a folder renamed from `src/a` to `src/b` is open according
   to `expanded` and unknown according to `dirs` — `treeNodes` gives it
   `children: undefined`, the row draws a collapsed chevron, and the click that
   should open it finds the path already in `expanded` and takes it out instead,
   so the first click does nothing visible and the second one opens the folder.
   The focus sweep cannot repair it either, since `refreshDirs` re-reads only
   what `dirs` already holds. The list is returned rather than read here because
   a paste has just read one of these folders itself. */
function followMove(from, to) {
  const under = (other) => other === from || other.startsWith(`${from}/`)
  const moved = (other) => `${to}${other.slice(from.length)}`
  /* Taken before anything moves: `tabList` is computed off the very list
     `renameTab` splices, and `diffTabs` is the list `closeDiff` splices. */
  const moving = tabList.value
    .filter((tab) => (tab.kind === 'file' || tab.kind === 'preview') && under(tab.id))
    .map((tab) => tab.id)
  const closingDiffs = diffTabs
    .filter((tab) => {
      const rel = relativeTo(filesState.root, `${tab.repo}/${tab.path}`)
      return rel !== null && under(rel)
    })
    .map((tab) => tab.id)
  for (const id of moving) renameTab(id, moved(id))
  for (const id of closingDiffs) closeDiff(id)
  const reopened = []
  for (let i = 0; i < project.expanded.length; i += 1) {
    if (!under(project.expanded[i])) continue
    project.expanded[i] = moved(project.expanded[i])
    reopened.push(project.expanded[i])
  }
  if (project.selectedPath && under(project.selectedPath)) {
    project.selectedPath = moved(project.selectedPath)
  }
  return reopened
}

/* Which of the two clipboards a paste would act on — `pasteChoice` in the
   absolute space the choice is made in, and `pasteRecord` the same answer put
   back into the tree's own, which is what the tree is drawn from.

   The conversion out and back is not ceremony: the two clipboards do not speak
   the same language. `filesState.clipboard` holds paths relative to the
   project, because everything in that store does; the machine's holds absolute
   ones, because a clipboard has no notion of a project. They can only be
   compared in one of the two, and it has to be the absolute one — the other
   cannot say "somewhere else on the disk", which is exactly what a file copied
   in Finder usually is.

   A path that comes back `null` from `relativeTo` is outside the project and is
   kept absolute. That is an ordinary paste — the file is copied **into** the
   project, which is what a paste means — and it is also why `canPasteInto`
   answers `ok` for one: an absolute path is not a prefix of any folder in the
   tree, so no row is greyed over it. That is the allowed direction of the two
   copies of that rule disagreeing; Rust's `refuse_into_self` still refuses a
   folder from outside that holds the project.

   Both halves of the tree read this rather than the raw record: what Paste is
   offered for, and which rows are drawn muted for a pending cut. A cut in the
   tree followed by a copy in Finder means the cut is not what will happen next,
   and a row still drawn muted would be a promise nothing was going to keep. */
function pasteChoice() {
  const internal = filesState.clipboard
  return pasteSource({
    internal: internal && {
      paths: internal.paths.map((path) => absolutePath(filesState.root, path)),
      mode: internal.mode
    },
    system: filesState.systemClipboard,
    spent: filesState.clipboardSpent
  })
}

const pasteRecord = computed(() => {
  const chosen = pasteChoice()
  if (!chosen) return null
  return {
    paths: chosen.paths.map((path) => relativeTo(filesState.root, path) ?? path),
    mode: chosen.mode
  }
})

/* Paste, which is a copy or a move depending on what put the record there, and
   which of the two clipboards is the one holding it.

   The system clipboard is re-read here rather than taken from the mirror the
   menu was drawn from, because this is the moment the answer has to be right:
   somebody may have copied something in Finder between opening the panel and
   picking the row, and the mirror is refreshed only on window focus and when
   the menu opens.

   **A source outside the project is copied and never moved**, whatever mode the
   platform stated. Windows and Linux both have a cut for files and both can say
   so, and honouring it would mean deleting a file this app has made no promises
   about, outside every check `files/fs.rs` makes — for a gesture made in
   another program. The file lands either way; what is left behind is the
   original, which is the smaller of the two mistakes. macOS cannot reach this
   at all, having no cut for files.

   One path out of the array, because the tree selects one entry: the shape is
   plural so that multiple selection does not change it later, and taking the
   first is what the rest of this will do until it arrives.

   The destination was checked before the menu was drawn — `canPasteInto` greys
   the row for a folder inside what was copied — and it is checked again in
   Rust. That is not the same check twice for nothing: the first is a label
   somebody reads instead of a click that fails, and the second is the one that
   stays true when a symlink is in the path. */
async function pasteInto(dir) {
  await readSystemClipboard()
  const record = pasteChoice()
  const source = record?.paths[0]
  if (!source) return
  /* The one question that decides which of the three calls this is. `null` is
     an ordinary answer and means the source is not in this project at all. */
  const inside = relativeTo(filesState.root, source)
  const cut = record.mode === 'cut' && inside !== null
  try {
    const made =
      inside === null
        ? await copyExternalEntry(source, dir)
        : cut
          ? await moveEntry(inside, dir)
          : await copyEntry(inside, dir)
    let reopened = []
    if (cut) {
      reopened = followMove(inside, made)
      /* Both folders, since nothing else will: the one that lost a row and, in
         `revealMade`, the one that gained it. */
      await listDir(parentOf(inside))
    }
    await revealMade(made, dir)
    /* Every folder that was open under what moved, at its new path — see
       `followMove`. `revealMade` has usually just read one of them, and it is
       read again rather than skipped: it returns without reading whenever the
       destination listing did not come back, which is reachable while a
       window-focus sweep holds that folder, and skipping on the strength of a
       read that may not have happened would leave standing exactly the dead
       first click this loop exists to kill. Asking `filesState.dirs` instead
       would answer yes for a listing left over from an earlier entry at the
       same path. One extra `files_list` on a folder just read is the cheaper
       of the two mistakes. */
    for (const open of reopened) await listDir(open)
  } catch (error) {
    /* The destination is re-read even though nothing was supposed to happen,
       because on one path something may have: a move across filesystems is a
       copy and then a delete (`move_across_devices` in `files/fs.rs`), and a
       delete that fails leaves the copy standing and answers `io`. A row that
       is on disk and not in the tree until the next window focus is worse than
       the toast beside it. */
    await listDir(dir)
    sayFileMenu({
      tone: 'error',
      title: cut ? 'Nothing was moved' : 'Nothing was pasted',
      description: copyErrorText(error)
    })
  }
}

/* Duplicate: a copy into the folder the entry is already in, in one action and
   with no clipboard in it at all — which is why it neither reads the record nor
   replaces it, and why a pending cut survives one.

   The name is the back end's: nothing here overwrites and nothing asks, so
   `report.md` beside a `report.md` becomes `report copy.md`, then
   `report copy 2.md`. */
async function duplicateEntry(path) {
  const dir = parentOf(path)
  try {
    await revealMade(await copyEntry(path, dir), dir)
  } catch (error) {
    sayFileMenu({
      tone: 'error',
      title: 'Nothing was duplicated',
      description: copyErrorText(error)
    })
  }
}

/* The new name the draft row was typed with, and the rule about it is
   `newEntry.js`'s — the same one `makeEntry` applies and for the same reason:
   an empty field is somebody who changed their mind and is answered with
   silence, and a name no entry can carry is answered here rather than by a trip
   across the IPC.

   A third answer this one has and making does not: the name typed back exactly
   as it was. `files_rename` would refuse it as `alreadyExists`, which is true
   and would put a red toast over a change nobody made, so it is a cancel. The
   comparison is against the **trimmed** name, since a person who typed
   " a.txt " over `a.txt` changed nothing either.

   Nothing is opened and nothing is selected that was not: the entry was already
   on screen, and the tab over it — if there was one — has followed it. */
async function renameEntryTo(path, typed) {
  const { verdict, name } = checkNewName(typed)
  if (verdict === 'nothing' || name === basenameOf(path)) return
  if (verdict === 'refused') {
    sayFileMenu({
      tone: 'error',
      title: 'Nothing was renamed',
      description: renameErrorText({ kind: 'badName' })
    })
    return
  }
  try {
    const made = await renameEntry(path, name)
    const reopened = followMove(path, made)
    await listDir(parentOf(path))
    /* The renamed folder itself among them, if it was open: nothing here plays
       `revealMade`'s part, so this loop is the only read those folders get. */
    for (const open of reopened) await listDir(open)
  } catch (error) {
    sayFileMenu({
      tone: 'error',
      title: 'Nothing was renamed',
      description: renameErrorText(error)
    })
  }
}

const onFileAction = async ({ kind, path, target, name }) => {
  const root = filesState.root
  if (kind === 'create-file' || kind === 'create-dir') {
    /* `path` is the folder the draft row sat in — the tree worked that out when
       it opened the field, since that is where the row was drawn. */
    await makeEntry(kind === 'create-dir' ? 'dir' : 'file', path, name)
  } else if (kind === 'commit-rename') {
    /* `path` is the entry the draft row was drawn over, and `name` is what was
       typed into it — the same shape the two making verbs come back in. */
    await renameEntryTo(path, name)
  } else if (kind === 'delete') {
    await deleteEntry(path)
  } else if (kind === 'copy' || kind === 'cut') {
    /* No disk: cutting a folder of any size is a record, which is what makes it
       instant and what makes changing one's mind free. The row is drawn muted
       from the moment `setClipboard` returns, which is before the await inside
       it. What is awaited is the trip to the machine's clipboard, so that a
       copy here can be pasted in Finder — and a refusal there is swallowed,
       since the record is what a paste inside the tree runs on. */
    await setClipboard([path], kind)
  } else if (kind === 'paste') {
    await pasteInto(folderOf({ path, target }))
  } else if (kind === 'duplicate') {
    await duplicateEntry(path)
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
  /* `new-file`, `new-folder` and `rename` never arrive here: all three put a
     field in the tree and come back later as `create-file`, `create-dir` or
     `commit-rename` with a name. */
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
  /* Held until dismissed is right for a note naming a folder, and it is what
     makes this line necessary: the folder it names is beside one project's
     `.beads`, so left standing it would sit over the next project telling
     somebody about a copy that is not in it. */
  repairNote.value = null
})

/* The Sessions tab reads the disk when it is opened, and again whenever the
   project changes under it. Deliberately both, and deliberately nothing else.

   Reading on the tab rather than on the project is what keeps the cost off
   everybody who never opens it: this is a walk of `~/.claude/projects`, which
   is hundreds of files and hundreds of megabytes for one project. There is no
   watcher for the same reason, and the store carries the whole of that
   argument.

   A project switch while the tab is closed leaves the store holding the list of
   the project somebody has left, and that is safe rather than overlooked: the
   store empties a list belonging to another project the moment this one is
   asked about, so the stale rows can never reach the screen — the tab cannot be
   drawn without this watcher having fired. */
watch(
  [activePath, () => project.rightTab],
  ([path, tab]) => {
    if (tab === 'sessions') loadSessionHistory(path)
  },
  { immediate: true }
)

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

/* The status footer's one sentence about this project. Derived rather than
   stored, like everything else in that strip: the rule is
   components/shell/headline.js and both of its inputs are already reactive
   here.

   The agents come from `agentCounts` and not from the rail's `projectStates`,
   which is the map that knows about every project at once. That map counted a
   person's own shells when this was written, so a shell that rang the bell
   would have had this strip announce an agent waiting on somebody in a project
   holding no agent at all; the mark carries a work kind now and the map drops
   them, but the source is unchanged — the counter beside this sentence is built
   from the sessions and the two have to agree. The store comment beside
   `agentCounts` has the whole of it. This is the active project's strip, so the
   active project's own list is the right source anyway. */
const stateHeadline = computed(() =>
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

/* The other document a finished session leaves, and a much shorter road than
   the one above it: a branch review has no switch, no bell and no choice to
   make. Somebody pressed Review and the report is the whole of what that
   session produces, so the only question is when, and the answer is the moment
   the agent exits.

   `reviewReportTabs` is the rule and it is the whole of it — which sessions
   have a document, and what it is called. Nothing watches `.smetana/reviews/`
   and nothing asks the disk on a timer: the app named the file before the
   session started, so the session's own ending is the signal, and a third
   filesystem watcher is not something this project is willing to grow.

   `openFile` is the file tree's own call, so the tab lands in `openTabs` as an
   ordinary project-relative path and survives a restart like every other one,
   and `reportTabActive` above is what makes the centre draw the document rather
   than its source. Permanent rather than a preview, for the reason `showReport`
   gives: this is a document somebody asked for by name.

   No file — an agent that fell over, or wrote somewhere else — is not handled
   here and deliberately: `openFile` meets it exactly as it meets any missing
   path, with the buffer carrying the read error, and inventing a check here
   would be a second way of asking whether a file exists.

   The key is the ids, `stoppedRuns`' shape, and `openedReviews` is
   `decidedRuns`' — `terminalState.sessions` is replaced wholesale on a project
   switch and read again on a return, so an ending that is already answered for
   arrives here as often as somebody moves between projects. An ending that
   landed while another project was on screen opens its tab on the way back
   instead, which is the first delivery rather than a repeat of one: sessions
   live in the worker's memory and none of them outlives the app, so there is no
   last night's document to be surprised by.

   `activePath` is handed to the rule rather than assumed, and that is what
   makes the paragraph above true: during a move the active project changes
   before the session list does, so without it an ending arriving in that gap
   would open a tab into the new project's list, have it overwritten by
   `applySection`, and be recorded as answered all the same. */
const finishedReviews = computed(() =>
  reviewReportTabs(terminalState.sessions, activePath.value)
)
const openedReviews = new Set()
watch(
  () => finishedReviews.value.map((review) => review.id).join(' '),
  () => {
    for (const review of finishedReviews.value) {
      if (openedReviews.has(review.id)) continue
      openedReviews.add(review.id)
      openFile(review.path, { permanent: true })
    }
  }
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
/* Panel scrolls its slot as one block, and both side columns need more than
   that: a tab row at the top and, in the left column, the worktree line under
   the foot, with only what is between them scrolling. Shared by the two since
   the right column grew a row of its own — the same four lines under both, so
   the two rows sit at the same height and stay there. */
const tabbedColumnStyle = {
  display: 'flex',
  flexDirection: 'column',
  height: '100%',
  minHeight: 0
}
/* The tab row itself is `components/shell/SegmentedTabs.vue` now, and both side
   columns draw theirs with it. It moved out of this file whole — the two style
   objects that used to stand here, the roles, the roving tabindex and the focus
   ring — when the right column grew a row of its own: two copies of that, a
   thousand lines apart in one file and obliged to match, is the pair that
   drifts where nobody is looking. */

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
   intercepts nothing: with no children its size is zero.

   The bottom is measured from the usage strip rather than from the window,
   which is the mirror of what `notificationsBoxStyle` above does with the scope
   bar: the strip is 30px tall against a 16px inset, so a toast measured from
   the window would rest on the bar instead of floating over the working area,
   and in compact both numbers shrink together. */
const toastStackStyle = {
  position: 'fixed',
  right: 'var(--space-6)',
  bottom: 'calc(var(--scope-bar-h) + var(--space-6))',
  zIndex: 'var(--z-toast)',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'flex-end',
  gap: 'var(--space-4)'
}
</script>

<template>
  <div :style="rootStyle">
    <!-- The project's name and the branch it is on, both live, and the whole of
         what this bar says now: what the project is *doing* is along the bottom
         of the window instead. `worktree` is left empty on purpose: the
         component shows worktree-or-branch in that slot and appends "@branch"
         only when both are set, so passing the branch alone is what puts it
         there once, undecorated. -->
    <ScopeIndicator
      ref="scopeBar"
      :repo="activePath ? basename(activePath) : '—'"
      worktree=""
      :branch="branchLabel"
      :notifications="notificationsState.items.length"
      :window-chrome="barChrome"
      :maximized="maximized"
      @notifications="toggleNotifications"
      @settings="openSettingsWindow()"
      @minimize="minimizeWindow"
      @toggle-maximize="toggleMaximizeWindow"
      @close="closeWindow"
    >
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
          @settings="openProjectSettings"
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
          <div :style="tabbedColumnStyle">
            <!-- Above what it scopes rather than at the foot of the column,
                 and the same component the right column draws its own row
                 with. -->
            <SegmentedTabs v-model="project.sideTab" :tabs="SIDE_TABS" />
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
                :clipboard="pasteRecord"
                @toggle="toggleDir"
                @select="onSelectFile"
                @open="onOpenFile"
                @menu="readSystemClipboard"
                @action="onFileAction"
              />
              <GitPanel
                v-else-if="project.sideTab === 'git'"
                :repos="vcsState.repos"
                :unlisted="vcsState.unlisted"
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
                :favorite-branches="project.favoriteBranches"
                :message="draftMessage()"
                :suggesting="vcsState.suggesting"
                :suggest-error="vcsState.suggestError"
                :conflicts="vcsState.conflict ? vcsState.conflict.files.length : 0"
                @resolve-conflicts="openConflict"
                @toggle="toggleGitSection"
                @toggle-folder="toggleBranchFolders"
                @favorite="setFavoriteBranches"
                @resize="resizeGitSection"
                @setup="openSetup(activePath, true)"
                @select="selectRepo"
                @checkout="checkout"
                @compare="openCompareWindow(vcsState.selected, $event)"
                @review="openReviewChanges($event)"
                @merge="merge"
                @rebase="rebase"
                @pull="pull"
                @push="push"
                @fetch="fetchNow"
                @new-branch="openNewBranch"
                @delete="openDeleteBranch"
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
               have no project root to open it in, the third would file a task
               against no tracker, and the fourth would ask for a review of no
               repositories. -->
          <template #afterPinned>
            <MenuButton
              icon="plus"
              label="New agent, terminal, task or review"
              :items="NEW_TAB_ITEMS"
              :width="180"
              :disabled="!activePath"
              @select="onNewTab"
            />
          </template>
        </TabBar>
        <!-- A merge or a rebase that stopped on conflicts. It closes now, and
             `dismissConflict` is what closing means: the record stays, the
             panel goes on drawing `Resolve conflicts` above the commit button
             for as long as the tree is conflicted, and pressing that is
             `openConflict`. There was no dismiss while there was no way back
             in; there is one, so a dialog with no exit would only be a trap.
             Everything drawn in it comes from that record — including which
             repository, since the panel's selection can have moved since. -->
        <!-- Every dialog of this app but two is a window of its own now,
             opened above by `openRun`, `openNewTask`, `openNewBranch`,
             `openDeleteBranch`, `openPromote`, `openSetup`,
             `openProjectSettings`, `openDeleteTask`, `openReadyTask` and
             `openDeleteSession` —
             the whole of `REGISTRY` in `dialogRegistry.js`, which is what
             finishes the epic this comment was first written in the middle of.
             What each of them is a question about — a list of branches, the
             board filling in behind a promote, the card somebody is about to
             delete — can be read beside it now instead of from behind a scrim.
             Two remain, and both are here rather than overlooked: the conflict
             above and "Save changes?" below exist in order to block, and a
             window somebody can push aside and click past is the one thing
             neither may be. -->
        <ConflictModal
          v-if="vcsState.conflictOpen && vcsState.conflict"
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
          @close="dismissConflict"
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
        <div
          v-else-if="trackerState.switching && !initing && !repairing"
          :style="{ padding: 'var(--panel-pad)' }"
        >
          <Skeleton :lines="6" :height="12" />
        </div>
        <EmptyState v-else-if="healthNotice" v-bind="healthNotice">
          <!-- What bd said, under the sentence about it: the diagnostic used to
               go to the console alone, which asked a person to open developer
               tools to find out why their board was empty. -->
          <!-- And the same line under a refused folder, where it is the path
               that was refused: "this folder" is one folder too few the moment
               a project's own directory and its `.beads` can be refused apart. -->
          <template
            v-if="
              (trackerState.health.state === 'error' ||
                trackerState.health.state === 'folder-refused') &&
              healthSaid
            "
            #detail
          >
            {{ healthSaid }}
          </template>
          <template v-if="trackerState.health.state === 'not-a-beads-repo'" #action>
            <Button variant="primary" size="sm" :disabled="initing" @click="initHere">
              {{ initing ? 'Initializing…' : 'Initialize bd' }}
            </Button>
          </template>
          <template v-else-if="trackerState.health.state === 'no-project'" #action>
            <Button variant="primary" size="sm" @click="onAddProject">Add project…</Button>
          </template>
          <!-- The deterministic door and the open-ended one, in that order and
               in that weighting: repairing costs four seconds and fixes the
               failure this screen was built for, while an agent costs a run and
               tokens, so it is the second button rather than the only one. It
               is offered on any failure, since there is nothing here that
               classifies one. -->
          <template v-else-if="trackerState.health.state === 'error'" #action>
            <div :style="{ display: 'flex', gap: 'var(--space-3)' }">
              <Button variant="primary" size="sm" :disabled="repairing" @click="repairHere">
                {{ repairing ? 'Repairing…' : 'Repair tracker' }}
              </Button>
              <Button variant="ghost" size="sm" :disabled="repairing" @click="askAgentAboutTracker">
                Ask an agent
              </Button>
            </div>
          </template>
          <!-- One button and no "Ask an agent" beside it: an agent cannot reach
               the permission database, and a folder this app may not read is a
               folder an agent started in it could not read either. Drawn only
               where there is something to press — elsewhere the sentence above
               carries the whole of what to do. -->
          <template
            v-else-if="
              trackerState.health.state === 'folder-refused' &&
              folderRefusedHasReset(trackerState.folderAccessRepair)
            "
            #action
          >
            <Button variant="primary" size="sm" :disabled="resettingAccess" @click="resetAccessHere">
              {{ resettingAccess ? 'Resetting…' : 'Reset and restart' }}
            </Button>
          </template>
        </EmptyState>
        <KanbanBoard
          v-else
          :columns="drawnColumns"
          :filtered="orderedColumns.length > 0"
          :selected-id="highlightedTask"
          :copied-id="copiedTaskId"
          :copy-state="taskIdCopyState"
          :add-to="ADD_TO"
          :run-from="runOffered ? ADD_TO : null"
          :run-blocked-reason="runBlockedReason"
          :promote-from="PROMOTE_FROM"
          @select="selectFromBoard"
          @add="openNewTask()"
          @run="openRun({ kind: 'queue' })"
          @promote="openPromote"
          @task-action="onTaskAction"
          @copy-id="copyTaskId"
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
          title="Task &amp; sessions"
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
               tab is the third condition, and it is the same sentence one level
               up: on Sessions there is no card in this column at all, and a
               menu offering to act on one nobody can see is worse than no menu.
               The
               menu is wider than this column ever gets, which costs nothing:
               `MenuButton` is fixed-position, right-aligned to the trigger and
               clamped to the window, so it opens leftwards over the board. -->
          <template #actions>
            <MenuButton
              v-if="project.rightTab === 'task' && inspectedIssue"
              :items="inspectedMenu"
              :label="`Actions for ${inspectedIssue.id}`"
              :width="MENU_W"
              icon="ellipsis"
              size="sm"
              @select="onTaskAction({ kind: $event.kind, id: inspectedIssue.id, value: $event.value })"
            />
          </template>

          <!-- Panel scrolls its slot as one block, and the tab row has to stay
               put over what it scopes — so the row and a scrolling box under it
               are this column's own, exactly as the left column builds them. -->
          <div :style="tabbedColumnStyle">
            <SegmentedTabs v-model="project.rightTab" :tabs="RIGHT_TABS" />
            <div :style="{ flex: 1, minHeight: 0, overflow: 'auto' }">
              <!-- The Task tab: what this column drew before there was a row
                   over it, unchanged. Which of the four is on screen is still
                   derived rather than stored — only the tab above is
                   remembered, never what the tab is filled with. -->
              <div v-if="project.rightTab === 'task'" :style="inspectorBody">
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
                  :copy-state="copyStateFor(inspectedIssue.id)"
                  @open="openExternal"
                  @copy-id="copyTaskId"
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

              <!-- The Sessions tab: this project's Claude Code sessions as they
                   are on disk, newest first, and none of the live agents of
                   this run of the app — those are the left column's Agents tab
                   and stay there. The tab used to draw that same live list,
                   which made a freshly launched app show an empty column while
                   the machine held hundreds of transcripts of work done in this
                   project; the premise that no history existed was simply
                   wrong.

                   A session here is a conversation that is over, so the row is
                   not `AgentList`'s: there is no state to watch, no timer and
                   nothing to remove — a title, what was last said, and how much
                   of it there was. -->
              <div v-else>
                <!-- The rule between two rows belongs to the lower of them,
                     which is why the index is here at all: an inline-style
                     component has no `:last-child`, so a list that let every
                     row draw its own rule ended in one with four hundred pixels
                     of empty panel under it. -->
                <SessionRow
                  v-for="(session, index) in sessionsState.sessions"
                  :key="session.id"
                  :session="session"
                  :now="sessionsState.now"
                  :agent="settings.agent"
                  :separated="index > 0"
                  :expanded="isSessionOpen(session.id)"
                  :busy="deletingSessionPath === session.path"
                  :copy-state="sessionCopyStateFor(session.id)"
                  :copy-noun="sessionCopyNounFor(session.id)"
                  @toggle="toggleSession"
                  @action="onSessionAction"
                />
                <!-- Said in words rather than left blank, and not while the
                     read is still out: this sentence is a claim about what is
                     on the disk, and making it before anybody has looked would
                     be a claim nobody checked. -->
                <EmptyState
                  v-if="!sessionsState.sessions.length && !sessionsState.loading"
                  compact
                  icon="terminal"
                  title="No sessions yet"
                  description="Claude Code sessions run in this project will appear here."
                />
              </div>
            </div>
          </div>
        </Panel>
      </div>
    </div>

    <!-- The strip along the bottom, the sibling of the scope bar above. At its
         left, what is left of the agent's subscription, which otherwise lives
         only on a tab of a window somebody has to open on purpose; at its
         right, what this project is doing right now. Outside the three columns
         rather than inside the middle one — neither half is about the board,
         and a strip that stopped at the board's edges would read as a caption
         to it. It is drawn in every state, this window's own `footer` beside
         `AppShell`'s slot of the same name.

         Both counters are the stores' own computeds and neither is counted
         here: the files are the Git panel's selected repository, so the number
         is the length of the list that panel draws, and the agents are the left
         column's rows minus the ones that have finished. The two rules live in
         vcs.js and terminals.js because a rule in this file is a rule no test
         can reach. Note that with several repositories in one project the
         branch in the bar above is the project root's while this count is the
         selected repository's — the panel is where a person is looking at that
         list, and this is the number they can check against it. -->
    <StatusFooter
      :usage="usageReading"
      :busy="usageBusy"
      :error="usageError"
      :dirty-count="dirtyCount"
      :agents-active="liveAgentCount"
      :headline="stateHeadline.text"
      :headline-level="stateHeadline.level"
      @refresh="readUsage"
    >
      <template #status>
        <!-- One segment per run, oldest first — a project holds several now,
             and each segment's stop names its own run by token. The strip's
             own gap spaces the segments; RunBar draws nothing for a run it was
             not given, so an empty list costs no width. `busy` is deliberately
             not bound: the run a confirm is starting has no segment until the
             worker answers, so `runStarting` is about none of these, and
             passing it disabled the other live runs' stop buttons over a start
             that never touches them. -->
        <RunBar v-for="r in runsState.runs" :key="r.token" :run="r" @stop="stopTheRun(r.token)" />
      </template>
    </StatusFooter>

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
      <!-- A repair's success, which the board coming back does not say: the
           copy it took is the one thing left on disk afterwards, and nothing
           will ever remove it. Held until dismissed, unlike the timed toast
           below — this one names a path. -->
      <Toast
        v-if="repairNote"
        tone="success"
        :title="repairNote.title"
        :description="repairNote.description"
        @close="repairNote = null"
      />
      <!-- The file tree's menu, which is the other thing in this column that
           has a success to report: a copy leaves nothing on screen behind it. -->
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
