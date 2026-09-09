<script setup>
/* Dev harness: renders every component in the library once, so a broken port
   shows up here rather than in the product. Not part of the shipped app —
   reachable at ?view=gallery. */
import { computed, ref, watchEffect } from 'vue'
import { orderColumns } from '../components/kanban/columnOrder.js'
import { branchMenuItems } from '../components/git/branchMenu.js'
import { CHANGE_MENU_W, changeMenuItems } from '../components/git/changeMenu.js'
import { filterBranches, originBranches } from '../components/git/branchTree.js'
import { FILE_MENU_W, fileMenuItems } from '../components/files/fileMenu.js'
import { MENU_W, taskMenuItems } from '../components/kanban/taskMenu.js'
import {
  copyNoun as copyVerbNoun,
  copyPayload,
  isCopyKind
} from '../components/agent/sessionMenu.js'
/* The app's own copy-confirmation policy rather than this page's rendering of
   it: one duration, one rule about which press owns the state, in one file. */
import { useCopyFeedback } from '../components/core/copyFeedback.js'
import { NEW_TAB_ITEMS } from '../components/shell/newTabMenu.js'
import { orderTabs } from '../components/shell/tabOrder.js'
import {
  AboutSettings,
  AgentList,
  AgentSettings,
  AppShell,
  Assignee,
  AttachmentStrip,
  BranchList,
  BranchPicker,
  BranchSelect,
  Button,
  ChangeList,
  CommitBox,
  CompareList,
  ChatMessage,
  Checkbox,
  ClaimedTasks,
  CodeBlock,
  ColumnHeader,
  CommandPalette,
  ConflictModal,
  ContextMenu,
  DeleteBranchModal,
  DeleteSessionModal,
  DeleteTaskModal,
  DependencyMark,
  DiffView,
  DiscardChangeModal,
  Dropdown,
  DependencySpine,
  DraftInspector,
  EditorSettings,
  EmptyState,
  FileEditor,
  FileTree,
  FileTreeDraftRow,
  FileTreeRow,
  GeneralSettings,
  GitSettings,
  GitPanel,
  Icon,
  IconButton,
  ImageViewer,
  Input,
  KanbanBoard,
  KanbanSettings,
  LogView,
  Markdown,
  MenuButton,
  NewBranchModal,
  Modal,
  RenameBranchModal,
  PointerMenu,
  NewTaskModal,
  NotificationCard,
  NotificationPanel,
  Panel,
  ProjectRail,
  ProjectTile,
  PromoteColumnModal,
  ReadyTaskModal,
  RepoList,
  ReviewChangesDialog,
  ScopeIndicator,
  SectionHeader,
  SegmentedTabs,
  ProjectSettingsModal,
  ReportView,
  SessionRow,
  Select,
  RunBar,
  SettingsGroup,
  SettingsRow,
  RunModal,
  SetupProjectModal,
  Skeleton,
  StatusBadge,
  StatusDot,
  StatusFooter,
  StorageSettings,
  Switch,
  TabBar,
  TaskCard,
  TaskInspector,
  TaskSearchButton,
  TerminalView,
  Textarea,
  TypeBadge,
  Toast,
  ToolCall,
  Tooltip,
  WindowControls
} from '../components/index.js'
import { gitActions } from '../components/git/gitActions.js'
import {
  runNotification,
  storageNotification,
  updateNotification
} from '../components/notifications/notifications.js'
import { logLines } from './desktopAppData.js'
import { folderRefusedNotice } from './folderAccess.js'
import { MOCK_TREE } from '../stores/mockBackend.js'
/* The app's one link-opening path, bound to what the inspector raises. In
   a browser it is a new tab; in the app it is the person's own browser.
   `copyText` is the other half of the same arrangement: a card's id and an
   inspector's raise `copy-id` and know nothing about a clipboard, so the thing
   drawing them has to answer — here as in `DesktopApp.vue`. */
import { copyText, openExternal } from '../stores/app.js'
/* The harness catalogue, so the agent picker on this page is the one the
   settings window draws rather than a second list written out here. Read once
   at startup in `main.js`; in a browser `mockBackend.js` answers it. */
import { agents } from '../stores/agents.js'
import { fileIconUrl } from '../catppuccinIcon.js'
import { documentTheme } from '../documentTheme.js'

/* Two attachments for the strip and for the dialog above it. Eight-pixel PNGs
   written out as data URLs, which is exactly the shape `attachments.js` builds
   from what Rust stored — a fixture that pointed at a file on disk would draw
   nothing here and nothing in the browser. */
const ATTACHMENTS = [
  {
    path: '/Users/you/Library/Application Support/com.invisor.smetana/attachments/20260806-121314-mock.png',
    name: '20260806-121314-mock.png',
    bytes: 96,
    url: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAD0lEQVR42mPIwwEYhpYEADyoUoFZDU7TAAAAAElFTkSuQmCC'
  },
  {
    path: '/Users/you/Library/Application Support/com.invisor.smetana/attachments/20260806-121315-flow.png',
    name: '20260806-121315-flow.png',
    bytes: 96,
    url: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAD0lEQVR42mNwwAEYhpYEAMHWMAEiHQbtAAAAAElFTkSuQmCC'
  }
]

/* Enough of them to overflow the strip's two-row ceiling, which is the state
   that would otherwise push a dialog's footer off a short screen. */
const MANY_ATTACHMENTS = Array.from({ length: 14 }, (_, i) => ({
  ...ATTACHMENTS[i % 2],
  path: `${ATTACHMENTS[i % 2].path}.${i}`,
  name: `2026080-12131${i}-shot.png`
}))

/* A picture larger than any frame it is shown in, for the viewer: everything
   that component does — fitting the whole of it, cropping nothing, scrolling
   nowhere — is only visible on one that does not fit. It is written out as an
   SVG rather than as a third base64 PNG because what is wanted from it is a
   size, and 1200 by 420 pixels written out as pixels would be a kilobyte of
   base64 sitting in this file. What the store actually holds is PNG and JPEG —
   the Rust side sniffs the bytes and names the file after what they are — so
   this one's name says svg rather than pretending otherwise. The greys inside
   it are the picture, not styling: nothing inside a `data:` URL can reach a
   token, which is the same limit `catppuccinIcon.js` carries. */
const WIDE_SVG = `<svg xmlns='http://www.w3.org/2000/svg' width='1200' height='420'>
  <rect width='1200' height='420' fill='#c9d1d2'/>
  <rect x='40' y='40' width='1120' height='60' fill='#6b777c'/>
  <rect x='40' y='140' width='700' height='240' fill='#f4f7f7'/>
  <rect x='780' y='140' width='380' height='240' fill='#a9b4b6'/>
</svg>`
const WIDE_ATTACHMENT = {
  path: '/Users/you/Library/Application Support/com.invisor.smetana/attachments/20260806-121316-wide.svg',
  name: '20260806-121316-wide.svg',
  bytes: WIDE_SVG.length,
  url: `data:image/svg+xml,${encodeURIComponent(WIDE_SVG)}`
}

/* What the toml crate actually produces for a misspelled key, caret line and
   all. Copied from a real failure rather than paraphrased: the run dialog shows
   it verbatim in `pre-wrap`, so the leading spaces and the line breaks are the
   thing being checked here, and a tidied-up one-liner would check nothing. */
const BROKEN_CONFIG = `TOML parse error at line 14, column 1
   |
14 | gate = ["npm test", "npm run build"]
   | ^^^^
unknown field \`gate\`, expected one of \`setup\`, \`gates\`, \`env_files\`
`

/* One Run, varied. Written here rather than imported so the states the bar has
   to draw are visible beside the thing drawing them. */
const runFixture = (state, extra = {}) => ({
  project: '/Users/you/dev/smetana',
  settings: {
    scope: { kind: 'queue' },
    mode: 'auto',
    target_branch: 'staging',
    min_priority: 2,
    max_parallel_tasks: 3,
    live_check: true,
    file_findings: true
  },
  state,
  session: 4,
  batches: 1,
  stopping: false,
  reduced: null,
  ...extra
})

/* A handful of tasks in the shape `DesktopApp.vue` hands the palette them:
   every issue in the project, the merge lock already gone and the status already
   translated into this system's vocabulary. Chosen so that one of each row shape
   is on screen at once — a blocked task, one with a parent, one with work
   waiting on it, one with none of the three, a closed one, and one wearing a
   status long enough to have to ellipsise inside its column. */
const PALETTE_ISSUES = [
  {
    id: 'holiday-curb-bhyv',
    title: 'Remove the date of birth field from the account profile',
    status: 'running',
    parent: null,
    updated_at: '2026-08-21T09:20:00Z'
  },
  {
    id: 'holiday-curb-24db',
    title: 'Drop dateOfBirth from the API contract and from the migration',
    status: 'ready',
    parent: 'holiday-curb-bhyv',
    updated_at: '2026-08-21T08:00:00Z'
  },
  {
    id: 'holiday-curb-3c9d',
    title: 'Hide the age on the public profile',
    status: 'blocked',
    parent: null,
    updated_at: '2026-08-20T19:00:00Z'
  },
  {
    id: 'holiday-curb-b120',
    title: 'Export an agent session to a file',
    status: 'needs-you',
    parent: null,
    updated_at: '2026-08-20T18:10:00Z'
  },
  {
    id: 'holiday-curb-91aa',
    title: 'Retry the payment provider webhooks',
    status: 'ready',
    parent: null,
    updated_at: '2026-08-20T11:00:00Z'
  },
  {
    id: 'holiday-curb-0f31',
    title: 'Clear the personal data out of the analytics exports',
    status: 'done',
    parent: null,
    updated_at: '2026-08-19T15:30:00Z'
  },
  {
    id: 'holiday-curb-77e1',
    title: 'Worktree name collision when one is cut a second time',
    status: 'ready-to-merge',
    parent: 'holiday-curb-epic',
    updated_at: '2026-08-19T09:00:00Z'
  },
  /* The six shapes above are the ones that have to be here. These are here for
     a different reason and it is just as mechanical: the scroll area stops at
     320px, so a list that fits inside it cannot show what the keyboard does when
     it does not — and walking ↑ and ↓ through a list longer than the panel is
     the one interaction in this component with nowhere else to be checked. The
     count is sixteen because compact rows are 22px: twelve overflows the
     comfortable panel and sits inside the compact one, which would have left
     half the check unavailable in half the densities. */
  {
    id: 'holiday-curb-6b04',
    title: 'Name the worktree after the task rather than after the branch',
    status: 'running',
    parent: null,
    updated_at: '2026-08-18T16:40:00Z'
  },
  {
    id: 'holiday-curb-a882',
    title: 'Stop the retry loop doubling the webhook body',
    status: 'failed',
    parent: null,
    updated_at: '2026-08-18T14:05:00Z'
  },
  {
    id: 'holiday-curb-e31f',
    title: 'Split the settings window into tabs',
    status: 'human-check',
    parent: 'holiday-curb-epic',
    updated_at: '2026-08-18T10:30:00Z'
  },
  {
    id: 'holiday-curb-c410',
    title: 'Backfill the consent flag for accounts made before June',
    status: 'ready',
    parent: null,
    updated_at: '2026-08-17T13:15:00Z'
  },
  {
    id: 'holiday-curb-9d77',
    title: 'Cache the provider status page lookups',
    status: 'done',
    parent: null,
    updated_at: '2026-08-17T09:45:00Z'
  },
  {
    id: 'holiday-curb-5e2a',
    title: 'Rotate the analytics export credentials',
    status: 'ready',
    parent: null,
    updated_at: '2026-08-16T18:00:00Z'
  },
  {
    id: 'holiday-curb-42fa',
    title: 'Drop the unused columns from the sessions table',
    status: 'deferred',
    parent: null,
    updated_at: '2026-08-16T11:20:00Z'
  },
  {
    id: 'holiday-curb-8ab3',
    title: 'Report which webhook the retry belonged to',
    status: 'ready',
    parent: null,
    updated_at: '2026-08-15T17:35:00Z'
  },
  {
    id: 'holiday-curb-1c60',
    title: 'Keep the export job from running twice on a restart',
    status: 'awaiting-review',
    parent: null,
    updated_at: '2026-08-15T09:10:00Z'
  }
]

/* The store's dependency maps, in the shape `dependencyEdges` gives them. Two
   tasks wait on `bhyv`, which is what puts `git-fork 2` on its row and a `lock`
   on theirs — and the palette reads these rather than an issue's own counters,
   so a blocker that closes stops blocking here and on the board at the same
   moment. */
const PALETTE_EDGES = {
  blockedBy: new Map([
    ['holiday-curb-3c9d', ['holiday-curb-bhyv']],
    ['holiday-curb-b120', ['holiday-curb-bhyv']]
  ]),
  blocking: new Map([['holiday-curb-bhyv', ['holiday-curb-3c9d', 'holiday-curb-b120']]])
}

/* What an empty query draws. The app keeps three — a watch on the selected task
   writes them — and every fixture is listed here for the one reason the gallery
   exists: every row shape has to be on screen at once to be checkable, and the
   list has to be longer than the panel for the keyboard's scrolling to be
   checkable at all. */
const PALETTE_RECENT = PALETTE_ISSUES.map((issue) => issue.id)

const PALETTE_SOME_RECENT = PALETTE_RECENT.slice(0, 3)

/* The palette is `position: absolute`, the same as every other overlay here, so
   the gallery hands it a box to be absolute inside — exactly what the modals
   above get, and for the same reason.

   Two heights, and the first is deliberately shorter than its own list: sixteen
   fixtures do not fit the 320px the rows are drawn in, and not fitting is the
   whole point of there being sixteen. The frame is tall enough for the panel —
   the input row, the capped scroll area and the legend — and no taller, since a
   frame sized to the list would hide the one behaviour the list was lengthened
   to expose. The state frames below hold three rows each and need no room to
   scroll. */
const paletteFrameStyle = {
  position: 'relative',
  height: '560px',
  border: 'var(--border-w) solid var(--border)',
  overflow: 'hidden'
}

const paletteStateFrameStyle = { ...paletteFrameStyle, height: '340px' }

/* A run's document, shortened. `report.rs` writes the real one and this is the
   same shape — its own `<style>`, its own colours, its own `prefers-color-scheme`
   block — because the point of drawing it here is seeing that the frame hands the
   document the whole box and paints nothing of its own over it.

   The `<script>` is not filler, and what it does had to be chosen with some
   care. It is the one thing the sandbox exists for — `report.rs` writes no
   script, but a report that has been sitting on somebody's disk since last
   night can be hand-edited between then and now — and the gallery is the only
   verification this project has, so a probe whose effect nobody could see would
   be worse than none: it would report success whether or not `sandbox=""` were
   still on the frame. `document.title` was exactly that mistake. It sets the
   *frame's* title, which no browser surfaces to the parent page, so removing
   the attribute entirely would have left this section rendering byte for byte
   the same.

   So the effect is inside the frame and impossible to miss: the script paints
   the document red and replaces it with a banner. Nothing in this app is ever
   red across a whole pane, which is the point — the failure cannot be confused
   with a normal render. The two readings are named in the document itself, so
   whoever checks this next does not have to infer them. */

/* Its stylesheet is a copy of the one `src-tauri/src/runs/report.rs` writes, and
   the copy has to keep that file's *shape*: the palette on a bare `:root`, again
   under `prefers-color-scheme`, and again under `[data-theme]`. That last block is
   the only reason this section changes palette at all: `reportTheme.js` names a
   theme on the root tag and the document's own rules answer it. **Nothing on the
   page moves it** — there is no theme control in this gallery, only `?theme=dark`
   and `?theme=light` on the URL, so the two palettes are checked by loading the
   page twice. A fixture written the old way would sit here light in a dark gallery
   and look like the bug rather than the fix.

   It is a fixture, and its colours are the document's rather than this system's:
   that is what a stand-in for a file another language writes costs, and it is the
   one place in `src/` where such a value is not a token. `report.rs` is still where
   the real ones live, and this copy is checked by eye alongside the app, never
   against it. */
const REPORT_HTML = `<!doctype html><html lang="en"><head><meta charset="utf-8">
<title>Run report</title><style>
:root{color-scheme:light;
--canvas:#eaeeef;--surface-sunken:#e1e6e7;--surface:#f4f7f7;--surface-raised:#ffffff;
--border-subtle:#dde3e3;--border:#c9d1d2;--border-strong:#a9b4b6;
--text-primary:#16201f;--text-secondary:#4a565a;--text-muted:#6b777c;
--text-link:#1f5d8f;--text-link-hover:#123f63;
--focus-ring:#1c6fd0;--selection-bg:#c6dcf0;--scrollbar-thumb:#c2caca;
--status-done-fg:#3f6b54;--status-done-bg:#e6eee9;--status-done-border:#c0d3c8;
--status-needs-you-fg:#8a5405;--status-needs-you-bg:#fbf0da;--status-needs-you-border:#e8ce94;
--attn-loud:#b96a06;--shadow-raised:0 1px 2px rgba(22,32,31,.08)}
@media(prefers-color-scheme:dark){:root:not([data-theme="light"]){color-scheme:dark;
--canvas:#10151a;--surface-sunken:#0c1116;--surface:#161b21;--surface-raised:#1b2229;
--border-subtle:#232b33;--border:#2e3841;--border-strong:#3d4954;
--text-primary:#e3e8ed;--text-secondary:#a8b3bd;--text-muted:#7c8b97;
--text-link:#8fb6e8;--text-link-hover:#b3cef2;
--focus-ring:#5fa8ff;--selection-bg:#2b4560;--scrollbar-thumb:#333e48;
--status-done-fg:#7fa792;--status-done-bg:#16211c;--status-done-border:#2c4136;
--status-needs-you-fg:#f2b03d;--status-needs-you-bg:#2b2010;--status-needs-you-border:#6a4e1b;
--attn-loud:#f2b03d;--shadow-raised:none}}
:root[data-theme="dark"]{color-scheme:dark;
--canvas:#10151a;--surface-sunken:#0c1116;--surface:#161b21;--surface-raised:#1b2229;
--border-subtle:#232b33;--border:#2e3841;--border-strong:#3d4954;
--text-primary:#e3e8ed;--text-secondary:#a8b3bd;--text-muted:#7c8b97;
--text-link:#8fb6e8;--text-link-hover:#b3cef2;
--focus-ring:#5fa8ff;--selection-bg:#2b4560;--scrollbar-thumb:#333e48;
--status-done-fg:#7fa792;--status-done-bg:#16211c;--status-done-border:#2c4136;
--status-needs-you-fg:#f2b03d;--status-needs-you-bg:#2b2010;--status-needs-you-border:#6a4e1b;
--attn-loud:#f2b03d;--shadow-raised:none}
:root[data-theme="light"]{color-scheme:light;
--canvas:#eaeeef;--surface-sunken:#e1e6e7;--surface:#f4f7f7;--surface-raised:#ffffff;
--border-subtle:#dde3e3;--border:#c9d1d2;--border-strong:#a9b4b6;
--text-primary:#16201f;--text-secondary:#4a565a;--text-muted:#6b777c;
--text-link:#1f5d8f;--text-link-hover:#123f63;
--focus-ring:#1c6fd0;--selection-bg:#c6dcf0;--scrollbar-thumb:#c2caca;
--status-done-fg:#3f6b54;--status-done-bg:#e6eee9;--status-done-border:#c0d3c8;
--status-needs-you-fg:#8a5405;--status-needs-you-bg:#fbf0da;--status-needs-you-border:#e8ce94;
--attn-loud:#b96a06;--shadow-raised:0 1px 2px rgba(22,32,31,.08)}
*,*::before,*::after{box-sizing:border-box}
::selection{background:var(--selection-bg)}
::-webkit-scrollbar{width:10px;height:10px}
::-webkit-scrollbar-track{background:transparent}
::-webkit-scrollbar-thumb{background:var(--scrollbar-thumb);border-radius:5px}
body{margin:0;padding:32px 16px 40px;background:var(--canvas);color:var(--text-primary);
font-family:system-ui,-apple-system,"Segoe UI","Noto Sans",Roboto,sans-serif;
font-size:13px;line-height:1.5}
.doc{max-width:52rem;margin:0 auto;display:flex;flex-direction:column;gap:24px}
code{font-family:ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace}
.eyebrow{font-family:ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:10px;letter-spacing:.07em;text-transform:uppercase;color:var(--text-muted);margin:0 0 8px}
h1{font-size:22px;font-weight:600;letter-spacing:-.006em;line-height:1.2;margin:0}
.meta{font-family:ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:12px;color:var(--text-secondary);word-break:break-all;margin:8px 0 0}
.strip{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:8px}
.cell{background:var(--surface-raised);border:1px solid var(--border-subtle);border-radius:4px;
box-shadow:var(--shadow-raised);padding:10px;display:flex;flex-direction:column;gap:4px}
.cell-label{font-family:ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:10px;letter-spacing:.07em;text-transform:uppercase;color:var(--text-muted)}
.cell-n{font-family:ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:22px;font-weight:500;line-height:1.2;color:var(--text-primary)}
.cell-done{color:var(--status-done-fg)}
.cell-loud{color:var(--attn-loud)}
.cell-none{color:var(--text-muted)}
.sec{display:flex;align-items:baseline;gap:8px;border-bottom:1px solid var(--border);
padding-bottom:6px;margin:0 0 -8px;
font-family:ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:10px;letter-spacing:.07em;text-transform:uppercase;font-weight:400;color:var(--text-secondary)}
.sec-n{color:var(--text-muted);letter-spacing:0}
.list{display:flex;flex-direction:column;gap:8px}
.card{background:var(--surface-raised);border:1px solid var(--border-subtle);border-radius:4px;
box-shadow:var(--shadow-raised);padding:16px;display:flex;flex-direction:column;gap:8px}
.card-parked{border-color:var(--status-needs-you-border)}
.card-batch{background:var(--surface);box-shadow:none}
.head{display:flex;align-items:center;gap:8px;flex-wrap:wrap}
.chip{font-family:ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:12px;font-weight:500;background:var(--surface-sunken);border:1px solid var(--border-subtle);
border-radius:3px;padding:1px 6px;white-space:nowrap}
.badge{font-family:ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:11px;border-radius:3px;padding:1px 6px;white-space:nowrap;border:1px solid}
.badge-done{background:var(--status-done-bg);color:var(--status-done-fg);border-color:var(--status-done-border)}
.badge-parked{background:var(--status-needs-you-bg);color:var(--status-needs-you-fg);
border-color:var(--status-needs-you-border)}
.batch-label{font-family:ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:10px;letter-spacing:.07em;text-transform:uppercase;color:var(--text-secondary)}
.right{margin-left:auto;
font-family:ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:11px;color:var(--text-muted)}
h3{margin:0;font-size:15px;font-weight:600;line-height:1.35}
.body{margin:0;color:var(--text-secondary)}
.body code{font-size:12px;color:var(--text-primary)}
.unknown{margin:0;color:var(--text-muted)}
.outcome{margin:0;color:var(--text-secondary)}
.outcome code{font-size:12px}
.held{margin:0;color:var(--status-needs-you-fg)}
.held code{font-size:12px}
.notice{background:var(--surface);border:1px solid var(--border-subtle);border-radius:4px;
padding:16px;color:var(--text-muted);margin:0}
.total{border-top:1px solid var(--border-strong);padding-top:12px;display:flex;align-items:baseline;gap:8px}
.total-label{font-family:ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:10px;letter-spacing:.07em;text-transform:uppercase;color:var(--text-secondary)}
.total-n{margin-left:auto;
font-family:ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:18px;font-weight:500;color:var(--text-primary)}
</style></head><body><div class="doc">
<header><p class="eyebrow">smetana &middot; run report</p><h1>Run report</h1>
<p class="meta">/Users/you/dev/smetana &middot; the ready queue &middot; finished 2026-08-12 14:31</p></header>
<div class="strip">
<div class="cell"><span class="cell-label">closed</span><span class="cell-n cell-done">2</span></div>
<div class="cell"><span class="cell-label">parked</span><span class="cell-n cell-loud">1</span></div>
<div class="cell"><span class="cell-label">batches</span><span class="cell-n">2</span></div>
<div class="cell"><span class="cell-label">total</span><span class="cell-n">2h 14m</span></div>
</div>
<div class="sec"><span>closed</span><span class="sec-n">2</span></div>
<div class="list">
<div class="card"><div class="head"><span class="chip">smetana-qca</span>
<span class="badge badge-done">done</span><span class="right">1h 12m</span></div>
<h3>The run writes its own report</h3>
<p class="body">Wrote <code>runs/report.rs</code> and the document it emits, with tests on both.</p></div>
<div class="card"><div class="head"><span class="chip">smetana-ajr</span>
<span class="badge badge-done">done</span><span class="right">&mdash;</span></div>
<h3>The run report tab</h3><p class="unknown">&mdash;</p></div>
</div>
<div class="sec"><span>parked</span><span class="sec-n">1</span></div>
<div class="list">
<div class="card card-parked"><div class="head"><span class="chip">smetana-rox</span>
<span class="badge badge-parked">needs you</span><span class="right">28m</span></div>
<h3>The report on the design system</h3>
<p class="body">Parked: the handoff asks for a theme switch and <code>sandbox=""</code> forbids one.</p></div>
</div>
<div class="sec"><span>batches</span><span class="sec-n">2</span></div>
<div class="list">
<div class="card card-batch"><div class="head"><span class="batch-label">batch 1</span>
<span class="right">1h 12m</span></div>
<p class="body">Nothing odd, though <code>bd list</code> was slow to answer.</p>
<p class="outcome">The run saw its session exit cleanly.</p></div>
<div class="card card-batch"><div class="head"><span class="batch-label">batch 2</span>
<span class="right">28m</span></div>
<p class="unknown">This batch left no account of itself.</p>
<p class="outcome">The run saw its session end with no exit code at all, which is what a signalled
process leaves.</p>
<p class="held">When this batch ended, its actor still held on the board:
<code>smetana-js4</code>, the merge lock (in_progress), <code>smetana-42v</code>
(ready_to_merge).</p>
<p class="outcome">The run released the merge lock <code>smetana-js4</code>, which
<code>smetana-run-7</code> was still holding: the process group of that session was
gone.</p></div>
</div>
<p class="notice">This document carries a script that would paint the whole page red and replace
everything on it with the words THE SANDBOX FAILED. If that is what you are looking at,
the frame lost its sandbox. If you are reading this report, the script did not run.</p>
<div class="total"><span class="total-label">total</span><span class="total-n">2h 14m</span></div>
</div>
<script>document.body.style.background='red';document.body.innerHTML='<h1>THE SANDBOX FAILED</h1>'<\/script>
</body></html>`

const props = defineProps({
  theme: { type: String, default: 'dark' },
  density: { type: String, default: 'comfortable' }
})

watchEffect(() => {
  const el = document.documentElement
  el.setAttribute('data-theme', props.theme)
  el.setAttribute('data-density', props.density)
})

const text = ref('wt/bd-a1b2')
const prose = ref('The board flashes a card twice when bd moves it, and once when we do.')
const editorText = ref('fn main() {\n    println!("hello");\n}\n')
const editorJs = ref('export function openFile(path, { permanent = false } = {}) {\n  // A single click opens a preview tab.\n  const state = project()\n  return state.openTabs.includes(path)\n}\n')
const editorMd = ref('# Heading\n\nA paragraph with **strong** and *emphasis*, plus a [link](https://example.com).\n\n- an item\n- another item\n')
const editorPlain = ref('no language for this extension\nplain text, no colour\n')
/* A file left mid-merge, and the only fixture on this page whose point is what
   the editor does *to* the text rather than the text itself. Two blocks, because
   git writes conflicts in two shapes: a plain one, and diff3's, which adds the
   common ancestor between `|||||||` and `=======` — that middle section takes no
   ground of its own and its marker is coloured like the other three.

   The same string feeds a `FileEditor` and a `DiffView` below. Two bindings and
   not one ref shared: the editor's copy is typed into, and the diff exists to be
   compared against a fixed HEAD. */
const CONFLICT_TEXT = `export function timeout(config) {
<<<<<<< HEAD
  return config.timeout ?? 30000
=======
  return config.timeout ?? 5000
>>>>>>> fix/timeout-pingdom-alerts
}

export function retries(config) {
<<<<<<< HEAD
  return config.retries ?? 3
||||||| merged common ancestors
  return config.retries ?? 1
=======
  return config.retries ?? 5
>>>>>>> fix/timeout-pingdom-alerts
}
`
/* The same file as HEAD has it: no markers, and the current side's values. What
   this buys is the cascade check — every marker line and both sides are changed
   lines in the working tree's pane, so the merge view paints them with
   `--diff-added-bg` and the conflict grounds have to win over it. */
const CONFLICT_HEAD = `export function timeout(config) {
  return config.timeout ?? 30000
}

export function retries(config) {
  return config.retries ?? 3
}
`
const editorConflict = ref(CONFLICT_TEXT)
/* One line, far wider than the pane, so the pair of editors below shows both
   positions of the Editor tab's word-wrap switch side by side: the same text
   scrolling sideways and wrapped. Two refs of one string rather than one ref
   bound twice — a shared ref would send every character typed in either editor
   through the other's `replaceDoc`, and the pair exists to be compared, so
   poking at one must leave the other where it was. */
const LONG_LINE =
  'const message = "one very long line, wider than any pane on this page, so that the difference between wrapping and scrolling sideways is visible without typing anything"\n'
const editorLongLine = ref(LONG_LINE)
const editorLongLineWrapped = ref(LONG_LINE)

/* The diff's two sides. Written to show every kind of chunk at once — a line
   changed in place, a line added, a line taken away — since which of the three
   is which is the whole of what the colours have to say.

   The third line replaces a word rather than appending to one, and that is the
   fixture's job rather than an idle choice: an insertion marks characters on
   the working tree's side only, so a fixture made of insertions alone leaves the
   HEAD side's intra-line mark undrawn and therefore unchecked. `parse` against
   `read` puts one on each side. */
const diffHead = `pub fn head(dir: &Path) -> Head {
    let git = git_dir(dir);
    let text = fs::read_to_string(git.join("HEAD")).ok();
    Head::parse(text.as_deref())
}
`
const diffWork = `pub fn head(dir: &Path) -> Head {
    // Refs are shared and HEAD is per-worktree.
    let git = git_dir(dir).unwrap_or_default();
    let text = fs::read_to_string(git.join("ORIG_HEAD")).ok();
    Head::read(text.as_deref())
}
`
const diffNew = `notes for the morning
nothing of this is in HEAD yet
`
const choice = ref('running')
/* The branch picker on its own: its three states are reached by clicking, and
   the dialog around it is not what needs looking at. */
const pickedBranch = ref('staging')
const branchIsNew = ref(false)
const groupedBranch = ref('develop')
const narrowBranch = ref('spike/auth')
/* Two lists holding a row that is known and cannot be picked. The settings
   window's agent picker is the one in the app, and a `.vue` file is reachable
   by no test here, so this section is where that row is looked at. The second
   one opens *on* an unavailable value, which is not a fixture whim: the block
   is drawn only in the front end, so a `settings.json` that already holds one
   comes back exactly like this. */
const pickedAgent = ref('claude')
const pickedMode = ref('done')
/* Records for the branch fields below — the shape `target_branches` actually
   answers with, now that git.js passes it straight through. */
const everywhere = (...names) => names.map((name) => ({ name, missing_in: [] }))
const partialBranch = ref('feature/runs-project-config')
const checked = ref(true)
const switched = ref(true)

/* The gallery's tab row is its own: in the app it comes from a store, while
   here we need a fixed set showing all four kinds at once. The glyphs come from
   the rule rather than being written out, so a fixture cannot claim a tab looks
   like something the app would never draw — `stores/tabs.js` calls the same
   function. The diff tab keeps its own: there the glyph says what kind of tab it
   is, not what kind of file.

   A computed and not a plain array: the icon carries a palette rather than a
   token, so flipping the theme has to rebuild it. In the app `tabList` is
   already a computed and gets this for free. */
const galleryTabs = computed(() => [
  { id: 'kanban', kind: 'pinned', label: 'Kanban' },
  { id: 'terminal', kind: 'pinned', label: 'Agent' },
  { id: 'tabs.rs', kind: 'file', label: 'tabs.rs', iconUrl: fileIconUrl('tabs.rs', documentTheme.value), dirty: true },
  { id: 'agent.rs', kind: 'preview', label: 'agent.rs', iconUrl: fileIconUrl('agent.rs', documentTheme.value) },
  { id: 'git.rs', kind: 'diff', label: 'git.rs', icon: 'git-compare' },
  {
    id: 'logo.png',
    kind: 'file',
    label: 'logo.png',
    iconUrl: fileIconUrl('logo.png', documentTheme.value),
    readOnly: true,
    readOnlyHint: 'Binary file — not shown.'
  },
  /* Last, which is where `tabList` puts it — after the files and after the
     diffs, because the order of the file tabs is the person's own and a tab
     nobody remembers has no place inside it. Closable like a file's, captioned
     in words like a pinned one. In the app its id is a zero byte and a session
     number, which nothing here draws: the shape that matters on screen is the
     kind. */
  { id: 'term:1', kind: 'terminal', label: 'Terminal 1', icon: 'terminal' }
])

/* The row is draggable here, which is the only way the gesture can be checked
   at all: no test in this repository reaches a `.vue`. The gallery plays the
   part `settings.json` plays in the app — it holds the order the row was
   dragged into and hands it back — so the sample behaves as the product does,
   Kanban and the Agent tab immovable included. Empty is "never rearranged", and
   `orderTabs` then draws the fixture as written. */
const galleryTabOrder = ref([])

const tabs = computed(() => orderTabs(galleryTabs.value, galleryTabOrder.value))

/* The same row with more tabs than fit across it, which is the only state where
   the arrows at its two ends are drawn at all — they appear on
   `scrollWidth > clientWidth` and on nothing else, so the sample needs both this
   longer list and the narrow container the template puts it in. Its own order
   ref, for the reason the row above has one: a drag that snapped back would read
   as a broken gesture rather than as a sample with nothing storing its order. */
const galleryOverflowTabOrder = ref([])

const overflowTabs = computed(() =>
  orderTabs(
    [
      ...galleryTabs.value,
      ...['main.rs', 'model.rs', 'service.rs', 'commands.rs', 'pty.rs'].map((name) => ({
        id: name,
        kind: 'file',
        label: name,
        iconUrl: fileIconUrl(name, documentTheme.value)
      }))
    ],
    galleryOverflowTabOrder.value
  )
)

/* `FileTree` walks a nested `children` array and draws a folder's contents only
   when `expanded` names it, while `MOCK_TREE` is keyed by directory the way
   `files_list` answers — so the gallery nests the one into the other and opens
   the folder. It was handed the flat root before, which drew six rows and left
   the language icons, the picture and the archive — the half of
   `src/catppuccinIcon.js`'s vocabulary a person is most likely to be looking
   at — visible nowhere in the gallery at all. */
const galleryTree = MOCK_TREE[''].map((node) =>
  node.kind === 'dir' ? { ...node, children: MOCK_TREE[node.path] ?? [] } : node
)
const galleryTreeExpanded = { src: true }

/* A pending cut, for the first of the three trees below. It is what wires the
   whole path together on this page — the record goes in as a prop, the row it
   names is drawn muted, and the tree's own menu offers Paste rather than
   greying it — where the section under this one draws the muted row by hand.
   Two of the three trees are left with no record on purpose, so the greyed
   Paste and the live one are side by side. */
const galleryClipboard = { paths: ['src/agent.rs'], mode: 'cut' }

/* Four of the answers the footer strip has to draw, since a state is not a prop
   somebody can flip on this page: a reading in the middle band with both halves
   in it; one in the top band with a half the harness did not print; an agent
   that does not report this at all; and a fresh week, whose session figure is a
   real `0`. The fifth case — nothing asked yet — needs no fixture at all and is
   the propless strip in the template below.

   The numbers are `claude.rs`'s own fixture output, so the reset strings in the
   hint are shaped exactly as the parser hands them over — the harness's words,
   timezone and all.

   The pair that matters most is the second and the fourth, and they are the two
   directions of one rule: a `null` half draws a dash while the other half draws
   its number, and a real `0` draws as `0%` and never as a dash. */
const galleryUsage = [
  {
    state: 'read',
    agent: 'claude',
    usage: {
      sessionPct: 10,
      sessionReset: 'Aug 7 at 8pm (Europe/Moscow)',
      weekPct: 78,
      weekReset: 'Aug 11 at 5:59pm (Europe/Moscow)'
    },
    band: 'reduced'
  },
  {
    state: 'read',
    agent: 'claude',
    usage: { sessionPct: 92, sessionReset: 'Aug 7 at 8pm (Europe/Moscow)', weekPct: null, weekReset: null },
    band: 'pause'
  },
  { state: 'unsupported', agent: 'codex' },
  {
    state: 'read',
    agent: 'claude',
    usage: { sessionPct: 0, sessionReset: null, weekPct: 3, weekReset: 'Aug 11 at 5:59pm (Europe/Moscow)' },
    band: 'normal'
  }
]

/* Four projects for the rail, one of them without a bd tracker and one whose
   name has no separator in it — `smetana` is the case `monogram` answers by
   taking the first two characters rather than the first letter of two
   segments, and a rail of nothing but hyphenated names would never show it.
   The states are `projectStates`' shape from stores/terminals.js: one waiting,
   one working, and two with nothing going on. */
const galleryProjects = [
  { path: '/Users/you/dev/smetana', name: 'smetana', tracked: true },
  { path: '/Users/you/dev/holiday-curb', name: 'holiday-curb', tracked: true },
  { path: '/Users/you/dev/beads-viewer', name: 'beads-viewer', tracked: true },
  { path: '/Users/you/notes', name: 'notes', tracked: false }
]
const galleryProjectStates = {
  '/Users/you/dev/smetana': { state: 'live', live: 1, loud: 0 },
  '/Users/you/dev/holiday-curb': { state: 'loud', live: 0, loud: 1 },
  '/Users/you/dev/beads-viewer': { state: 'live', live: 2, loud: 0 }
}

/* AgentList reads rows and activeId as props, so a plain local fixture is
   enough here — as it is for TerminalView below, which takes the session it
   draws as a prop too and reaches the store only for the output behind it. */
/* Every caption the store can produce, once each: a run that has taken work
   and one that has not, an edit, a filing, a setup, and a bare agent. That is
   the whole of what `captionOf` in `src/stores/terminals.js` answers, and this
   is the only place all six can be seen side by side — which is what the
   check is for, since prose and issue ids are set in different families and a
   row has to hold both without either one wandering. */
const agentRows = [
  /* A pinned row, first because that is where the panel puts one: pinned rows
     lead the list and nothing may be dragged above them. What is worth looking
     at here is what it does *not* draw — the cross is gone, and the pin stands
     in exactly its box, so pinning a row moves nothing else on it. The way back
     out is `Unpin` in the row's own menu. `orderAgents` is what lifts it in the
     app; this page draws rows rather than the rule, so the order is written out
     here by hand, the way the sessions below are. */
  {
    id: 4242,
    conversation: 'a1b2c3d4-5e6f-4a7b-8c9d-0e1f2a3b4c5d',
    label: 'Editing',
    /* Whether *this session's* harness has a line that clears a conversation.
       A field of the row rather than a prop of the list, since two rows of one
       panel can be on two harnesses now — the Tasks row and the Code row of the
       settings window are free to name different ones. Only the menu reads it;
       nothing on a row draws it. Most of the rows here say `true`, and the one
       below on Codex says `false`, so both sentences of that menu row can be
       read on this page. */
    clearable: true,
    tasks: ['smetana-h7l4'],
    state: 'running',
    elapsed: '26m'
  },
  {
    id: 1,
    conversation: '2b3c4d5e-6f70-4812-9a3b-4c5d6e7f8091',
    label: null,
    clearable: true,
    tasks: ['smetana-42'],
    state: 'needs-you',
    elapsed: '2h 14m'
  },
  /* A run holding several. Also the longest caption the list can be asked to
     draw, and therefore the one that says whether the elapsed time and the
     remove button still have room. */
  /* No conversation id, and that is the truth about a batch rather than a gap
     in the fixture: a run's session records nothing in `.smetana/agents.json`,
     so there is nothing a pin could survive a restart under. Its menu is where
     the refusal `Pin to top — nothing to remember it by` can be read. */
  { id: 2, conversation: null, label: null, clearable: true, tasks: ['smetana-42', 'smetana-9je', 'smetana-hvw'], state: 'running', elapsed: '1h 02m' },
  /* The one row on another harness, and the only reason it is here: its menu
     draws `Clear conversation — this agent cannot do it` where every row above
     it offers the verb. Before roles existed that was a fact about the project
     and every row of this list agreed about it; a session started for the Tasks
     or the Code row can be on its own harness now, so the panel has to be able
     to draw both at once. */
  { id: 3, conversation: '3c4d5e6f-7081-4923-ab4c-5d6e7f809123', label: 'Editing', clearable: false, tasks: ['smetana-8av'], state: 'running', elapsed: '41m' },
  { id: 4, label: 'Creating a task', clearable: true, tasks: [], state: 'running', elapsed: '3m' },
  { id: 5, label: 'Project setup', clearable: true, tasks: [], state: 'done', elapsed: '18m' },
  /* A bare agent, and also a run that has not claimed anything yet: the same
     caption, deliberately — it is an agent, and there is no work to name. */
  { id: 6, label: 'Agent', clearable: true, tasks: [], state: 'ready', elapsed: '2m' },
  /* An agent the worker has not answered about yet: the word in place of a
     time, and a remove button with nothing to remove. Captioned exactly as it
     will be once the session lands, so the handover moves nothing on screen.
     It lasts about a second in the app, which is exactly why it belongs here —
     the only place it can be looked at for longer than that. */
  { id: 'start-1', label: 'Creating a task', clearable: true, tasks: [], state: 'running', elapsed: 'starting', starting: true },
  /* A session the last run of the app left behind, off `.smetana/agents.json`.
     The row that answers "the agent is gone after a restart", and the whole of
     what it is worth is how it is drawn: quiet, so it reads as the project's
     past rather than as something running, with the word in place of a time
     the way a start has one. `done` is the state deliberately — it is what
     `attentionLevel` reads to dim the row, and the same word an ordinary
     session that exited cleanly carries.

     Last in the list because that is where `agentRows` puts these, under the
     live rows: the agents somebody is watching keep the top of the column.
     Beside the row above it is also the pair worth looking at — two rows that
     are neither running nor finished, saying two different things in the same
     slot. */
  {
    id: '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60',
    /* A restored row's id *is* its conversation id — that is what
       `.smetana/agents.json` is keyed by — and the row carries it under both
       names, exactly as `agentRows` builds it. Written out so this page shows
       what the panel really offers on such a row: it can be pinned, which is
       the whole point of a pin, since this is what a pinned agent comes back
       as. */
    conversation: '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60',
    label: 'Editing',
    clearable: true,
    tasks: ['smetana-42'],
    state: 'done',
    elapsed: 'offline',
    restored: true
  }
]

/* Which of the rows above the panel is holding at the top, by conversation id —
   the one the first row carries. The list is what `settings.json` keeps and
   what `AgentList` takes as a prop, so this is that file's own shape. */
const AGENT_PINS = ['a1b2c3d4-5e6f-4a7b-8c9d-0e1f2a3b4c5d']

/* The right column's Sessions tab: Claude Code's own transcripts, as
   `sessions_list` hands them over. A fixed clock rather than `Date.now()`, and
   dates written against it: the row's time label is relative, so a fixture
   dated from the machine's clock would draw a different string every time
   somebody opened this page and there would be nothing to check it against.
   Read down the column and the labels are `4m ago`, `18h ago`, `2d ago`,
   `3w ago`, `5w ago`, `1y ago` — every rung of the ladder except `just now`,
   and that one is left out for want of a slot rather than for want of a way to
   hold it still: the clock above is a constant precisely so any of them can be
   drawn on demand, and a `just now` row would say the same as the four-minute
   one about every other thing on it — and a slot here is a case, as the next
   paragraph spends all six of them.

   Six rows and no two of them the same case: a session Claude Code titled
   itself, whose title and first prompt are two different sentences, subagents
   and none, a title long enough to ellipsise, a session out of a worktree, one
   with no branch, and one with neither title nor last message — the row's two
   fallbacks, which appear nowhere else. Newest first, as the store sorts them: this page draws rows
   rather than the store, so the order is written out here by hand. */
const GALLERY_SESSION_NOW = Date.parse('2026-08-28T12:00:00Z')
const galleryAt = (ms) => new Date(GALLERY_SESSION_NOW - ms).toISOString()
const GALLERY_SESSIONS = [
  {
    id: '3a7e5b10-1c2d-4e3f-9a8b-7c6d5e4f3a2b',
    path: '/Users/you/.claude/projects/-Users-you-dev-smetana/3a7e5b10.jsonl',
    cwd: '/Users/you/dev/smetana',
    cwdExists: true,
    branch: 'develop',
    title: 'Why does the scope bar count dirty files it cannot see',
    firstPrompt: 'Why does the scope bar count dirty files it cannot see',
    lastRole: 'user',
    lastText: 'Leave it for now, file it as a task instead.',
    messages: 1,
    subagents: 0,
    model: 'claude-opus-5',
    modifiedAt: galleryAt(4 * 60 * 1000),
    size: 148_392
  },
  {
    id: '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60',
    path: '/Users/you/.claude/projects/-Users-you-dev-smetana/9f1c0a2e.jsonl',
    cwd: '/Users/you/dev/smetana',
    cwdExists: true,
    branch: 'main',
    /* The row this page opens on load, and it is this one deliberately: Claude
       Code titled the session itself, so the line at the top and the block
       inside say two different things. A card drawn from a row whose title and
       first prompt matched would look right whether it read the right field or
       the wrong one. */
    title: 'Sessions tab reads Claude Code transcripts off disk',
    firstPrompt:
      'Talk to me in Russian: everything you say in this project, and keep the commit messages in Russian too',
    lastRole: 'assistant',
    lastText:
      'Done. The three columns are drawn from the tracker now, and the fixture that used to stand in for the log pane is gone with it.',
    messages: 48,
    subagents: 3,
    model: 'claude-opus-5',
    modifiedAt: galleryAt(18 * 60 * 60 * 1000),
    size: 2_884_016
  },
  {
    id: '5d2f8c41-9b0a-4c1d-8e7f-6a5b4c3d2e1f',
    path: '/Users/you/.claude/projects/-Users-you-dev-smetana--worktrees-smetana-oln/5d2f8c41.jsonl',
    cwd: '/Users/you/dev/smetana/.worktrees/smetana-oln-sessions-tab-disk-history',
    /* The worktree was removed when its task merged and the transcript stayed
       behind, which is what greys this card's two launching buttons — the one
       state of them the other five fixtures cannot show. */
    cwdExists: false,
    branch: 'feature/smetana-oln-sessions-tab-disk-history',
    /* The long one the row has to ellipsise, and a transcript with no generated
       title, so both fields hold the one string — the shape every row had
       before that record was read. */
    title:
      'Implement the front-end half of the sessions tab, the row and the opened card, against the fixtures alone',
    firstPrompt:
      'Implement the front-end half of the sessions tab, the row and the opened card, against the fixtures alone',
    lastRole: 'assistant',
    lastText:
      'Both gates are green. The row draws in all four theme and density combinations; what is left is the pass over the gallery.',
    messages: 214,
    subagents: 1,
    model: 'claude-opus-5',
    modifiedAt: galleryAt(2 * 24 * 60 * 60 * 1000),
    size: 16_402_771
  },
  {
    id: 'c81b0e39-4a5f-4b6c-9d0e-1f2a3b4c5d6e',
    path: '/Users/you/.claude/projects/-Users-you-dev-smetana-src-tauri/c81b0e39.jsonl',
    cwd: '/Users/you/dev/smetana/src-tauri',
    cwdExists: true,
    branch: null,
    title: 'Check whether the sidecar digest matches the pinned release',
    firstPrompt: 'Check whether the sidecar digest matches the pinned release',
    lastRole: 'user',
    lastText: 'It does. Nothing to do.',
    messages: 6,
    subagents: 0,
    model: 'claude-sonnet-4-5',
    modifiedAt: galleryAt(21 * 24 * 60 * 60 * 1000),
    size: 41_508
  },
  {
    id: 'e4a90d77-2b3c-4d5e-8f90-1a2b3c4d5e6f',
    path: '/Users/you/.claude/projects/-Users-you-dev-smetana/e4a90d77.jsonl',
    cwd: '/Users/you/dev/smetana',
    cwdExists: true,
    branch: 'main',
    title: null,
    firstPrompt: null,
    lastRole: null,
    lastText: null,
    messages: 0,
    subagents: 0,
    model: null,
    modifiedAt: galleryAt(40 * 24 * 60 * 60 * 1000),
    size: 0
  },
  {
    id: '7b6a5948-3c2d-4e1f-9a0b-8c7d6e5f4a3b',
    path: '/Users/you/.claude/projects/-Users-you-dev-smetana/7b6a5948.jsonl',
    cwd: '/Users/you/dev/smetana',
    cwdExists: true,
    branch: 'staging',
    title: 'Port the branch list to the design system',
    firstPrompt: 'Port the branch list to the design system',
    lastRole: 'assistant',
    lastText:
      'The rebase glyph is git-graph; lucide ships no rebase mark and that is the one about the shape of the history.',
    messages: 97,
    subagents: 12,
    model: 'claude-opus-5',
    modifiedAt: galleryAt(400 * 24 * 60 * 60 * 1000),
    size: 7_115_240
  }
]

/* This page's own half of what the Sessions tab holds for a card, in the small,
   and it is here for the reason the `copyId` harness above is: the component
   takes `expanded`, `copyState` and `copyNoun` as props and raises `toggle` and
   `action`, so a page that drew it with none of them would exercise exactly the
   collapsed half of it and leave the opened card — which is most of this task —
   verified nowhere. The one verification this project has for anything under
   `src/components/` is this page.

   The second row starts open, so the opened card is on screen without anybody
   having to find and press a chevron in four theme and density combinations.
   The third starts open too, and for the same reason one card further on: its
   working directory is gone, so it is where the refused launching verbs — both
   buttons greyed, with the one reason they share written once under them — can
   be seen without pressing anything.

   `action` is answered with the same copy policy the app keeps — literally the
   same, `useCopyFeedback`, rather than a second writing of it — and with
   nothing at all for the four verbs that reach a desktop or a disk: a gallery
   has neither, and the menu opening, walking and closing is what there is to
   check here. */
const openSessions = ref([GALLERY_SESSIONS[1].id, GALLERY_SESSIONS[2].id])

const toggleSession = (id) => {
  const at = openSessions.value.indexOf(id)
  if (at >= 0) openSessions.value.splice(at, 1)
  else openSessions.value.push(id)
}

const {
  stateFor: sessionCopyStateFor,
  nounFor: sessionCopyNounFor,
  copy: sessionCopyFeedback
} = useCopyFeedback(copyText)

const onSessionAction = ({ kind, session }) => {
  /* Everything that is not a copy is left alone, the two launching verbs among
     them: a gallery has no worker to start a session in, and a press that did
     nothing is the honest answer where a fabricated agent row would not be.
     What is checkable here is the rows and the buttons — that they are drawn,
     greyed and explained — which is the half a `.vue` file keeps to itself. */
  if (!isCopyKind(kind)) return
  return sessionCopyFeedback(session?.id ?? null, copyPayload(kind, session), copyVerbNoun(kind))
}

/* The Git panel's three states, in the shape `src-tauri/src/vcs/` answers with.
   Two repositories rather than one, since a project made of several is what the
   repository list exists for and the case a single-repository machine can never
   show; the second is on a detached HEAD, which the row has to say rather than
   dress up as a branch. */
const REPOS = [
  { name: '.', path: '/Users/you/dev/smetana', branch: 'feat/worktree-rename', detached: null },
  { name: 'admin', path: '/Users/you/dev/smetana/admin', branch: null, detached: 'a1b2c3d' }
]

/* One row of every kind the panel can draw, including the two that are easiest
   to get wrong: a rename, which carries the path it came from, and a conflict,
   which is the one row with a colour of its own. The long path is deliberate —
   it is what says whether the file's own name survives the truncation. */
const CHANGES = [
  { path: 'src/stores/vcs.js', origPath: null, kind: 'modified', staged: false, unstaged: true },
  { path: 'src/components/git/GitPanel.vue', origPath: null, kind: 'added', staged: true, unstaged: false },
  { path: 'src/views/desktopAppData.js', origPath: null, kind: 'deleted', staged: true, unstaged: false },
  {
    path: 'src/components/git/RepoList.vue',
    origPath: 'src/components/shell/RepoList.vue',
    kind: 'renamed',
    staged: true,
    unstaged: false
  },
  { path: 'src/components/files/editor/languages.js', origPath: null, kind: 'typeChanged', staged: false, unstaged: true },
  { path: 'notes.txt', origPath: null, kind: 'untracked', staged: false, unstaged: true },
  /* An untracked *directory*, which is what `--untracked-files=normal` reports
     rather than every file under it: the trailing slash is git's and is kept. */
  { path: 'src/components/git/', origPath: null, kind: 'untracked', staged: false, unstaged: true },
  { path: 'src/stores/tabs.js', origPath: null, kind: 'conflicted', staged: false, unstaged: true }
]

const CLEAN_TREE = { branch: 'main', detached: null, changes: [] }

/* What one branch differs from the current one by, in the shape `vcs_compare`
   answers with — `CompareChange` and not `Change`: between two commits there is
   no staged flag and no untracked file, and two fields that are always false
   are two fields somebody would one day read as an answer.

   Every kind `git diff --name-status` can report is here, including the rename
   that carries the path it came from: that is the row with two paths on it, and
   the one a parser reading a record as a single field puts every row after it
   out of step. The long paths are deliberate, since what they say is whether
   the file's own name survives the truncation. */
const COMPARE_FILES = [
  { path: 'src/stores/vcs.js', origPath: null, kind: 'modified' },
  { path: 'src/components/git/CompareList.vue', origPath: null, kind: 'added' },
  { path: 'src/views/desktopAppData.js', origPath: null, kind: 'deleted' },
  { path: 'src/components/files/editor/languages.js', origPath: null, kind: 'typeChanged' },
  {
    path: 'src/components/git/RepoList.vue',
    origPath: 'src/components/shell/RepoList.vue',
    kind: 'renamed'
  }
]

/* The switch is live here, which is the whole reason it is a ref: pressing a
   position is what says the two labels fit the column and that the pair reads
   as one control rather than as two buttons that happen to be adjacent. */
const compareMode = ref('diverged')

/* Branches in the order `git::by_recency` gives them and the panel draws them:
   what was worked on here most recently first, and the tail a fresh clone
   leaves alphabetical behind it. One is the branch the repository is on, marked
   and not offered as a target. The long name is deliberate — it is what says
   whether a row loses its middle rather than pushing the mark off the end. */
const BRANCHES = [
  { name: 'feat/worktree-rename', current: true },
  { name: 'develop', current: false },
  { name: 'main', current: false },
  { name: 'feature/smetana-8ok-git-panel-branches', current: false },
  { name: 'release/7', current: false }
]

/* The branch-review window: the project's pair at the top, the repositories
   under it, and the one row that differs.

   Four repositories and three rows on purpose. The rule's branch is missing
   from `extension`, which is why that one is out of the review and named in the
   notes block, and from `infra`, which is in the table anyway because somebody
   added it by hand — the row with the `man` badge, a pair of its own and the
   `x` that takes it out again. `infra` also sits outside the project, so it is
   the one row whose path is drawn `~/work/smetana-infra` rather than `./…`.

   The rule's branch is the long one deliberately: `feature/smetana-4nsa-remote-branches-repo`
   has to be readable whole in the field, in the branch list the field opens and
   in a row of the table, which is the whole reason this window was rebuilt. */
const REVIEW_REPOS = [
  { name: '.', path: '/Users/you/dev/smetana' },
  { name: 'admin', path: '/Users/you/dev/smetana/admin' },
  { name: 'extension', path: '/Users/you/dev/smetana/extension' },
  { name: 'infra', path: '/Users/you/work/smetana-infra' }
]
const REVIEW_ROOT = '/Users/you/dev/smetana'
const REVIEW_HOME = '/Users/you'
const REVIEW_HOUR = 3600
const REVIEW_NOW = Math.floor(Date.now() / 1000)
/* `target_branches`' answer, and `missing_in` speaks the names above — the
   project root's is `.`, which is why one entry names it that way. */
const REVIEW_BRANCHES = [
  {
    name: 'feature/smetana-4nsa-remote-branches-repo',
    missing_in: ['extension', 'infra'],
    at: REVIEW_NOW - 2 * REVIEW_HOUR
  },
  { name: 'main', missing_in: [], at: REVIEW_NOW - 26 * REVIEW_HOUR },
  { name: 'develop', missing_in: ['admin'], at: REVIEW_NOW - 3 * 24 * REVIEW_HOUR },
  { name: 'staging', missing_in: [], at: REVIEW_NOW - 40 * 60 },
  { name: 'release/7', missing_in: ['admin', 'extension'], at: REVIEW_NOW - 9 * 24 * REVIEW_HOUR },
  {
    name: 'infra/4nsa-remote-branches',
    missing_in: ['.', 'admin', 'extension'],
    at: REVIEW_NOW - 5 * REVIEW_HOUR
  }
]
const REVIEW_FORM = {
  base: { ref: 'main', remote: false },
  head: { ref: 'feature/smetana-4nsa-remote-branches-repo', remote: false },
  repoIds: [
    '/Users/you/dev/smetana',
    '/Users/you/dev/smetana/admin',
    '/Users/you/work/smetana-infra'
  ],
  overrides: {
    '/Users/you/work/smetana-infra': {
      base: { ref: 'main', remote: true },
      head: { ref: 'infra/4nsa-remote-branches', remote: false }
    }
  },
  manual: ['/Users/you/work/smetana-infra']
}
/* The `New review` door: a pair with nothing on the checked side, no rows at
   all, and a footer that says `0 pairs` over a Review nobody may press. */
const REVIEW_EMPTY = {
  base: { ref: 'main', remote: false },
  head: null,
  repoIds: [],
  overrides: {},
  manual: []
}
/* Deliberately not the local list: `spike/origin-only` has never been checked
   out here, which is what a side reading `origin` is for. */
const REVIEW_REMOTE = {
  '/Users/you/dev/smetana': ['main', 'staging', 'spike/origin-only'],
  '/Users/you/dev/smetana/admin': ['main', 'spike/origin-only']
}
const REVIEW_FETCHED_AT = {
  '/Users/you/dev/smetana': REVIEW_NOW - 2 * 60,
  '/Users/you/dev/smetana/admin': REVIEW_NOW - 3 * REVIEW_HOUR,
  '/Users/you/work/smetana-infra': REVIEW_NOW - 20 * REVIEW_HOUR
}

/* The verdict a live run produces, taken from the rule itself rather than
   written out here: a frame quoting a sentence by hand is a copy that goes on
   reading well long after the rule stopped saying it. */
const RUN_GOING = gitActions([{ token: 1, state: { kind: 'running' } }])

/* What an agent left behind when it parked a task, quoted by `ReadyTaskModal`.
   Two of them and the second one long, because the list is the content of that
   dialog and one short line would show neither the gap between rows nor a
   question wrapping under its own triangle. */
const PARKED_QUESTIONS = [
  'needs a decision on where the strip sits',
  'still waiting on the design call about whether a second run may take a task another run has already claimed and abandoned'
]

/* The Git panel's folds and section heights, kept here so one frame is live:
   the app holds them in `settings.layout` and this stands in for it, which is
   what lets a chevron and a separator actually be tried in the gallery. The two
   handlers are the same two lines `DesktopApp.vue` writes, since the panel is
   presentational on this too — it emits a resolved row count and is told what
   the state became. */
const gitFolds = ref({
  reposRows: null,
  branchRows: null,
  reposOpen: true,
  changesOpen: true,
  branchesOpen: true
})
const GIT_FOLD_KEY = { repos: 'reposOpen', changes: 'changesOpen', branches: 'branchesOpen' }
const GIT_ROWS_KEY = { repos: 'reposRows', branches: 'branchRows' }
const toggleGitSection = (section) => {
  const key = GIT_FOLD_KEY[section]
  if (key) gitFolds.value[key] = !gitFolds.value[key]
}
const resizeGitSection = ({ section, rows }) => {
  const key = GIT_ROWS_KEY[section]
  if (key) gitFolds.value[key] = rows
}

/* Branch names with slashes in them, which is what most repositories are made
   of and what the folders are for. Two prefixes, one of them nested a second
   time, and two names with none — so the frames below show a heading standing
   where its most recent branch stood, `main` staying exactly where it was, and
   a leaf drawn without the prefix every one of its siblings repeats. */
const FOLDER_BRANCHES = [
  { name: 'feature/holiday-curb-y5bt.8-drop-depot-columns', current: true },
  { name: 'main', current: false },
  { name: 'fix/holiday-curb-w78w-warehouse-geocode-precision', current: false },
  { name: 'feature/smetana-8ok.5-branch-folders', current: false },
  { name: 'fix/legacy/depot-import', current: false },
  { name: 'develop', current: false }
]

/* Every state a row can be in against its upstream, keyed by branch name the
   way `vcsState.tracking` holds it: behind, ahead, both at once, level with the
   remote, a branch nobody has pushed (no record at all, like `release/7`) and
   one whose upstream was deleted there. Only the first three draw a mark and
   only the two with something to pull take the colour. */
const TRACKING = {
  'feat/worktree-rename': { upstream: 'origin/feat/worktree-rename', ahead: 0, behind: 3, gone: false },
  develop: { upstream: 'origin/develop', ahead: 2, behind: 0, gone: false },
  main: { upstream: 'origin/main', ahead: 4, behind: 12, gone: false },
  'feature/smetana-8ok-git-panel-branches': {
    upstream: 'origin/feature/smetana-8ok-git-panel-branches',
    ahead: 0,
    behind: 0,
    gone: false
  },
  spike: { upstream: null, ahead: 0, behind: 0, gone: false },
  old: { upstream: 'origin/old', ahead: 0, behind: 0, gone: true }
}

/* The list the two caption buttons are checked against: every name in it has a
   record in `TRACKING`, so which branch a frame is *on* is the whole of what
   changes between them — behind, ahead and behind at once, never pushed, or an
   upstream deleted on the remote. `current` is set per frame rather than here,
   because the pair in the caption is about that one branch and nothing else. */
const REMOTE_BRANCHES = [
  'feat/worktree-rename',
  'develop',
  'main',
  'spike',
  'old',
  'feature/smetana-8ok-git-panel-branches'
]
const onBranch = (name) => REMOTE_BRANCHES.map((branch) => ({ name: branch, current: branch === name }))

/* The same for the folded list, where the point is the heading rather than the
   row: `fix/legacy/depot-import` is behind, and it is inside two folded
   folders, so the bare `↓` has to reach the heading of each. */
const FOLDER_TRACKING = {
  'fix/legacy/depot-import': { upstream: 'origin/fix/legacy/depot-import', ahead: 0, behind: 2, gone: false },
  main: { upstream: 'origin/main', ahead: 1, behind: 0, gone: false }
}

/* Which of them are unfolded, held here the way the app holds it under the
   project. Both start at `null`, which is not the same as an empty list: it
   means nobody has chosen, and the folder holding the current branch opens by
   itself so the tick is on screen. Fold that one away and the list becomes
   empty, which is a choice and stays. */
const branchFolders = ref(null)
const gitFolders = ref(null)
/* Everything folded, which is the state the heading's own mark exists for —
   and live, so unfolding one heading and watching the mark move to the next is
   a thing that can be done here. */
const foldedTracking = ref([])

/* What `origin` holds, against `FOLDER_BRANCHES`: two names the local list also
   has — which the Origin tab draws like every other, marked as having a local
   twin — and three it does not, two of them under folders so the tree is
   checkable here, and one with no slash in it. The order is alphabetical
   because that is how `vcs_remote_branches` answers and nothing on the front
   end re-sorts it.

   Named after the tab it draws rather than "remote", which is taken above by
   the list the caption's two buttons are checked against — a different
   question, and the collision is worth avoiding now that this panel draws real
   remote branches. */
const ORIGIN_BRANCHES = [
  'develop',
  'feature/smetana-xbxc-origin-branches',
  'hotfix/nxc-231',
  'main',
  'spike-origin-only'
]

/* Its folders, unfolded, and live: press `feature` to fold it away. The paths
   carry no group prefix — there is no heading above them any more — which is
   also the difference an old `settings.json` shows, where `origin/feature`
   matches nothing and simply leaves the folder folded. */
const remoteFolders = ref(['feature', 'hotfix'])

/* The shipped default — every folder folded, which is what an empty list means
   here — and live too, so the rows under a run can be reached by pressing a
   heading that a run is deliberately not allowed to refuse. */
const foldedRemote = ref([])

/* Which side of the branch list the live `GitPanel` frame is showing, held here
   the way the app holds it under the project. It starts on Local, which is what
   a project starts on; press `Origin` and the count in the caption above has to
   change with the list under it. */
const gitBranchTab = ref('local')

/* And the same under a run, live too — the claim that frame makes is that the
   row goes on answering while every write in the panel is held, which a frame
   whose press does nothing cannot show. */
const gitRunTab = ref('origin')

/* The branches somebody pinned, against `FOLDER_BRANCHES` — two that live
   inside folders, one that has no slash in it at all, and the branch the
   repository is on, which is marked as well. That last one is the case worth
   looking at: it is one row and not two, first, with the star on it.

   **Written in an order the frames must not draw.** `develop` is last in the
   branch list and first here; `fix/holiday-curb-w78w…` is third there and
   second here. The block at the top has to come out in the branch list's order
   — `fix/holiday-curb…`, then `feature/smetana-8ok.5…`, then `develop` — and a
   fixture whose two orders happened to agree could not show that by eye at all,
   which is what this one used to be.

   `feature/smetana-8ok.5-branch-folders` is here for the other half:
   marking it empties the `feature/` folder, so the frame below can actually
   show a heading that is not drawn because everything under it was lifted.
   `FOLDER_BRANCHES` itself is deliberately untouched — the frame above it is
   the only place the current-branch-behind-a-fold rule can be looked at.

   The list is live, so the menu item on any row moves that row in and out of
   the block at the top. */
const favoriteBranches = ref([
  'develop',
  'fix/holiday-curb-w78w-warehouse-geocode-precision',
  'feature/smetana-8ok.5-branch-folders',
  'feature/holiday-curb-y5bt.8-drop-depot-columns'
])
/* Everything folded, so the frame below shows what the marks do to the tree
   they were lifted out of: `fix` counts one fewer, and the heading a marked
   branch was the whole of is not drawn at all. */
const favoriteFolders = ref([])

/* The name filter's frames, and what each of them is for.

   The query is a constant per frame rather than a live field: what `BranchList`
   is handed is the **answer** — `filterBranches`' hits, built by `GitPanel`
   because the tab labels need both sides' counts anyway — and the field itself
   is the caption of the live panel frame above, which is where it can be typed
   into for real.

   `depot` is two hits in two different folders, one of them marked and one of
   them behind its upstream, so a filtered row is visibly still a row. `fix/le`
   is the case a substring over the *whole* name exists for: the match runs
   across the slash and lands inside the muted prefix, which is the thing to
   look at — the highlight must not change the weight or the colour of what it
   is over, only its ground. */
const FILTER_QUERY = 'depot'
const FILTER_FAVORITE = 'feature/holiday-curb-y5bt.8-drop-depot-columns'
const FILTER_HITS = filterBranches(FOLDER_BRANCHES, FILTER_QUERY, {
  favorites: [FILTER_FAVORITE]
})
const PREFIX_QUERY = 'fix/le'
const PREFIX_HITS = filterBranches(FOLDER_BRANCHES, PREFIX_QUERY)

/* The Origin tab filtered, over the same three local branches the unfiltered
   origin frame uses: `in` finds the branch the repository is on, which keeps
   its tick, and two `origin` has alone, which keep their `cloud`. */
const ORIGIN_LOCAL = [
  { name: 'main', current: true },
  { name: 'develop', current: false },
  { name: 'fix/legacy/depot-import', current: false }
]
const ORIGIN_QUERY = 'in'
const ORIGIN_HITS = filterBranches(originBranches(ORIGIN_BRANCHES, ORIGIN_LOCAL), ORIGIN_QUERY)

/* Nothing matched, which is two different sentences: the other tab has some and
   says how many, or nothing anywhere has them and the query is named back. */
const NO_MATCH_QUERY = 'kickbox'

/* The caption on its own, in all three of the states it can be in: unfolded
   with a count, unfolded without one, and folded — which still carries its
   count, because somebody who folds the branches away is saying they do not
   want to read the list, not that they no longer want to know there are nine
   of them. */
const headerFolds = ref({ withCount: true, bare: true, folded: false, withActions: true })
/* The caption as a field, live in its own frame: the row is a fixture of the
   real one — the whole field is `GitPanel`'s, since it owns the query — and
   what this frame is for is the swap itself, that the fold is unreachable while
   it stands and that the row is still exactly `--row-h` in both densities.

   The second ref is the fixture's half of what says the field has the focus:
   the plate steps up under the caret instead of the input drawing a ring, so
   the frame has to carry the flag the panel carries. `GitPanel.vue`'s
   `fieldStyle` holds the reasoning and the measurements. */
const headerSearch = ref('')
const headerSearchFocused = ref(false)
/* The live commit box's own draft. Empty to start with, since that is the
   state the button's refusal is drawn in. */
const commitDraft = ref('')
/* The field's height, so the separator under it actually moves something here.
   In the app this lives in `settings.json`; a gallery frame holds its own, for
   the reason every other live frame on this page does — the component is
   presentational and a drag that changed nothing would be a control nobody
   could check. */
const commitRows = ref(2)

/* More branches than the branch section's cap, which is the state this
   repository and most others are actually in — and the one that hid git's
   refusal of a checkout below the fold of an inner scroller. A frame with five
   branches cannot show that, so the refusal frame below uses this. */
const LONG_BRANCHES = [
  ...BRANCHES,
  { name: 'feature/smetana-8ok.4-git-panel-merge', current: false },
  { name: 'fix/smetana-qw6-run-settings-shadowed-project', current: false },
  { name: 'staging', current: false }
]

/* What `BranchPicker` is handed: `target_branches`' answer with the branch's
   own last touch in `at`, in epoch seconds. Twelve branches, because the list
   is capped at nine rows and a fixture short enough to fit would say nothing
   about the scrolling or about the highlight being pulled back into view.

   Measured against the clock rather than written as fixed dates: the meta line
   is an age, and a date from the day this page was written would read `2y` by
   the time anybody looks. The long name is the whole point of the component —
   `feature/smetana-4nsa-remote-branches-repo` has to be readable end to end at
   the review window's own 720px, which is what the first frame below is. */
const PICKER_NOW = Math.floor(Date.now() / 1000)
const PICKER_HOUR = 3600
const PICKER_BRANCHES = [
  { name: 'feature/smetana-4nsa-remote-branches-repo', missing_in: [], at: PICKER_NOW - 2 * PICKER_HOUR },
  { name: 'main', missing_in: [], at: PICKER_NOW - 26 * PICKER_HOUR },
  { name: 'develop', missing_in: ['admin'], at: PICKER_NOW - 3 * 24 * PICKER_HOUR },
  { name: 'staging', missing_in: [], at: PICKER_NOW - 40 * 60 },
  { name: 'release/7', missing_in: ['admin', 'extension'], at: PICKER_NOW - 9 * 24 * PICKER_HOUR },
  { name: 'feature/smetana-8ok-git-panel-branches', missing_in: [], at: PICKER_NOW - 5 * PICKER_HOUR },
  { name: 'fix/smetana-qw6-run-settings-shadowed-project', missing_in: ['extension'], at: PICKER_NOW - 60 * 24 * PICKER_HOUR },
  /* One branch with no timestamp at all, which is what a list from before that
     field existed looks like: its meta line has to come out one piece shorter
     and never `NaN`. */
  { name: 'spike/auth', missing_in: ['admin', 'extension'] },
  { name: 'feat/worktree-rename', missing_in: [], at: PICKER_NOW - 11 * PICKER_HOUR },
  { name: 'chore/bump-tauri', missing_in: [], at: PICKER_NOW - 30 * 24 * PICKER_HOUR },
  { name: 'fix/legacy/warehouse-geocode', missing_in: ['admin'], at: PICKER_NOW - 200 * 24 * PICKER_HOUR },
  { name: 'docs/release-notes', missing_in: [], at: PICKER_NOW - 20 }
]
/* Live, so the highlight, the filter and the keyboard can actually be tried:
   click into the field, walk with the arrows and press Enter. */
const pickerChoice = ref({ name: 'main', origin: false })

/* Which side each frame is showing. One ref per frame rather than one shared
   between them: the toggles are a control this page is here to try, and a press
   in one frame moving the two beside it would be a thing the component does not
   do. In the app the same value comes from `layout.branchSide` and a press is
   reported back to the app window; here the frame is the only side there is.

   The second is set to `origin` on purpose, so the origin list is on the page
   without anybody having to press anything: `origin · fetched 2m ago` on every
   row, the cloud lit in the filter row, and — in that frame, which is told no
   fetch time — `origin` and nothing after it. */
const pickerSide = ref('local')
const pickerSideBare = ref('origin')
const pickerSideNarrow = ref('local')

/* git's own sentence, verbatim from a repository where a second worktree held
   the branch — which is exactly what a run's provisioning phase leaves behind,
   and the message that tells somebody why the tick did not move. The `op` is
   what decides the title over it, since one block serves all three writes. */
const CHECKOUT_REFUSED = {
  kind: 'git',
  op: 'checkout',
  message: "fatal: 'develop' is already checked out at '/Users/you/dev/smetana/.worktrees/smetana-8ok.3'"
}

/* The same block for the other two writes: git refused, nothing about the tree
   changed, and the title is the only thing that differs. */
const MERGE_REFUSED = {
  kind: 'git',
  op: 'merge',
  message:
    'error: Your local changes to the following files would be overwritten by merge:\n\tsrc/stores/vcs.js\nPlease commit your changes or stash them before you merge.\nAborting'
}

/* What a merge or a rebase that stopped on conflicts leaves, in the shape
   `stores/vcs.js` records it. Several files, because the number is what
   somebody weighs the two doors with, and a long path because the dialog is
   480px wide. */
const CONFLICT = {
  repo: '/Users/you/dev/smetana',
  op: 'merge',
  ours: 'feat/worktree-rename',
  theirs: 'develop',
  files: [
    'src/stores/vcs.js',
    'src/components/git/BranchList.vue',
    'src-tauri/src/vcs/commands.rs',
    'src/views/desktopAppData.js'
  ]
}

/* Every construct the parser supports, in one issue description: a heading at
   each of the two sizes, a paragraph carrying strong, emphasis, code and both
   link forms, a task list with a nested list under it, a numbered list, a
   quote, a fenced block and a rule. The last paragraph is the invariant on
   screen — a table, a reference link, an HTML tag and a link this app may not
   open are none of them supported, and every character of them is still drawn.

   One constant for two places: the card below looks at the component on its
   own, and `FULL_ISSUE` reads the same text through the inspector, where the
   heading sizes have to hold against the issue title above them. Two fixtures
   would have drifted the first time either was edited. */
const MARKDOWN_SAMPLE = [
  '# Background',
  '',
  'The health notice renders only while the columns are empty, so a version',
  'mismatch is invisible exactly when there are cards — a person is looking at',
  'stale data with nothing to say so.',
  '',
  '## Acceptance criteria',
  '',
  '- [ ] The notice is visible over a board with cards on it',
  '- [x] It never covers the card that needs a human',
  '  - Checked against `--status-needs-you-fg` at both densities',
  '',
  '### What it looks like',
  '',
  'A **quiet** strip above the columns rather than a *replacement* of them,',
  'drawn by `src/components/kanban/KanbanBoard.vue`.',
  '',
  '> The board stays usable while it says the data may be stale.',
  '',
  '1. Read the health',
  '2. Draw the strip',
  '3. Leave the columns alone',
  '',
  '```sh',
  'npm test -- tests/components/kanban/boardView.test.js',
  '```',
  '',
  'See [the design system](https://claude.ai/design) and',
  '<http://localhost:5173/?view=gallery>. A [local note](file:///tmp/run.log)',
  'stays text, and so do an | unsupported | table |, a [reference][ref] and an',
  '<b>HTML tag</b>.',
  '',
  '---',
  '',
  'Filed under smetana-29j.'
].join('\n')

/* This page's own copy of what `DesktopApp.vue` keeps for the id somebody
   clicked, in the small: a card and an inspector raise `copy-id` and take back
   a `copyState`, and neither of them knows a clipboard exists, so the harness
   has to answer them the way the app does. Without it the new prop and the new
   emit would be drawn here and exercised nowhere, and the tooltip would sit on
   `Copy id` for ever — which reads as a broken feature to whoever checks this
   page by eye. It reaches the boards below through the same two hops the app
   uses, `copiedId` and `copyState`, so those are exercised too.

   The policy itself is not written out here at all any more — `useCopyFeedback`
   is the same code the app runs, and that is the whole point of this pair. It
   was the half the hazards list warns about: this page is the only verification
   this project has for anything under `src/components/`, so a copy fixed in the
   app and not here would leave the harness reproducing a defect the product no
   longer has, which by eye is indistinguishable from a real one. It had already
   happened once, over a stranded timer that had to be found in both copies. */
const {
  target: copiedId,
  state: copyState,
  stateFor: copyStateFor,
  copy: copyIdFeedback
} = useCopyFeedback(copyText)

const copyId = (id) => copyIdFeedback(id, id)

/* Two issues in bd's own shape: one that has everything the inspector can
   draw, and one that has almost nothing. The second is the case worth looking
   at — a panel that reads as a form with blank rows is the defect this section
   exists to catch. */
const FULL_ISSUE = {
  id: 'smetana-29j.11',
  title: 'Show the tracker state on a non-empty board too',
  status: 'in_progress',
  /* Markdown, because everything bd stores is: the panel is where a task is
     read before somebody decides to run it, and this is the fixture that shows
     it rendered at the width it is actually read at. */
  description: MARKDOWN_SAMPLE,
  acceptance_criteria: [
    '- [x] The notice is visible over a board with cards on it',
    '- [ ] It never covers the card that needs a human',
    '- [ ] Checked by eye in all four `theme` × `density` combinations'
  ].join('\n'),
  design:
    'A quiet strip above the columns rather than a replacement of them: the board stays usable while it says the data may be stale.',
  // Two lines on purpose: every `bd note` appends, and the panel owes the
  // whole log, latest line included.
  notes:
    'parked: needs a decision on where the strip sits\nparked: still waiting on the design call',
  priority: 1,
  issue_type: 'bug',
  owner: 'merazent@gmail.com',
  // A different value from the owner on purpose: bd emits both keys and they
  // hold two different people (smetana-a5b). This issue is in_progress, so an
  // agent session's actor is what holds it. SPARSE_ISSUE below has neither, which
  // is where the panel drawing no such row is checkable.
  assignee: 'smetana-run-7',
  created_at: '2026-07-28T09:15:00Z',
  created_by: 'flexo',
  started_at: '2026-07-30T11:02:00Z',
  updated_at: '2026-07-31T19:04:52Z',
  closed_at: null,
  close_reason: null,
  comment_count: 3,
  parent: 'smetana-29j',
  labels: ['tracker', 'ui'],
  dependencies: [
    { issue_id: 'smetana-29j.11', depends_on_id: 'smetana-1or', type: 'blocks' },
    { issue_id: 'smetana-29j.11', depends_on_id: 'smetana-29j', type: 'parent-child' }
  ]
}

const SPARSE_ISSUE = {
  id: 'smetana-4tz',
  title: 'Vendor the latin subset of IBM Plex Mono',
  status: 'open',
  updated_at: '2026-08-01T08:30:00Z',
  labels: [],
  dependencies: []
}

/* Two drafts, because the pair of Auto fields is the whole of what can go wrong
   here: Auto arrives as null and has to be drawn as the word rather than as a
   type nobody chose. One has both fields set, the other neither. The parent
   rides along the same split — the first was filed from a card's own menu and
   draws a Follow-up to row, the second from "+ New task" and has none. */
const FULL_DRAFT = {
  text:
    'The log view drops lines once it is past about ten thousand of them, and nothing says so — it just stops scrolling back. It should either keep them or say plainly that it stopped.',
  issueType: 'bug',
  priority: 1,
  parent: 'smetana-3uv'
}
const AUTO_DRAFT = {
  text: 'Vendor the latin subset of IBM Plex Mono so an offline build has a face to set identifiers in.',
  issueType: null,
  priority: null,
  parent: null
}

/* What a run has taken. Three of them, one without a title — the tracker may
   not hold an issue the run claimed, and the id alone is still a row. */
const CLAIMED = [
  { id: 'smetana-42', title: 'Show the tracker state on a non-empty board too' },
  { id: 'smetana-9je', title: 'Deterministic status colours for custom statuses, including the very long ones' },
  { id: 'smetana-hvw', title: null }
]

/* bd's six types plus a custom one, to show both halves of the type palette:
   the three that carry a hue and the neutral set everything else falls into. */
const types = ['bug', 'feature', 'epic', 'task', 'chore', 'decision', 'tech-debt']

/* Reserved statuses plus generated ones, to show both halves of the algorithm.
   `human_check` is here in bd's own spelling and not for symmetry: it is the
   badge the task inspector draws for a card waiting on somebody's eye, and its
   two-letter code is the whole of what tells that status apart from the other
   generated ones. */
const statuses = [
  'blocked', 'ready', 'running', 'needs-you', 'done', 'failed',
  'human_check',
  'awaiting-review', 'needs-triage', 'on-hold', 'shipped'
]

/* The board is here for its one interactive part: the columns are dragged by
   their headers, and alt+left/right moves a focused one. The order lives in a
   ref rather than in the settings — the product stores it per project, and
   there is no project here. Without a consumer for `reorder` a dragged column
   would spring back, which is exactly what a broken drag looks like. */
const boardOrder = ref([])
const boardColumns = computed(() =>
  orderColumns(
    [
      /* Everything unfinished is runnable, including the child of the epic;
         the done one is not. That is the whole of the rule the board applies —
         see `runnableTask`. `bdStatus` rides beside it because a card's menu
         offers to move the issue and bd's own word is what it would write —
         the deferred pair is where the two vocabularies differ, so the submenu
         there appends the status the issue actually holds. */
      { status: 'ready', tasks: [{ id: 'bd-a1b2', title: 'Rename worktree when the branch changes', status: 'ready', bdStatus: 'open', type: 'bug', runnable: true }] },
      { status: 'running', tasks: [{ id: 'bd-3c9d', title: 'Virtualise the log list above 10k lines', status: 'running', bdStatus: 'in_progress', type: 'feature', assignee: { kind: 'agent', name: 'claude-1' }, spawnedFrom: 'bd-7f31', runnable: true }] },
      { status: 'needs-you', tasks: [{ id: 'bd-7f31', title: 'Approve the migration plan', status: 'needs-you', bdStatus: 'open', type: 'epic', needsResponse: true, runnable: true }] },
      { status: 'done', tasks: [{ id: 'bd-12cd', title: 'Bump tauri to 2.1', status: 'done', bdStatus: 'closed', type: 'chore' }] },
      /* Where a run files what it found, and the one column that carries the
         whole-column press. Its cards are not runnable: a run takes only what
         is already open, which is exactly what that button is for. */
      { status: 'deferred', tasks: [
        { id: 'bd-5a10', title: 'Resizer promises arrow keys it does not have', status: 'deferred', bdStatus: 'deferred', type: 'bug', spawnedFrom: 'bd-a1b2' },
        { id: 'bd-5a11', title: 'Vendor the mono subset for offline builds', status: 'deferred', bdStatus: 'deferred', type: 'chore' }
      ] }
    ],
    boardOrder.value
  )
)

/* The same board while runs are going. The board-level prop greys the column
   header's play only, so a card's own grey has to ride the task object — the
   channel DesktopApp uses — and this is the one place in the gallery that
   state is rendered at all: one card carries a task-run reason of its own,
   in the runScopes.js vocabulary. */
const busyBoardColumns = computed(() =>
  boardColumns.value.map((column) => ({
    ...column,
    tasks: column.tasks.map((task) =>
      task.id === 'bd-a1b2'
        ? { ...task, runBlockedReason: 'a run over task bd-a1b2 is already going' }
        : task
    )
  }))
)

/* Every glyph a column header can draw: bd's built-in vocabulary, the two
   reserved statuses no bd column carries but a custom one might, the three
   custom statuses that have a glyph of their own — `ready_to_merge` and
   `human_check` in bd's own spelling, to show `normalizeStatus` doing its half
   — and one on the end with no glyph, for the generic tag. `running` appears
   twice, because the spinner is the count's business: it turns over work and
   stands still over an empty column. */
const columnHeaders = [
  { status: 'ready', count: 4 },
  { status: 'running', count: 2 },
  { status: 'running', count: 0 },
  { status: 'blocked', count: 1 },
  { status: 'deferred', count: 3 },
  { status: 'pinned', count: 1 },
  { status: 'hooked', count: 2 },
  { status: 'needs-you', count: 1 },
  { status: 'done', count: 9 },
  { status: 'failed', count: 0 },
  { status: 'parked', count: 2 },
  { status: 'ready_to_merge', count: 1 },
  { status: 'human_check', count: 3 },
  { status: 'awaiting-review', count: 2 }
]

const menuItems = [
  { type: 'label', label: 'Worktree' },
  { label: 'Open in editor', icon: 'file-code', shortcut: '⏎' },
  { label: 'Copy path', icon: 'copy', shortcut: '⌘C' },
  { type: 'separator' },
  { label: 'Discard worktree', icon: 'x', tone: 'danger' },
  { label: 'Rebase', icon: 'git-branch', disabled: true }
]

/* Built by the rule rather than written out, so the menu drawn here cannot
   drift from the one the board draws. The second is a card with a write in
   flight on it, which is the state every row is greyed in — and it carries the
   longest label the menu can produce, over an id the length bd actually issues
   (a project prefix and a three-character suffix, so eleven), because that
   sentence is what `TaskCard`'s width was measured against. A shorter id here
   would let the width regress without the gallery showing it. */
const CARD_MENU = taskMenuItems({
  bdStatus: 'open',
  runnable: true,
  runBlockedReason: '',
  busy: false
})
/* The done card, which is the only shape of this menu with no play and no edit
   on it at all: the work is merged, so what is offered is a correction to it
   rather than a run over it. */
const DONE_CARD_MENU = taskMenuItems({
  bdStatus: 'closed',
  runnable: false,
  runBlockedReason: '',
  busy: false
})
const BUSY_CARD_MENU = taskMenuItems({
  bdStatus: 'open',
  runnable: true,
  runBlockedReason: 'a run over task smetana-hth is already going',
  busy: true
})
/* The parked card, which is the only shape of this menu with the answer row on
   top and the play under it dead, since `runnableTask` in DesktopApp refuses a
   parked task for the same reason the Ready dialog asks about one. */
const PARKED_CARD_MENU = taskMenuItems({
  bdStatus: 'parked',
  runnable: false,
  runBlockedReason: '',
  busy: false
})

/* The + button's own two rows, from the same module the app reads them from —
   the gallery draws what ships, never a second copy of the words. */

/* TerminalView is handed the session it draws, the way the app's two tab
   branches hand it one: the prop is what it attaches to, and the mock backend
   answers that attach with terminalFixture.js's captured output. Nothing in
   this file touches the terminal store — the panel below it takes its rows and
   its selection as props too. */
const GALLERY_SESSION = 1

/* The settings window's own state lives in that window and reaches it as
   events from the app window; here the tabs are simply driven by local refs, so
   every control is live enough to look at in all four theme x density
   combinations. */
const galleryTheme = ref('system')
const galleryUiFont = ref(13)
const galleryEditorFont = ref(12)
/* The Editor tab's switch, local like the refs above and for the same reason.
   Bound rather than left to its default so it actually moves when pressed. */
const galleryEditorWordWrap = ref(false)
/* The Git tab's two switches, local like the refs above and for the same
   reason: in the app these values come from the main window and go back to it
   as events, and neither end exists here. Bound rather than left to their
   defaults so the switches actually move when pressed — a control that does not
   respond is the one thing this page cannot be used to check. Deliberately not
   both on: the page is where the pair of rows is looked at, and one of each
   shows both positions of a switch side by side. */
const galleryGitAutoFetch = ref(true)
const galleryRemoveWorktrees = ref(false)
/* The two notification sounds, local for the same reason. Deliberately not
   both on a sound: this page is where the pair of rows is looked at, and one of
   each shows the chosen state and the silent one side by side rather than the
   same word twice. */
const galleryRunSound = ref('sound-1')
const galleryNeedsSound = ref('off')
/* The report switch that sits between those two rows, local for the same
   reason, and on because that is what the app ships — this page is where the
   Notifications group is looked at as a whole, and the row wants to be seen in
   the position a person's app will actually be in. */
const galleryShowReport = ref(true)
/* The switch under the two sound rows, local for the same reason and on for the
   one the report switch above it carries: this page is where the Notifications
   group is looked at as a whole, and the row wants to be seen in the position
   the shipped app puts it in. */
const galleryOnlyWhenUnfocused = ref(true)
/* The Startup group's three, local for the same reason. `supported` is deliberately
   `true` here and nowhere else: in the app this row is disabled in every build
   a person can run this page from — a development build says so in its own
   sentence, and `?view=settings` in a browser has no operating system to ask —
   so this is the only place the live control can be looked at at all. The
   disabled state is the one that needs no fixture. */
const galleryAutostartSupported = ref(true)
const galleryAutostartEnabled = ref(false)
const galleryRestoreGeometry = ref(true)
/* The third row of that group, on because that is what the app ships: this page
   is where the group is looked at whole, and the row wants to be seen in the
   position a person's app will actually be in. */
const galleryUpdatesAutoCheck = ref(true)
const galleryAgent = ref('claude')
/* The Models group, and **no two rows alike**, for the reason the languages
   below carry: five rows all showing the same thing would never draw the state
   worth looking at, and a row bound to the wrong pair would read as correct.

   So the four states the group has are all on screen at once. Default names a
   harness and a model, since it always has both. Tasks names the other harness
   and one of its own models, which is what a role that overrode the default
   looks like — and it is the row whose model list has to be the other harness's.
   Code names a harness and no model, the half state. Run lead and Branch review
   are untouched, which is "Same as default" in both fields and the state every
   settings file on a person's disk is in right now. */
const galleryAgentModel = ref('opus')
const galleryAgentRoles = ref({
  tasks: { agent: 'codex', model: 'gpt-5.6-sol' },
  code: { agent: 'claude', model: '' },
  runLead: { agent: '', model: '' },
  reviewBranch: { agent: '', model: '' }
})
/* One edit out of the group, unpacked the way `SettingsWindow.vue` unpacks it —
   the Default row is `null` and lands on the root pair, a role writes its own.
   Written out here rather than left unbound: the one case worth checking by eye
   is choosing a model in an untouched role, which has to fill that role's
   harness in as well, and a cell that dropped the event would show nothing at
   all happening. */
const galleryChooseRole = ({ role, pair }) => {
  if (!role) {
    galleryAgent.value = pair.agent
    galleryAgentModel.value = pair.model
    return
  }
  galleryAgentRoles.value = { ...galleryAgentRoles.value, [role]: pair }
}
/* The Agents tab's three language pickers, and **no two of them alike**. Not
   all on English, because the longest label any of the lists holds is the one
   worth looking at and a tab showing "English" three times would never draw it
   — and not two of them on the same language either, since a row bound to the
   wrong prop would then read as correct here. That is not hypothetical: a
   shared ref in this file has hidden exactly that defect before (38e300a), and
   these four cells are the only verification a `.vue` file gets. */
const galleryAgentLanguage = ref('ru')
const galleryTaskLanguage = ref('zh-Hans')
const galleryCommitLanguage = ref('ja')
const galleryReportLanguage = ref('de')
/* A standing instruction with something in it, since the empty state of this
   field is its placeholder and the filled one is the layout worth checking:
   six lines of somebody's own prose in a column of its own. */
const galleryAgentPrompt = ref('Talk to me briefly. This machine has no Docker.')
/* The subscription block. A reading rather than one of the two empty states:
   those are a sentence each, while this is the shape with a layout to check —
   two rows, the line about what a run would do, and a live Refresh beside the
   heading. `reduced` rather than a comfortable level so that line says
   something other than "a full batch", and the reset strings are the harness's
   own words, timezone and all, exactly as `claude.rs` hands them over.

   The agent is `claude` while the picker above it shows the same, which is the
   ordinary case; the block naming a *different* agent is the substitution
   `agents::pick` makes, and there is nothing in a browser to make it happen. */
const galleryAgentUsage = {
  state: 'read',
  agent: 'claude',
  usage: {
    sessionPct: 10,
    sessionReset: 'Aug 7 at 8pm (Europe/Moscow)',
    weekPct: 78,
    weekReset: 'Aug 11 at 5:59pm (Europe/Moscow)'
  },
  band: 'reduced'
}
/* The other shapes the block takes, and none of them is reachable any other
   way: the mock answers a reading, so `?view=settings&tab=agents` cannot show
   them either, and two of them are what an acceptance criterion is about.
   `unsupported` is the one that changes the layout rather than the words — no
   Refresh at all, so the heading loses the counterweight `space-between` gives
   it — and it is also the one where the heading names an agent the picker
   above it does not. */
const galleryAgentUsageUnsupported = { state: 'unsupported', agent: 'codex' }
const galleryAgentUsageUnreadable = { state: 'unreadable', agent: 'claude' }
/* Half a reading: one of the two lines the harness prints was reworded, so
   Rust sends the week across as `null` rather than as a zero it never read
   (smetana-7rp). One row and the sentence under it, which is the shape worth
   looking at — it is the one that would be a second row saying "This week: 0%
   used" if either half of this went wrong. */
const galleryAgentUsageHalf = {
  state: 'read',
  agent: 'claude',
  usage: {
    sessionPct: 78,
    sessionReset: 'Aug 7 at 8pm (Europe/Moscow)',
    weekPct: null,
    weekReset: null
  },
  band: 'reduced'
}
/* The Kanban tab. Both lists live rather than off, since the interesting shape
   of this tab is a checkbox column that does something — and the fixture board
   deliberately carries a name no column of it matches (`triage`), which is the
   second group, the whole price of storing these lists globally. */
const galleryKanbanColumns = ref('non-empty')
const galleryKanbanAlwaysShow = ref(['ready', 'triage'])
const galleryKanbanInterval = ref('week')
const galleryKanbanUnlimited = ref(['blocked'])
const galleryBoardColumns = ['blocked', 'ready', 'running', 'needs-you', 'done']
/* `attachments_survey`'s answer in Rust's own shape — a store bigger than this
   project's share of it, some of that share in use and some of it not. */
const gallerySurvey = {
  store: { files: 14, bytes: 22 * 1024 * 1024 },
  project: '/Users/you/Projects/smetana',
  board: 'ok',
  kept: { files: 5, bytes: 6 * 1024 * 1024 },
  removable: { files: 6, bytes: 9 * 1024 * 1024 }
}
const galleryEmptySurvey = {
  store: { files: 5, bytes: 6 * 1024 * 1024 },
  project: '/Users/you/Projects/smetana',
  board: 'ok',
  kept: { files: 5, bytes: 6 * 1024 * 1024 },
  removable: { files: 0, bytes: 0 }
}
/* The board unreadable: the counts are zero because nothing may be judged off
   it, and the sentence has to say that rather than let the zero read as a fact
   about somebody's pictures. This is the state the `?view=gallery` harness
   exists for — it cannot be reached in the dev server, where the mock's board
   is always healthy. */
const galleryNoBoardSurvey = {
  store: { files: 14, bytes: 22 * 1024 * 1024 },
  project: '/Users/you/Projects/smetana',
  board: 'error',
  kept: { files: 0, bytes: 0 },
  removable: { files: 0, bytes: 0 }
}
const galleryCleaned = { removed: { files: 6, bytes: 9 * 1024 * 1024 }, failed: 0 }

/* What the bell has to say, built by the rule itself rather than typed out here:
   a card whose prose came from a fixture would go on looking right after the
   sentence in `notifications.js` had changed under it. Both ends of the panel
   are drawn — a card, and the empty answer, which is the state the panel is
   most often in and the one worth checking is not a blank rectangle. */
const galleryNotifications = [
  /* Both sources, in the order the panel puts them in, and each built the same
     way: from a run as the worker would have sent it, so the wording is the
     rule's own. This is the only place a finished run's card can be looked at
     without spending a night on a real one. */
  runNotification({
    token: 7,
    project: '/Users/you/Projects/smetana',
    state: { kind: 'stopped', reason: { kind: 'queue_empty' } },
    summary: {
      seconds: 8040,
      tasks: {
        closed: [
          { id: 'smetana-a1b', title: 'One' },
          { id: 'smetana-c3d', title: 'Two' },
          { id: 'smetana-e5f', title: 'Three' }
        ],
        parked: [{ id: 'smetana-g7h', title: 'Four' }]
      },
      report: '/Users/you/Projects/smetana/.smetana/reports/2026-08-12-143155.html'
    }
  }),
  /* The third source, built the same way and from the one state that produces
     a card: `ready`. Every other state of the update machine answers `null`
     here, which is why there is only one of these — checking and downloading
     are not news, and a failed check belongs on the About tab below. */
  updateNotification({ kind: 'ready', version: '0.2.0' }),
  storageNotification('/Users/you/Projects/smetana', 62 * 1024 * 1024 + 700 * 1024, 50)
]

/* The update machine's six states, in Rust's own shape, for the About tab
   below. Seven entries, because `downloading` is worth looking at twice: a
   server that said how long the body is, and one that did not — the second
   draws a size with no end to measure it against, which is deliberate and would
   otherwise never be seen.

   This is the only place any of them can be looked at. A real one needs a
   signed release on GitHub, a shipped build to run it in (a development build
   refuses to update itself, by design) and a version newer than the one on
   disk. */
const galleryUpdateStates = [
  { kind: 'idle' },
  { kind: 'checking' },
  { kind: 'available', version: '0.2.0', notes: null, date: '2026-08-20' },
  { kind: 'downloading', received: 12 * 1024 * 1024 + 400 * 1024, total: 48 * 1024 * 1024 },
  { kind: 'downloading', received: 12 * 1024 * 1024 + 400 * 1024, total: null },
  { kind: 'ready', version: '0.2.0' },
  { kind: 'failed', message: 'Could not check for updates: the release feed could not be reached.' }
]

/* The run gate, as `updates_install` refuses it — `UpdateError`'s `{kind,
   detail}` exactly, since that is what a rejected `invoke` hands the window.
   The refusal is the whole reason the Install control is never drawn dead: this
   window cannot see a run in a project nobody is looking at, so the only honest
   version of "you cannot install right now" is one that arrives from Rust with
   the projects named. */
const galleryUpdateRefusal = { kind: 'run_live', detail: { projects: 'smetana, holiday-curb' } }

const sectionStyle = {
  display: 'flex', flexDirection: 'column', gap: 'var(--space-5)',
  padding: 'var(--space-6)', borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const headStyle = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)', textTransform: 'uppercase', color: 'var(--text-muted)'
}
const rowStyle = { display: 'flex', alignItems: 'center', gap: 'var(--space-5)', flexWrap: 'wrap' }

/* Both tab rows the app draws with this one component: the left column's three
   and the right column's two. Both are here rather than one of them, because
   the segments divide the row between them — three of them is where the type
   comes closest to the segment's edges, and two is where the fill is widest —
   and the row is drawn at a panel's own width, since a segment stretched across
   the page would be the one thing this component never does in the app. */
const GALLERY_SIDE_TABS = [
  { id: 'files', label: 'Files' },
  { id: 'git', label: 'Git' },
  { id: 'agents', label: 'Agents' }
]
const GALLERY_RIGHT_TABS = [
  { id: 'task', label: 'Task' },
  { id: 'sessions', label: 'Sessions' }
]
const gallerySideTab = ref('git')
const galleryRightTab = ref('sessions')
/* The panel this row lives under, minus nothing: the border is what shows that
   the row's own rule sits on the panel's edge rather than floating over it. */
const segmentedFrameStyle = {
  width: '236px',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-2)',
  background: 'var(--surface)'
}

/* `PointerMenu` draws nothing at all until a secondary click gives it a point
   to hang off, so unlike every other component here it needs somewhere to be
   clicked. The box is the frame; the menu is what opens over it, at the
   pointer, and both branch cases are here because the refused one is where the
   caption above the greyed rows can be read. */
const branchMenu = ref(null)
const refusedBranchMenu = ref(null)
const BRANCH_MENU = branchMenuItems()
const REFUSED_BRANCH_MENU = branchMenuItems({ allowed: false })
/* The same menu over a row that is already marked, which is the only item here
   whose label changes: `Remove from favourites` is the longer of its two
   wordings and is what the 280px width has to hold. */
const MARKED_BRANCH_MENU = branchMenuItems({ favorite: true })
/* A change row's own menu, drawn straight into a `ContextMenu` for the reason
   the branch menu's copy is: the panel itself is behind a right-click, which is
   a gesture an automated pass cannot reliably raise and a person has to
   remember to make, so the rows get a frame that is simply on the page. Live,
   from the rule itself, so they cannot drift from what the menu offers.

   Three and not two, which is one more than "one ordinary and one refused": the
   menu has three refusals with three different sentences, and no single row can
   carry more than one of them at a time. The ordinary one is what the gutter,
   the two separators and the five labels are checked in; the folder is the
   untracked-directory record, where the two rows that need a file go; and the
   third is a deleted file in a repository outside the project, which is the
   only arrangement that draws the other two sentences together. The width is
   `CHANGE_MENU_W` and imported rather than written out, because what it has to
   hold is the longest of those sentences. */
const CHANGE_MENU = changeMenuItems({ path: 'src/stores/vcs.js', kind: 'modified' })
const FOLDER_CHANGE_MENU = changeMenuItems({ path: 'src/components/git/', kind: 'untracked' })
const GONE_CHANGE_MENU = changeMenuItems({
  path: 'src/views/desktopAppData.js',
  kind: 'deleted',
  insideProject: false
})
/* The last group's own two refusals, which none of the three above can show:
   they are about the sixth row alone and they are said in two different ways.
   The conflicted row wears its reason as a suffix, because that fact is about
   the row; a run holding the project puts `frozen`'s sentence in a caption over
   the group, because that fact is about the repository — and the five rows
   above it stay live under it, which is the thing to check here. */
const CONFLICTED_CHANGE_MENU = changeMenuItems({
  path: 'src/stores/tabs.js',
  kind: 'conflicted'
})
const HELD_CHANGE_MENU = changeMenuItems({
  path: 'src/stores/vcs.js',
  kind: 'modified',
  allowed: false
})

/* Room for the tree and room under it. The empty space below the last row is
   what opens the root's menu, and a box the height of its rows has none — at
   320px this fixture overflowed in comfortable density and the second half of
   the gesture could be checked in exactly one of the four combinations. */
const fileMenuBoxStyle = {
  width: '240px',
  height: '420px',
  overflow: 'auto',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-3)'
}
/* The armed Delete, which is the one row in the app that asks a second time in
   place. It cannot be reached in the gallery the way it is reached in the app —
   that takes a secondary click, a first pick, and a panel that stays up — so
   the rows are drawn straight into a `ContextMenu`, which is what `PointerMenu`
   puts inside itself anyway. The unarmed list is beside it, because what has to
   be read here is the difference between the two: one row's words, in the same
   place, in the same tone. */
const FILE_MENU = fileMenuItems({
  target: 'file',
  canAttach: true,
  hasLiveAgent: true,
  canPaste: true
})
const ARMED_FILE_MENU = fileMenuItems({
  target: 'file',
  canAttach: true,
  hasLiveAgent: true,
  canPaste: true,
  confirmingDelete: true
})
/* Paste in each of the two states it is refused in. It is the one row of the
   clipboard group that is greyed rather than absent, and both of its reasons
   are written into the label itself — a row here clips rather than wraps and
   has no tooltip, so the width is what has to hold the longer sentence. The
   second is drawn on a folder because it is the only place it can happen: a
   folder is what a folder can be pasted into. */
const NOTHING_COPIED_FILE_MENU = fileMenuItems({
  target: 'file',
  canAttach: true,
  hasLiveAgent: true
})
const PASTE_INTO_SELF_FILE_MENU = fileMenuItems({
  target: 'dir',
  canAttach: true,
  hasLiveAgent: true,
  pasteReason: 'intoSelf'
})

/* The box the two fixture trees above sit in. It was written inline twice and
   is one constant now that there are two of them: a box that differed between
   the making states and the cut ones would put the two sets of rows at
   different widths, which is the one thing this section is for comparing. */
const fileTreeStatesStyle = {
  width: '240px',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-3)'
}

const menuTargetStyle = {
  display: 'flex', alignItems: 'center', justifyContent: 'center',
  width: '200px', height: 'calc(3 * var(--row-h))',
  border: 'var(--border-w) dashed var(--border-strong)', borderRadius: 'var(--radius-3)',
  color: 'var(--text-muted)', fontSize: 'var(--text-xs)', cursor: 'default'
}
</script>

<template>
  <div :style="{ height: '100vh', overflow: 'auto', background: 'var(--canvas)', color: 'var(--text-primary)' }">
    <section :style="sectionStyle">
      <div :style="headStyle">Buttons</div>
      <div :style="rowStyle">
        <Button variant="primary">Overwrite</Button>
        <Button variant="secondary" icon="git-branch">Pick new name</Button>
        <Button variant="ghost">Cancel</Button>
        <Button variant="danger" icon="triangle-alert">Discard worktree</Button>
        <Button variant="secondary" disabled>Disabled</Button>
        <!-- The same refusal with the reason for it beside it, which is the
             one thing about a disabled button that cannot be checked without
             this frame. `Button` states a refusal with `aria-disabled` rather
             than the native attribute, so it keeps its place in the tab order
             and the hint opens on focus as well as on hover: Tab onto this one
             and the panel is the whole of what says why the press is refused.
             What to check beside that — that Enter, Space and a press all do
             nothing, that a press does not take the focus off whatever had it,
             and that it is drawn exactly like the bare one to its left, which
             is what the appearance is checked against. The label is the Git
             panel's own sentence for a branch with nothing to send. -->
        <Tooltip label="This branch has nothing the remote does not already have.">
          <Button variant="secondary" disabled>Disabled with a reason</Button>
        </Tooltip>
        <Button variant="secondary" size="sm">Small</Button>
        <Button variant="secondary" size="lg">Large</Button>
        <IconButton icon="bell" label="Notifications" />
        <IconButton icon="settings" label="Settings" variant="solid" />
        <IconButton icon="pause" label="Pause" selected />
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Form controls</div>
      <div :style="rowStyle">
        <div :style="{ width: '220px' }">
          <Input v-model="text" mono placeholder="Worktree name">
            <template #prefix><Icon name="search" :size="12" /></template>
          </Input>
        </div>
        <div :style="{ width: '220px' }"><Input model-value="bad name" invalid /></div>
        <div :style="{ width: '260px' }">
          <Textarea v-model="prose" :rows="3" placeholder="What needs doing" />
        </div>
        <!-- Both, side by side, and this is now the only place either sits
             beside the other: Dropdown is what the product draws everywhere,
             and Select is here alone. It is kept because it is not broken and
             because the difference is worth being able to see — the one below
             is painted in tokens, this one is painted by the operating system
             and follows neither the theme nor the app-wide font size. -->
        <Select v-model="choice" :options="['ready', 'running', 'done']" />
        <div :style="{ width: '160px' }">
          <Dropdown v-model="choice" :options="['ready', 'running', 'done']" />
        </div>
        <Checkbox v-model="checked" label="Follow tail" />
        <Checkbox :model-value="false" indeterminate label="Partial" />
        <Switch v-model="switched" label="Compact density" />
        <Tooltip label="Read-only while an agent is working" shortcut="⌘R">
          <Button variant="secondary" size="sm">Hover me</Button>
        </Tooltip>
        <!-- The same panel with a wait in front of it, which is what a column
             header asks for: prose about the thing under the pointer, on a
             surface people cross on the way to something else. Both are here
             because the difference is a behaviour and only a hover shows it. -->
        <Tooltip label="A column's description opens after a wait this long" :delay="2000">
          <Button variant="secondary" size="sm">Hold me</Button>
        </Tooltip>
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Status — reserved and generated</div>
      <div :style="rowStyle">
        <StatusBadge v-for="s in statuses" :key="s" :status="s" />
      </div>
      <div :style="rowStyle">
        <StatusDot v-for="s in statuses" :key="s" :status="s" :size="10" />
      </div>
      <div :style="rowStyle">
        <!-- With ids the hint names the tasks; without them it falls back to
             the count, which is the state a fixture board is in. -->
        <DependencyMark
          :blocked-by="2"
          :blocks="5"
          :blocked-by-ids="['bd-91ac', 'bd-4d2e']"
          spawned-from="bd-7f31"
        />
        <DependencyMark :blocked-by="1" :blocks="3" />
        <DependencySpine state="active" :height="24" />
        <Assignee kind="agent" name="claude-1" />
        <Assignee kind="human" name="you" />
        <Assignee />
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Kanban</div>
      <div :style="rowStyle">
        <TypeBadge v-for="t in types" :key="t" :type="t" />
      </div>
      <!-- The glyph vocabulary of the column headers. The second `running` is
           the one to check: an empty column's glyph must sit perfectly still. -->
      <div :style="{ display: 'flex', flexWrap: 'wrap', gap: 'var(--space-4)' }">
        <div v-for="(col, i) in columnHeaders" :key="i" :style="{ width: '176px' }">
          <ColumnHeader :status="col.status" :count="col.count" :addable="false" />
        </div>
      </div>
      <!-- The header that can move its whole column into the queue, drawn every
           way it can be. The button comes off the count, so an empty column
           carries none — the 0 beside it already says why; which way its arrow
           points is the note on the header that carries the other one. -->
      <div :style="{ display: 'flex', flexWrap: 'wrap', gap: 'var(--space-4)' }">
        <div :style="{ width: '176px' }">
          <ColumnHeader status="deferred" :count="12" :addable="false" promotable @promote="() => {}" />
        </div>
        <div :style="{ width: '176px' }">
          <ColumnHeader status="deferred" :count="0" :addable="false" promotable @promote="() => {}" />
        </div>
        <!-- The same header on a board where the queue has been dragged to the
             left of it: the arrow is the mirrored glyph, everything else about
             the button is the same. -->
        <div :style="{ width: '176px' }">
          <ColumnHeader
            status="deferred"
            :count="12"
            :addable="false"
            promotable
            promote-side="left"
            @promote="() => {}"
          />
        </div>
      </div>
      <!-- The card at the width the board gives it, since the badge shares its
           bottom row with the dependency marks and the assignee. -->
      <div :style="{ display: 'flex', gap: 'var(--space-4)', alignItems: 'flex-start' }">
        <div :style="{ width: '212px' }">
          <TaskCard
            id="bd-a1b2"
            title="Rename worktree when the branch changes"
            status="needs-you"
            bd-status="open"
            type="bug"
            needs-response
            runnable
            :blocks="5"
            :copy-state="copyStateFor('bd-a1b2')"
            @copy-id="copyId"
          />
        </div>
        <div :style="{ width: '212px' }">
          <TaskCard
            id="bd-3c9d"
            title="Virtualise the log list above 10k lines"
            status="running"
            bd-status="in_progress"
            type="feature"
            :blocked-by="2"
            :blocked-by-ids="['bd-91ac', 'bd-4d2e']"
            spawned-from="bd-7f31"
            :assignee="{ kind: 'agent', name: 'claude-1' }"
            :copy-state="copyStateFor('bd-3c9d')"
            @copy-id="copyId"
          />
        </div>
        <div :style="{ width: '212px' }">
          <TaskCard
            id="bd-12cd"
            title="Bump tauri to 2.1"
            status="done"
            bd-status="closed"
            type="chore"
            :copy-state="copyStateFor('bd-12cd')"
            @copy-id="copyId"
          />
        </div>
        <!-- A title with an identifier in it, which is the ordinary case on a
             board an agent files to and the case that has no spaces in it to
             break at. Kept here because it is the only way this stays checkable:
             every other fixture wraps at a space and would pass with the rule
             taken back out. -->
        <div :style="{ width: '212px' }">
          <TaskCard
            id="bd-ybh0"
            title="Fix why check_whether_the_shell_on_this_machine_answers hangs under load"
            status="ready"
            bd-status="open"
            type="bug"
            :copy-state="copyStateFor('bd-ybh0')"
            @copy-id="copyId"
          />
        </div>
        <!-- Runnable, and not runnable now: the menu's Run row stays where it
             was, grey, and carries the reason in its own label. A row that
             vanished while a run was going would look like the board had lost
             the feature. -->
        <div :style="{ width: '212px' }">
          <TaskCard
            id="bd-77e0"
            title="Fold the settings debounce into the store"
            status="ready"
            bd-status="open"
            type="task"
            runnable
            run-blocked-reason="a run over task smetana-hth is already going"
            :copy-state="copyStateFor('bd-77e0')"
            @copy-id="copyId"
          />
        </div>
        <!-- A write in flight on this issue: every row of its menu is greyed,
             and only this card's. -->
        <div :style="{ width: '212px' }">
          <TaskCard
            id="bd-5g1x"
            title="Move the queue gate before the batch"
            status="ready"
            bd-status="pinned"
            type="task"
            runnable
            busy
            :copy-state="copyStateFor('bd-5g1x')"
            @copy-id="copyId"
          />
        </div>
      </div>
      <!-- The board grows to fill its parent, so the harness has to give it one
           with a height. Drag a column by its header, or focus one and press
           alt+left/right; escape abandons a drag. -->
      <div :style="{ display: 'flex', height: '300px', border: 'var(--border-w) solid var(--border)' }">
        <KanbanBoard
          :columns="boardColumns"
          selected-id="bd-3c9d"
          add-to="ready"
          run-from="ready"
          promote-from="deferred"
          :copied-id="copiedId"
          :copy-state="copyState"
          @select="() => {}"
          @add="() => {}"
          @run="() => {}"
          @task-action="() => {}"
          @promote="() => {}"
          @copy-id="copyId"
          @reorder="boardOrder = $event"
        />
      </div>
      <!-- The same board with runs already going in the project: the refusal
           is per scope now, so the column header's play is greyed by the
           board-level prop over a queue run, one card's play by its own
           task-run reason riding the task object, and both carry their
           sentence rather than disappearing. -->
      <div :style="{ display: 'flex', height: '300px', border: 'var(--border-w) solid var(--border)' }">
        <KanbanBoard
          :columns="busyBoardColumns"
          add-to="ready"
          run-from="ready"
          run-blocked-reason="a run over the queue is already going"
          :reorderable="false"
          :copied-id="copiedId"
          :copy-state="copyState"
          @select="() => {}"
          @add="() => {}"
          @run="() => {}"
          @task-action="() => {}"
          @copy-id="copyId"
        />
      </div>
      <!-- A board with no columns to draw, in both of the opposite facts that
           can mean: nothing is connected, and everything is hidden by the view
           settings over a board that is perfectly full. The second is the one
           worth having here — it is the only place its sentence can be read. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)' }">
        <div :style="{ flex: 1, display: 'flex', height: '200px', border: 'var(--border-w) solid var(--border)' }">
          <KanbanBoard :columns="[]" />
        </div>
        <div :style="{ flex: 1, display: 'flex', height: '200px', border: 'var(--border-w) solid var(--border)' }">
          <KanbanBoard :columns="[]" filtered />
        </div>
      </div>
      <!-- Tall enough for the whole dialog, footer included: a frame that
           clips it turns the one harness that would catch a broken modal into
           a picture of the top half. It grew from 400px with the images row
           and from 520px with the Spec/Plan row; adding another row means
           measuring it again.

           What the number is made of, at comfortable, which is the taller of
           the two densities. The dialog itself is about 460px — a ~62px
           header, a body of ~352px (a 5-row textarea at 104, the images block
           at 96 with two thumbnails, two field rows at 44 each, three 12px
           gaps and the 12px bottom padding) and a ~45px footer. The second
           field row is what cost the last 56 of that: a 10px label, a 6px gap
           under it, a 28px control and the 12px gap above the row. On top of
           the dialog sits the scrim's own `paddingTop: 8vh`, and `vh` is the
           window's rather than this box's — 72px in a 900px-tall window, 115px
           in a 1440px one — so the frame has to hold the sum, and the headroom
           here is what keeps the footer on screen on a tall display. -->
      <div :style="{ position: 'relative', height: '640px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <NewTaskModal
          :open="true"
          :attachments="ATTACHMENTS"
          @close="() => {}"
          @submit="() => {}"
          @attach="() => {}"
          @files="() => {}"
          @remove="() => {}"
          @view="() => {}"
        />
      </div>
      <!-- The same dialog with nothing attached and something being dragged
           over the window: the empty state and the invitation are the two
           halves nobody sees together in the app. The same height as the cell
           above rather than the shorter one this state would fit in — the pair
           is read as a pair, and two frames of different heights side by side
           read as a difference in the dialog. -->
      <div :style="{ position: 'relative', height: '640px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <NewTaskModal
          :open="true"
          :dragging="true"
          error="cat.gif is 12582912 bytes; the ceiling is 8388608 bytes"
          @close="() => {}"
          @submit="() => {}"
          @attach="() => {}"
          @files="() => {}"
          @remove="() => {}"
          @view="() => {}"
        />
      </div>
      <!-- The same dialog opened from a card's own menu. The parent's title is
           deliberately a long one: the note is the only line here that can be
           handed arbitrary prose from the board, and a two-line wrap is the
           state to check rather than the tidy one-line case. The subtitle is
           the other half — it names the parent and says nothing about ready,
           which is the parent's to decide and not this dialog's. -->
      <div :style="{ position: 'relative', height: '640px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <NewTaskModal
          :open="true"
          status="ready"
          :parent="{ id: 'smetana-3uv', title: 'done column: cards ordered by the date they were closed, freshest first' }"
          @close="() => {}"
          @submit="() => {}"
          @attach="() => {}"
          @files="() => {}"
          @remove="() => {}"
          @view="() => {}"
        />
      </div>
      <!-- The three things the whole-column confirm can be saying. The middle
           one is the state nobody sees for long in the app and the longest one
           in wall-clock time: twenty issues at two seconds each. The last is
           the only one that reports numbers, and the only one whose footer has
           nothing left to confirm. -->
      <div :style="{ position: 'relative', height: '260px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <PromoteColumnModal :open="true" :count="12" @close="() => {}" @confirm="() => {}" />
      </div>
      <div :style="{ position: 'relative', height: '260px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <PromoteColumnModal :open="true" :count="12" :moved="4" busy @close="() => {}" @confirm="() => {}" />
      </div>
      <div :style="{ position: 'relative', height: '260px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <PromoteColumnModal :open="true" :count="12" :moved="9" :failed="3" @close="() => {}" @confirm="() => {}" />
      </div>
      <!-- Deleting a task, in both of its states: the question, and the second
           or two while bd is answering it, where every way out is dead
           including the cross — a delete that failed has a message to show, and
           it belongs over the dialog that asked. -->
      <div :style="{ position: 'relative', height: '340px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DeleteTaskModal
          :open="true"
          id="smetana-a1b2"
          task-title="stale board: say so over the cards rather than in place of them"
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <div :style="{ position: 'relative', height: '340px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DeleteTaskModal
          :open="true"
          id="smetana-a1b2"
          task-title="stale board: say so over the cards rather than in place of them"
          busy
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <!-- Deleting a Claude Code transcript, which is the one thing in this app
           that unlinks a file the app did not make. Both states again, and the
           record is the 16 MB session out of a worktree: the size is half of
           what a person checks before pressing this, and a dialog drawn over a
           small one would never show what a long path and a large number do to
           the layout. -->
      <div :style="{ position: 'relative', height: '420px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DeleteSessionModal
          :open="true"
          :session="GALLERY_SESSIONS[2]"
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <div :style="{ position: 'relative', height: '420px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DeleteSessionModal
          :open="true"
          :session="GALLERY_SESSIONS[2]"
          busy
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <!-- A parked task on its way back to Ready. Both wordings are here, and
           the empty one is not the tidy case: a task parked by hand carries no
           note, so the dialog has to be worth reading with nothing to quote. -->
      <div :style="{ position: 'relative', height: '420px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <ReadyTaskModal
          :open="true"
          id="smetana-3uv"
          task-title="done column: cards ordered by the date they were closed, freshest first"
          :questions="PARKED_QUESTIONS"
          @close="() => {}"
          @confirm="() => {}"
          @resolve="() => {}"
        />
      </div>
      <div :style="{ position: 'relative', height: '420px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <ReadyTaskModal
          :open="true"
          id="smetana-3uv"
          task-title="done column: cards ordered by the date they were closed, freshest first"
          @close="() => {}"
          @confirm="() => {}"
          @resolve="() => {}"
        />
      </div>
      <!-- Both strips stand in a frame, and the frame is not decoration here:
           it is the bounded box the strip is drawn against, standing in for the
           dialog it sits in. The rule being shown is a ceiling — two rows of
           thumbnails and then it scrolls — and a cell with no bounds would let
           the strip below simply grow to all fourteen, which is the one state
           these two cells exist to tell apart.
           A thumbnail draws nothing here any more. It emits `view` with the
           picture's path and name, and in the app `views/DialogWindow.vue`
           answers by asking the desktop for a window of its own
           (`views/ImageWindow.vue`, whose body is the `ImageViewer` cell far
           below). The handler in these cells is empty, so a click is inert —
           there is no overlay left for this frame to contain. -->
      <div :style="{ position: 'relative', width: '340px', height: '260px', padding: 'var(--space-5)', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <AttachmentStrip :items="ATTACHMENTS" @remove="() => {}" @view="() => {}" />
      </div>
      <!-- Past two rows the strip scrolls instead of growing: nothing bounds
           how many images are attached, and the dialog has no scrolling of its
           own to absorb them. -->
      <div :style="{ position: 'relative', width: '400px', height: '260px', padding: 'var(--space-5)', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <AttachmentStrip :items="MANY_ATTACHMENTS" @remove="() => {}" @view="() => {}" />
      </div>
      <!-- Both wordings: the first run, which promises a file will appear, and
           the second over a file that is already there, which promises the
           opposite — that what it already gets right survives. -->
      <div :style="{ position: 'relative', height: '400px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <SetupProjectModal :open="true" name="holiday-curb" @close="() => {}" @confirm="() => {}" />
      </div>
      <div :style="{ position: 'relative', height: '400px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <SetupProjectModal :open="true" name="holiday-curb" existing @close="() => {}" @confirm="() => {}" />
      </div>
      <!-- The other window about the same file, and the one that changes it
           without starting anything. Deliberately not on its defaults: a form
           showing 2, 3 and 5 with no branch proves nothing about the fields,
           and the branch here is one `branchOptions` had to keep because the
           list no longer holds it. -->
      <div :style="{ position: 'relative', height: '760px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <ProjectSettingsModal
          :open="true"
          :defaults="{
            target_branch: 'release/7',
            min_priority: 1,
            max_parallel_tasks: 6,
            review_passes: 2
          }"
          :branches="everywhere('main', 'staging')"
          @close="() => {}"
          @save="() => {}"
        />
      </div>
      <!-- And the shape a refusal takes: the command's own message under the
           fields, which is what "the file will not parse" looks like when the
           file changed under an open window. -->
      <div :style="{ position: 'relative', height: '760px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <ProjectSettingsModal
          :open="true"
          :defaults="{
            target_branch: 'main',
            min_priority: 0,
            max_parallel_tasks: 16,
            review_passes: 10
          }"
          :branches="everywhere('main', 'staging')"
          error="unknown field `gate` — .smetana/project.toml could not be read"
          @close="() => {}"
          @save="() => {}"
        />
      </div>
      <!-- The same window over a project with no configuration at all, and over
           one whose file will not parse: no fields, no Save, one sentence in
           their place — which is the whole reason the menu item that opens this
           is no longer greyed in either state. The ghost button reads Close
           rather than Cancel here, since there is nothing on screen to undo. -->
      <div :style="{ position: 'relative', height: '400px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <ProjectSettingsModal :open="true" config-state="missing" @close="() => {}" />
      </div>
      <div :style="{ position: 'relative', height: '400px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <ProjectSettingsModal :open="true" config-state="broken" @close="() => {}" />
      </div>
      <!-- Cutting a branch, from a row in the branch list. Live, because the
           line under the field is the half worth looking at: type a space or
           `develop` into it and it says which rule that broke, and the button
           goes dead as it does. The box is held whether or not there is a
           sentence in it, so the checkbox under it does not step up and down as
           somebody types. -->
      <div :style="{ position: 'relative', height: '400px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <NewBranchModal :open="true" from="develop" :branches="BRANCHES" @close="() => {}" @create="() => {}" />
      </div>
      <!-- And the same dialog with a run holding the repository, which is the
           state it can arrive in without being reopened: the button is dead and
           the line under the field carries `gitActions.js`'s own sentence
           instead of anything about the name. -->
      <div :style="{ position: 'relative', height: '400px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <NewBranchModal :open="true" from="feat/worktree-rename" :branches="BRANCHES" :actions="RUN_GOING" @close="() => {}" @create="() => {}" />
      </div>
      <!-- Renaming one, from a row in the branch list. Live, and the field opens
           filled with the whole name and selected — type one character and the
           lot is replaced, which is the gesture this dialog is built around.
           What to check beside that: putting the name back exactly as it was
           kills the Rename button and draws **no** red line, since an unchanged
           name is not a mistake; typing `develop` refuses it as taken, while the
           branch's own name never is. -->
      <div :style="{ position: 'relative', height: '340px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <RenameBranchModal :open="true" from="feat/worktree-rename" :branches="BRANCHES" @close="() => {}" @rename="() => {}" />
      </div>
      <!-- And the same window with a run holding the repository, which is the
           state it can arrive in without being reopened: the button is dead and
           the line under the field carries `gitActions.js`'s own sentence
           instead of anything about the name. -->
      <div :style="{ position: 'relative', height: '340px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <RenameBranchModal :open="true" from="release/7" :branches="BRANCHES" :actions="RUN_GOING" @close="() => {}" @rename="() => {}" />
      </div>
      <!-- Deleting one, which is the only confirm in this app that asks twice.
           The question first: what a branch is and is not, and one Delete.

           The name is long and slashed on purpose — it is the subject of the
           heading and of the line in the body, and a short one would show
           neither wrapping. -->
      <div :style="{ position: 'relative', height: '340px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DeleteBranchModal
          :open="true"
          branch="feature/smetana-8ok-git-panel-branches"
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <!-- The second state, which is the same window after git declined the
           plain delete: the sentence names what is about to be lost and the
           button says `Delete anyway`. What to check is that the two frames are
           the same size and the same shape — this is one window changing what
           it says, not a second dialog — and that the red button is the only
           thing in either of them drawing attention. -->
      <div :style="{ position: 'relative', height: '340px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DeleteBranchModal
          :open="true"
          branch="feature/smetana-8ok-git-panel-branches"
          not-merged
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <!-- And the third: a refusal forcing would repeat, in git's own words, in
           the same mono block under the same failed-red title `GitPanel` draws
           one in. There is no delete button at all here, which is the whole
           point of the state — the only way out is Cancel. -->
      <div :style="{ position: 'relative', height: '400px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DeleteBranchModal
          :open="true"
          branch="feature/smetana-8ok-git-panel-branches"
          refusal="error: Cannot delete branch 'feature/smetana-8ok-git-panel-branches' checked out at '/Users/you/dev/smetana/.worktrees/smetana-8ok'"
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <!-- git working, where every way out is dead including the cross: the
           call can fail and the message belongs over the window that asked. -->
      <div :style="{ position: 'relative', height: '340px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DeleteBranchModal
          :open="true"
          branch="feature/smetana-8ok-git-panel-branches"
          busy
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <!-- Throwing one file's changes away, which is the one thing in the Git
           panel that loses work with nothing to undo it. Three frames, one per
           sentence, because the sentence is the whole of what this window
           decides: what a person loses differs by what git says the change is,
           and a single wording would be telling somebody a file is being
           deleted when it is about to be restored.

           The path is long and nested on purpose — it is in the sentence and
           again in the mono line under it, and a short one would show neither
           wrapping.

           First: an ordinary edit, where the difference from the last commit
           goes and the file stays. -->
      <div :style="{ position: 'relative', height: '340px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DiscardChangeModal
          :open="true"
          path="src/components/git/DiscardChangeModal.vue"
          kind="modified"
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <!-- Second: a path the last commit does not have, which is the only
           sentence in this app that says a file will stop existing. An
           untracked directory record, trailing slash and all, since that is the
           row this reads worst on if the slash is ever dropped. -->
      <div :style="{ position: 'relative', height: '340px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DiscardChangeModal
          :open="true"
          path="src/components/git/"
          kind="untracked"
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <!-- Third: a deleted file, the one row where discarding puts something
           back rather than taking it away. The red button over a sentence about
           restoring is deliberate — the act is still the same act, and the
           colour is about the row it came from. -->
      <div :style="{ position: 'relative', height: '300px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DiscardChangeModal
          :open="true"
          path="src/views/desktopAppData.js"
          kind="deleted"
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <!-- git's refusal, in the same mono block under the same failed-red title
           `GitPanel` and `DeleteBranchModal` draw one in. There is no Discard
           button at all here, which is the whole point of the state: nothing
           forces this, so the only way out is Cancel. -->
      <div :style="{ position: 'relative', height: '400px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DiscardChangeModal
          :open="true"
          path="src/stores/tabs.js"
          kind="modified"
          refusal="error: unable to unlink old 'src/stores/tabs.js': Permission denied"
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <!-- This discard, in flight: every way out is dead including the cross,
           exactly as the delete above it, and the button says what is
           happening. `busy` rides with it, since a discard that is running is
           git working by any reading. -->
      <div :style="{ position: 'relative', height: '300px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DiscardChangeModal
          :open="true"
          path="src/components/git/DiscardChangeModal.vue"
          kind="modified"
          busy
          discarding
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <!-- And the state the two props exist to tell apart: git is working on
           something else — a pull, a commit sitting on somebody's hooks — so
           Discard is refused because the store would refuse it anyway, while
           **Cancel stays live and the cross stays on the frame**. This window
           has no scrim and a write has five minutes to finish; a way out that
           an unrelated operation can take away is the thing to check is not
           happening here. The button reads `Discard` and not `Discarding…`,
           because nothing about this window is running. -->
      <div :style="{ position: 'relative', height: '300px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <DiscardChangeModal
          :open="true"
          path="src/components/git/DiscardChangeModal.vue"
          kind="modified"
          busy
          @close="() => {}"
          @confirm="() => {}"
        />
      </div>
      <!-- Choosing what an agent reviews. Live, because working the window is
           the only way to see what it is: click either side of the pair and the
           branch list opens in place of the table, press the pencil on a row
           and that row keeps a pair of its own, press `undo-2` and it follows
           the project again, open `Add a repository` and the repositories that
           are not in the review are offered with the reason each one is out.

           Three rows out of four repositories on purpose — one that differs,
           one added by hand, one repository left out and named in the notes
           block. The frame is 720 wide because that is what the window is in
           the app, and tall enough for the branch list, which is this window's
           own ceiling: with the list open the table is not drawn at all, so the
           height stops growing there whatever the project is made of.

           The frames below hold the tallest state each one can reach **plus the
           scrim's own `8vh` of top inset**, and both halves of that are why the
           numbers look generous. Outside a dialog window `Modal` places itself
           against the top of the scrim with that gap above it, so a frame sized
           to the dialog alone hides its footer — the busy frame's own subject —
           at every viewport height. And the gap is a share of the *viewport*
           rather than of the frame, so the slack shrinks as the browser grows:
           these three carry enough for a browser about 2000px tall, which is an
           ordinary maximised window on a large display.

           The tallest state is the branch list open over a **filled** form, and
           it is **574** in both of the frames that can reach it, measured at
           comfortable — not the state either one opens on, and not the 524 the
           `New review` frame shows before a branch is picked. Once a branch is
           picked the table fills and the notes block appears, and reopening the
           list there costs the same 50px it costs in the first frame; that
           frame's whole subject is that picking a branch fills the table, so
           the state after the pick is the one to size it for. 574 plus 160 is
           what makes both of them 740.

           The busy frame is the exception at 680, and it is genuinely done:
           every control in it is off, so its list cannot be opened and its 474
           never moves. Compact is shorter everywhere and clears all three. -->
      <div :style="{ position: 'relative', height: '740px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <ReviewChangesDialog
          :open="true"
          :form="REVIEW_FORM"
          :repos="REVIEW_REPOS"
          :root="REVIEW_ROOT"
          :home="REVIEW_HOME"
          :branches="REVIEW_BRANCHES"
          :remote="REVIEW_REMOTE"
          :fetched-at="REVIEW_FETCHED_AT"
          @close="() => {}"
          @submit="() => {}"
          @branch-side="() => {}"
        />
      </div>
      <!-- The same window after Review was pressed: the body recedes, every
           action is off, and the footer says what is being started. It is on
           screen for as long as `git fetch` takes in every repository with an
           `origin` side, which is up to a minute, and it is the only thing
           saying why the button has gone quiet — so it is a state worth being
           able to look at rather than one to catch by timing.

           All three service messages at once, which is the case the notes block
           exists for: three sentences loose under a table read as one
           paragraph. None of them is an error and none of them is red — a fetch
           that failed leaves the review reading the copy of origin already on
           this disk, which is how old an answer is rather than a failure. -->
      <div :style="{ position: 'relative', height: '680px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <ReviewChangesDialog
          :open="true"
          :form="REVIEW_FORM"
          :repos="REVIEW_REPOS"
          :root="REVIEW_ROOT"
          :home="REVIEW_HOME"
          :branches="REVIEW_BRANCHES"
          :remote="REVIEW_REMOTE"
          :fetched-at="REVIEW_FETCHED_AT"
          :fetching="['/Users/you/dev/smetana']"
          :fetch-failed="['/Users/you/dev/smetana/admin']"
          busy
          @close="() => {}"
          @submit="() => {}"
        />
      </div>
      <!-- The second door, `New review`, which knows no branch: the checked
           side is a dashed field asking for one, the table is two rows of
           nothing waiting for it, the footer says `0 pairs` and Review is
           refused. Picking a branch here fills the table — the same rule the
           other door opens with — so this frame is also where that is checked.
           Its caption comes from the app window, which is why it is passed as a
           prop: the OS frame draws it, and a title written into the template
           would be silently overdrawn.

           It opens on the shortest state of the three and is sized like the
           first, which is not a mistake: this is the door where the branch list
           is the first thing anybody touches, and what decides a frame's height
           is the list reopened after a branch has been picked. -->
      <div :style="{ position: 'relative', height: '740px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <ReviewChangesDialog
          :open="true"
          title="New review"
          :form="REVIEW_EMPTY"
          :repos="REVIEW_REPOS"
          :root="REVIEW_ROOT"
          :home="REVIEW_HOME"
          :branches="REVIEW_BRANCHES"
          :remote="REVIEW_REMOTE"
          :fetched-at="REVIEW_FETCHED_AT"
          @close="() => {}"
          @submit="() => {}"
          @branch-side="() => {}"
        />
      </div>
      <!-- Three, because the fields differ between them: solo is offered for a
           single task and refused for a queue, and that is the model's rule
           rather than the dialog's to soften. The second also carries a refusal
           and a project that declares no live check; the third opens in solo,
           where the "How many at once" row is not drawn at all — a state behind
           two clicks is a state nobody checks, so it is on the page like every
           other one.

           The frames are 800px tall rather than 640: the whole dialog has to be
           visible, footer included, at comfortable density, which is the taller
           of the two densities — a frame that clips it turns the one harness
           that would catch a broken modal into a picture of the top half. The
           tallest of the three, measured, leaves about 70px over. Adding a row
           to this dialog means measuring this number again — the blocked live
           check below does not add one: its reason is a tooltip on the switch,
           teleported to the body, and the note under it belongs to the other
           reason and stays absent here. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', flexWrap: 'wrap' }">
        <div :style="{ position: 'relative', width: '480px', height: '800px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <RunModal
            :open="true"
            :scope="{ kind: 'queue' }"
            :count="12"
            :branches="everywhere('main', 'staging', 'feature/runs-project-config')"
            default-branch="staging"
            @close="() => {}"
            @confirm="() => {}"
          />
        </div>
        <div :style="{ position: 'relative', width: '480px', height: '800px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <RunModal
            :open="true"
            :scope="{ kind: 'task', id: 'smetana-9', title: 'Rename the worktree when the branch changes' }"
            :count="1"
            :part-of="{ id: 'smetana-4', title: 'Worktree lifecycle', siblings: 2 }"
            :branches="everywhere('main', 'staging')"
            default-branch="main"
            :live-check-available="false"
            :default-parallel="5"
            error="unknown field `gate` — .smetana/project.toml could not be read"
            @close="() => {}"
            @confirm="() => {}"
            @rescope="() => {}"
          />
        </div>
        <!-- The third also carries the one state the line about Ready has: a
             task standing outside that column, which the run moves there on
             the press and says so beforehand. It is on this frame rather than
             a sixth because this is the shortest of the three — solo draws no
             "How many at once" row — so the sentence costs a line the height
             above already has room for. -->
        <div :style="{ position: 'relative', width: '480px', height: '800px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <RunModal
            :open="true"
            :scope="{ kind: 'task', id: 'smetana-77', title: 'Fold the settings debounce into the store' }"
            task-status="deferred"
            :count="1"
            :branches="everywhere('main', 'staging')"
            default-branch="main"
            :remembered="{ mode: 'solo' }"
            @close="() => {}"
            @confirm="() => {}"
          />
        </div>
        <!-- The damaged configuration, with the parser's own message in it —
             the caret line and its leading spaces are the point, so this
             fixture keeps them. Every field below the notice is disabled and so
             is Run: this dialog says what is wrong and offers nothing that
             would repair it. The way out is "Set up again" in the right-click
             menu on the project's own row, down in the Projects section. -->
        <div :style="{ position: 'relative', width: '480px', height: '800px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <RunModal
            :open="true"
            :count="12"
            :branches="everywhere('main', 'staging')"
            default-branch="staging"
            :config-error="BROKEN_CONFIG"
            @close="() => {}"
            @confirm="() => {}"
          />
        </div>
        <!-- The project wants a browser live check and the machine has nothing
             to drive one with. The switch is off and inactive with no note under
             it — the reason is on the switch, under the pointer, because it is
             about this machine rather than about the project. The frame clips
             nothing that matters: the tooltip is teleported to the body and
             opens over the page. -->
        <div :style="{ position: 'relative', width: '480px', height: '800px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <RunModal
            :open="true"
            :scope="{ kind: 'queue' }"
            :count="12"
            :branches="everywhere('main', 'staging')"
            default-branch="main"
            live-check-blocked="Nothing here can drive a browser: Playwright's MCP server is not in the agent's configuration and the Claude in Chrome extension was not found in a Chrome profile."
            @close="() => {}"
            @confirm="() => {}"
          />
        </div>
      </div>
      <!-- The renderer on its own, at the width it is read at in the app: the
           right column is 320px, and a code block that scrolls there rather
           than widening the panel is the thing to look at. Nothing in it is a
           control — a task item is a glyph, and clicking one does nothing. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start' }">
        <div :style="{ width: '320px' }">
          <Markdown :text="MARKDOWN_SAMPLE" @open="openExternal" />
        </div>
      </div>
      <!-- Two of them: the panel draws only the fields an issue has, so the
           sparse case is a different component to look at, not the same one
           with less in it. The full one carries the same markdown as the card
           above, which is where the heading sizes are checked against the
           title; the sparse one has no prose at all and must look exactly as it
           did before markdown reached this panel. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start' }">
        <div :style="{ width: '320px' }">
          <TaskInspector
            :issue="FULL_ISSUE"
            ui-status="running"
            :copy-state="copyStateFor(FULL_ISSUE.id)"
            @open="openExternal"
            @copy-id="copyId"
          />
        </div>
        <div :style="{ width: '320px' }">
          <TaskInspector
            :issue="SPARSE_ISSUE"
            ui-status="ready"
            :copy-state="copyStateFor(SPARSE_ISSUE.id)"
            @open="openExternal"
            @copy-id="copyId"
          />
        </div>
      </div>
      <!-- The other thing that stands in the inspector's slot: a task an agent
           is filing, which has no id, no status and nothing to act on. Both
           Auto positions are here, because Auto arrives as null and drawing it
           as anything but the word would claim a choice nobody made. Same
           320px, so the two panels can be compared where they actually sit. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start' }">
        <div :style="{ width: '320px' }">
          <DraftInspector :draft="FULL_DRAFT" />
        </div>
        <div :style="{ width: '320px' }">
          <DraftInspector :draft="AUTO_DRAFT" />
        </div>
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Scope bar</div>
      <!-- The bar runs across the top of the app window, so each instance takes
           the whole width of the page rather than sitting in a frame: what it
           has to survive is the name, the search and the two buttons meeting in
           one row, and a box would answer at a width nobody uses.

           What it no longer has to survive is the project's own state: the
           headline, the counters and the run segments are the status footer's
           now, and their states are drawn under Shell below. What is left here
           is the name of the place, so the instances are about the shapes that
           name can take.

           The bell's singular is hover-only, being a tooltip: point at the bell
           in the third bar for "1 notification". -->
      <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }">
        <!-- A worktree with a branch checked out in it: `scopeName` draws the
             worktree, and the branch follows it after an @. A count on the
             bell. -->
        <ScopeIndicator
          repo="smetana"
          worktree="smetana-f69-scope-indicator"
          branch="feature/smetana-f69-scope-indicator"
          :notifications="2"
        />
        <!-- No worktree, which is what the app itself passes today: `scopeName`
             falls back to the branch and there is no @ segment after it. -->
        <ScopeIndicator
          repo="holiday-curb"
          branch="develop"
        />
        <!-- One notification, the one place in this bar there is a noun to get
             wrong. -->
        <ScopeIndicator
          repo="smetana"
          branch="main"
          :notifications="1"
        />
        <!-- A bell with no badge on it, and a branch name long enough to be the
             thing that gives way when the window is narrowed. -->
        <ScopeIndicator
          repo="smetana"
          branch="feature/smetana-f0bf-project-state-to-footer"
          :notifications="0"
        />
      </div>

      <!-- The three states of the window's own chrome, which decide what this
           bar has to do about the title bar it now is. In the app the state is
           an attribute on the document root, written by `paintRoot` from what
           Rust answers; a browser is always `none`, which is why the first of
           these is every bar above and the other two need saying.

           The attribute is put on a wrapper here rather than on the root, and
           that is the whole trick that makes `traffic-lights` visible on a
           machine that is not a Mac: the tokens are declared against
           `[data-window-chrome=…]`, which matches any element, and custom
           properties inherit. `data-density` rides along on the same wrapper
           because the compact floor is a compound selector and needs both
           attributes on one element to match. -->
      <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }">
        <!-- macOS: the real traffic lights are drawn by the system over the
             left end of this bar, so the bar clears 78px for them. The repo
             name has to start clear of the lights in every density and at every
             font size — that is what this instance is here to show. -->
        <div :data-density="density" data-window-chrome="traffic-lights">
          <ScopeIndicator
            repo="smetana"
            branch="main"
            window-chrome="traffic-lights"
            :notifications="1"
          />
        </div>
        <!-- Windows and Linux: no decorations at all, so the bar draws the
             three buttons itself, after the gear. -->
        <ScopeIndicator
          repo="smetana"
          branch="main"
          window-chrome="buttons"
          :notifications="1"
        />
        <!-- The same bar over a maximized window: the middle button alone
             changes, to `copy` and "Restore". -->
        <ScopeIndicator
          repo="smetana"
          branch="main"
          window-chrome="buttons"
          maximized
          :notifications="1"
        />
      </div>

      <!-- The buttons on their own, both ways round, which is the only place
           the two glyphs sit near enough to compare. -->
      <div :style="{ display: 'flex', alignItems: 'center', gap: 'var(--space-5)' }">
        <WindowControls />
        <WindowControls maximized />
      </div>

      <!-- What the bar keeps of the search: a button saying the search exists
           and which key opens it, in the slot it actually occupies — immediately
           left of the bell. Hovering steps the surface up and changes neither
           the colour nor the position, so a bar this dense cannot twitch. -->
      <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }">
        <ScopeIndicator
          repo="smetana"
          branch="feature/smetana-mht-command-palette"
          :notifications="1"
        >
          <template #search>
            <TaskSearchButton />
          </template>
        </ScopeIndicator>
      </div>

      <!-- The palette, open, with an empty query — which is the `Recent`
           screen. Every row shape is here at once and each is worth looking at:
           `bhyv` carries `git-fork 2` for the two tasks waiting on it, `3c9d`
           and `b120` carry the `lock` naming it, `24db` carries its parent,
           `91aa` carries nothing, `0f31` is dimmed for being closed, and `77e1`
           wears a status too long for its column and ellipsises inside it
           instead of pushing the dot off the row.

           The list is deliberately longer than the 320px it is drawn in, which
           is what makes the keyboard checkable: hold ↓ to the bottom and the
           selected row stays in view, ↑ from the first row wraps to the last and
           scrolls there, and the heading above the list never covers the row
           that was just scrolled to — it is outside the scroll area for exactly
           that reason.

           It is live, and typing is how the rest of it is checked: type `date`
           for the text matches under `Matching text` and the counter beside the
           field; type `zzz` for the empty state, which is a block that appears
           where the heading was rather than under it — there is never more than
           one heading on this panel. `⌘⏎` puts the `meaning` chip in the field
           and the mode row at the bottom into its second wording; nothing
           answers here, since the agent is the app's and not the gallery's. ↑
           and ↓ wrap at both ends, the mouse moves the same one highlight, and
           `esc` closes. -->
      <div :style="paletteFrameStyle">
        <CommandPalette
          open
          :issues="PALETTE_ISSUES"
          :edges="PALETTE_EDGES"
          :recent="PALETTE_RECENT"
        />
      </div>

      <!-- The three screens the meaning tier has, and the reason the heading
           follows the answer rather than the mode.

           The first is the wait, and it says so twice: the counter at the end
           of the field is a spinner, and where the heading would stand there is
           a waiting row — the same spinner, the words, and a count of seconds
           climbing once a second, since the agent has ninety of them and only a
           number that moves tells a slow answer from a hung one. Type `date`
           and the rows under it stay the text matches, which is the point: the
           heading does not become `By meaning` before there is a meaning, and
           `⏎` opens a text match without waiting for the agent at all. Type
           `zzzqqq` into the same frame for the other half of it — the waiting
           row stands over an empty list, where `Nothing matched` would be an
           answer nobody has given yet. The second is an answer: type `date` and
           the heading becomes `By meaning` with the agent's own two ids in its
           own order. The last is a refusal, standing where the empty state
           would, in the failed colour and in the words `OneshotError` wrote —
           the handoff draws no error state at all, which is a hole rather than
           a decision.

           `answered` with no ids at all is the one between them, and it is the
           one worth looking at hardest: the agent looked and named nothing,
           which is a legitimate answer and gets a sentence of its own rather
           than the text mode's, because nobody checked any substrings. Type
           `date` into it. -->
      <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }">
        <div :style="paletteStateFrameStyle">
          <CommandPalette
            open
            pending
            :issues="PALETTE_ISSUES"
            :edges="PALETTE_EDGES"
            :recent="PALETTE_SOME_RECENT"
          />
        </div>
        <div :style="paletteStateFrameStyle">
          <CommandPalette
            open
            answered
            :issues="PALETTE_ISSUES"
            :edges="PALETTE_EDGES"
            :recent="PALETTE_SOME_RECENT"
            :semantic-ids="['holiday-curb-0f31', 'holiday-curb-bhyv']"
          />
        </div>
        <div :style="paletteStateFrameStyle">
          <CommandPalette
            open
            answered
            :issues="PALETTE_ISSUES"
            :edges="PALETTE_EDGES"
            :recent="PALETTE_SOME_RECENT"
          />
        </div>
        <div :style="paletteStateFrameStyle">
          <CommandPalette
            open
            :issues="PALETTE_ISSUES"
            :edges="PALETTE_EDGES"
            :recent="PALETTE_SOME_RECENT"
            error="Smetana looked for claude on your PATH and found nothing."
          />
        </div>
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Shell</div>
      <TabBar :tabs="tabs" active-id="kanban" @reorder="galleryTabOrder = $event">
        <!-- The row's second slot, inside the scrolling strip and right after
             the pinned tabs, which is where the app puts it: the control is
             about those tabs and has to stay beside them however many files are
             open. -->
        <template #afterPinned>
          <MenuButton icon="plus" label="New task, agent, terminal or review" :items="NEW_TAB_ITEMS" :width="180" />
        </template>
      </TabBar>

      <!-- The row again, in a container too narrow to hold it. The strip's own
           scrollbar is hidden, so these two arrows are the only way to reach a
           tab that has slid past an edge; at either end the arrow of that end is
           disabled rather than taken away, since a row that changed width as it
           scrolled would move the tabs out from under the pointer. -->
      <div :style="{ width: '360px', maxWidth: '100%' }">
        <TabBar
          :tabs="overflowTabs"
          active-id="main.rs"
          @reorder="galleryOverflowTabOrder = $event"
        />
      </div>

      <!-- The other tab row, and the one that is not a tab row of files: the
           segmented strip under a side panel's header, drawn here at the width
           a panel gives it. Live rather than fixed, since the fill under the
           active segment and the fill under the pointer are the whole of what
           it draws — press one, and hover the other. **Hover the light theme
           especially**: the segment stands on the group's sunken ground, so
           its hover is a lift to `--surface` rather than the `--surface-hover`
           written for a control on `--surface`, and that token over this ground
           was three units of grey and read as nothing at all.

           What the row is, since it is the design handoff's and every part of
           it is load-bearing: one bordered, rounded group on `--surface-sunken`
           with the segments flush inside it, a hairline between neighbours and
           none after the last, and the selected segment a fill edge to edge
           with no radius of its own — its outer corners are the group's, so
           check them against the group's own rounding rather than for a plate
           standing inside the box. Sentence case in sans; the row sets nothing
           in uppercase and nothing in mono. A segment's mono half is its
           optional count, which only the Git panel's tabs carry and only while
           the filter is on — type into the branch filter in the frames further
           down this page to see one, and check there that the figure is mono
           against a sans word.

           It is a `tablist` and the keyboard is the other half of what to check
           here: one Tab reaches the selected segment and one more leaves the row
           altogether, and the left and right arrows walk it, wrapping at both
           ends and taking the focus with the choice. The ring is the
           stylesheet's own, pulled inside the segment because the group clips
           whatever hangs over its edge, so what to look for is **four whole
           sides** — over the fill of the selected segment and over the sunken
           ground of an unselected one, in both themes. -->
      <div :style="rowStyle">
        <div :style="segmentedFrameStyle">
          <SegmentedTabs v-model="gallerySideTab" :tabs="GALLERY_SIDE_TABS" />
        </div>
        <div :style="segmentedFrameStyle">
          <SegmentedTabs v-model="galleryRightTab" :tabs="GALLERY_RIGHT_TABS" />
        </div>
      </div>

      <!-- Taller than the other boxes on this page, and the file tree is why:
           at 160px the shell showed five rows of it, so half the tree's glyph
           vocabulary sat below a fold in the one place it can be checked. -->
      <div :style="{ height: '320px', border: 'var(--border-w) solid var(--border)' }">
        <AppShell :height="320" :left-width="180" :right-width="180">
          <template #left>
            <Panel title="Files" side="left">
              <FileTree
                :nodes="galleryTree"
                :expanded="galleryTreeExpanded"
                selected-path="Cargo.toml"
              />
            </Panel>
          </template>
          <template #center>
            <div :style="{ padding: 'var(--panel-pad)', fontSize: 'var(--text-sm)' }">Centre</div>
          </template>
          <template #right><Panel title="Task" side="right" collapsed /></template>
          <!-- The shell's second bar slot, and the only place on this page
               where the strip is seen where it actually lives: under the three
               columns, across the whole shell, outside their resizers. -->
          <template #footer><StatusFooter :usage="galleryUsage[0]" /></template>
        </AppShell>
      </div>

      <!-- The left end of the strip, one state under the other, since the
           reading is an answer from Rust rather than a control anybody can
           reach from here. In order: a half the harness did not print, drawn as
           a dash beside the half it did; an agent that does not report this at
           all; a fresh week's real `0`, which is a number and not a dash;
           nothing asked yet, which names nobody; a probe on its way, which keeps
           the last numbers and says so in the hint; and `invoke` refusing, which
           is the channel rather than an answer and says which. Hover any of
           them — the hint is the whole of the reset times, of what a run would
           do, and of why there is nothing to read. The right end is empty in
           all six, which is what a quiet project looks like and the state to
           compare the block below against. -->
      <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-4)' }">
        <StatusFooter :usage="galleryUsage[1]" />
        <StatusFooter :usage="galleryUsage[2]" />
        <StatusFooter :usage="galleryUsage[3]" />
        <StatusFooter />
        <StatusFooter :usage="galleryUsage[0]" busy />
        <StatusFooter error="the worker is not answering" />
      </div>

      <!-- The right end: what the project is doing, which used to be drawn by
           the scope bar above and is drawn here now.

           The counters are why there are so many. Each is drawn only above
           zero, and an unknown number of uncommitted files — `null`, what
           `stores/vcs.js` hands over when the working tree could not be read —
           draws nothing at all, exactly as a clean tree does. The two look
           identical on screen on purpose, so the pair in the middle is the only
           place the difference can be seen against its own props.

           The singulars are hover-only, being tooltips: point at the file and
           agent counters in the second strip for "1 uncommitted file" and "1
           agent running".

           The last three are the headline, which none of the others draw — the
           empty case is the common one and has to be seen as the strip closing
           up rather than as a gap. The live one is also where the strip is
           narrowed: the app's own window stops at 1024px and nothing has to
           give up anything there, so this takes a browser dragged to about half
           that. What must happen then is that the sentence ellipsises and goes,
           while the subscription keeps every letter of `Claude Code Session
           Week` and both counters keep their numbers — the sentence is the only
           thing on the strip written to be lost. -->
      <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-4)' }">
        <!-- Both counters plural. -->
        <StatusFooter :usage="galleryUsage[0]" :dirty-count="12" :agents-active="3" />
        <!-- Ones, in both places there is a noun to get wrong. -->
        <StatusFooter :usage="galleryUsage[0]" :dirty-count="1" :agents-active="1" />
        <!-- Unknown rather than zero: no file glyph and no number, with the
             agents counter beside it to show the strip is otherwise alive. -->
        <StatusFooter :usage="galleryUsage[0]" :dirty-count="null" :agents-active="2" />
        <!-- A clean tree with nothing running: the same nothing as above from
             the opposite fact. -->
        <StatusFooter :usage="galleryUsage[0]" :dirty-count="0" :agents-active="0" />

        <!-- Nothing to say, said explicitly: the prop is there and empty, and
             the strip between the subscription and the counters closes up. This
             is the one to compare the two below against. -->
        <StatusFooter :usage="galleryUsage[0]" headline="" :dirty-count="2" :agents-active="0" />
        <!-- Live: muted, no glyph, and beside the agents counter it is a
             sentence rather than a number. It carries a run segment as well,
             because in the app this sentence is only ever drawn beside one —
             this is the crowded case, and the stop button on it is the one to
             press: it belongs to the run and not to the subscription, which is
             the whole reason the strip's press target ends where its own words
             do. -->
        <StatusFooter
          :usage="galleryUsage[0]"
          headline="Run under way"
          headline-level="live"
          :dirty-count="4"
          :agents-active="2"
        >
          <template #status>
            <RunBar :run="runFixture({ kind: 'working', iteration: 2 }, { batches: 3 })" @stop="() => {}" />
          </template>
        </StatusFooter>
        <!-- Loud, which is the case the glyph exists for: this strip is one of
             the one or two places on a screen allowed to shout, and the colour
             is never the only thing saying so. -->
        <StatusFooter
          :usage="galleryUsage[0]"
          headline="1 agent needs you"
          headline-level="loud"
          :dirty-count="1"
          :agents-active="3"
        />
      </div>
    </section>

    <!-- The tree's own context menu, which nothing else on this page can show:
         `PointerMenu` draws nothing until a secondary click gives it a point,
         and the rows here are the tree's rather than a fixture. Three copies,
         because the one row that changes between them is Attach to agent, and
         it has three states: live, and greyed with either of the two reasons
         written into the label — a row in this panel has no tooltip and no
         title, so the label is the only place a reason can be. The middle box
         is the one that is easy to miss and is the ordinary case: an agent is
         running, it is simply not the one selected.

         All three are taller than their trees on purpose: the space below the
         last row opens the root's menu, which is the one without Attach to
         agent or Delete on it — and, of the clipboard group, with Paste alone
         on it — and the only way to reach the second half of this menu in a
         project whose first screen is nothing but folders.

         The first tree is also the one with something on the clipboard: the row
         it names is drawn muted, its menu offers Paste, and the other two grey
         that row with the reason in its label. Paste on a folder inside what
         was cut is the second reason and cannot be reached here, since
         `src/agent.rs` is a file with nothing under it; it is drawn by hand in
         the section below. -->
    <section :style="sectionStyle">
      <div :style="headStyle">File tree menu</div>
      <div :style="rowStyle">
        <div :style="fileMenuBoxStyle">
          <FileTree
            :nodes="galleryTree"
            :expanded="galleryTreeExpanded"
            selected-path="Cargo.toml"
            can-attach
            has-live-agent
            :clipboard="galleryClipboard"
            @action="() => {}"
          />
        </div>
        <div :style="fileMenuBoxStyle">
          <FileTree
            :nodes="galleryTree"
            :expanded="galleryTreeExpanded"
            selected-path="Cargo.toml"
            has-live-agent
            @action="() => {}"
          />
        </div>
        <div :style="fileMenuBoxStyle">
          <FileTree
            :nodes="galleryTree"
            :expanded="galleryTreeExpanded"
            selected-path="Cargo.toml"
            @action="() => {}"
          />
        </div>
      </div>
    </section>

    <!-- Every state of the tree that writes to disk, in the forms they are hard
         to reach in a browser: the draft row, which in the app appears only
         after a pick; the same row filled for a rename; a row that has been
         cut; and Delete asking a second time, which needs a panel that has
         stayed open through one pick.

         The draft rows are drawn between ordinary tree rows on purpose — this
         section exists to check one thing, that the row is a place in the tree
         and not something floating over it. Height, indent per level and type
         have to match the rows above and below exactly, in both densities and
         both themes; a field built out of `Input` would be `--control-h` tall
         and would say so by pushing everything under it down. The rename row is
         the same check with the field full: a name of any length must not push
         the row wider or taller than the ones it sits between.

         What this page cannot show is the selection inside the rename field.
         `focusOnMount` is off here for the reason that prop's own comment gives
         — a field that focuses itself scrolls this page to itself on load —
         and a selection is only painted in a focused field. The rule behind it
         is `renameName.js`'s and is checked by its own test.

         The selected folder at the foot of the first box is the state the
         keyboard added: the shortcuts act on the selected row, so a folder had
         to become selectable for a Paste to have anywhere to land, and the
         selected surface under a folder's own glyph and chevron is a pairing
         nothing drew before.

         The second box is the cut row, which is the one thing on screen that
         says a cut is pending: nothing has happened on disk, so the muting is
         the whole of the signal. It is drawn on a file, on a folder and on the
         selected row, because the selected surface underneath is where a
         dimming is easiest to get wrong. -->
    <section :style="sectionStyle">
      <div :style="headStyle">File tree: making, renaming and deleting</div>
      <div :style="rowStyle">
        <div :style="fileTreeStatesStyle">
          <FileTreeRow name="src" kind="dir" :depth="0" expanded />
          <FileTreeDraftRow kind="file" :depth="1" :focus-on-mount="false" />
          <FileTreeRow name="App.vue" kind="file" :depth="1" />
          <FileTreeDraftRow kind="file" :depth="1" value="report.md" :focus-on-mount="false" />
          <FileTreeRow name="main.js" kind="file" :depth="1" />
          <FileTreeDraftRow kind="dir" :depth="0" :focus-on-mount="false" />
          <FileTreeRow name="Cargo.toml" kind="file" :depth="0" selected />
          <FileTreeRow name="tests" kind="dir" :depth="0" selected />
        </div>
        <div :style="fileTreeStatesStyle">
          <FileTreeRow name="src" kind="dir" :depth="0" expanded />
          <FileTreeRow name="App.vue" kind="file" :depth="1" cut />
          <FileTreeRow name="main.js" kind="file" :depth="1" />
          <FileTreeRow name="node_modules" kind="dir" :depth="0" git="ignored" cut />
          <FileTreeRow name="docs" kind="dir" :depth="0" cut />
          <FileTreeRow name="Cargo.toml" kind="file" :depth="0" selected cut />
        </div>
        <ContextMenu :items="FILE_MENU" :width="FILE_MENU_W" />
        <ContextMenu :items="ARMED_FILE_MENU" :width="FILE_MENU_W" />
        <ContextMenu :items="NOTHING_COPIED_FILE_MENU" :width="FILE_MENU_W" />
        <ContextMenu :items="PASTE_INTO_SELF_FILE_MENU" :width="FILE_MENU_W" />
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Editor</div>
      <div :style="{ height: '200px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <FileEditor v-model="editorText" path="src/main.rs" />
      </div>
      <div :style="{ height: '160px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <FileEditor v-model="editorJs" path="src/stores/tabs.js" />
      </div>
      <div :style="{ height: '160px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <FileEditor v-model="editorMd" path="README.md" />
      </div>
      <div :style="{ height: '120px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <FileEditor v-model="editorPlain" path="notes.unknownext" />
      </div>
      <!-- The conflict markers as structure: the current side on one stripe, the
           incoming side on the other, all four marker lines in the conflict
           colour and a shade heavier. The diff3 block's base section is the one
           to check for the absence of a ground. -->
      <div :style="{ height: '260px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <FileEditor v-model="editorConflict" path="src/api/axios.js" />
      </div>
      <!-- The same long line twice: scrolling sideways, then wrapped. -->
      <div :style="{ height: '100px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <FileEditor v-model="editorLongLine" path="src/wide.js" />
      </div>
      <div :style="{ height: '100px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <FileEditor v-model="editorLongLineWrapped" path="src/wide-wrapped.js" word-wrap />
      </div>
      <div :style="{ height: '120px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <FileEditor
          model-value=""
          read-only
          path="assets/logo.png"
          :notice="{ tone: 'blocked', text: 'Binary file — not shown.' }"
        />
      </div>
      <div :style="{ height: '120px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <FileEditor
          model-value="my local edits"
          path="src/app.js"
          :notice="{ tone: 'stale', text: 'This file changed on disk since it was opened.' }"
        />
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Diff</div>
      <!-- The same frame the editor above uses, and for the same reason: the
           merge view fills the height it is given and scrolls inside it, so a
           frame with none would grow to the length of the longer file.

           Three frames, because the three things a diff can be are all worth
           looking at: a file changed on both sides, a file HEAD does not have —
           where the left column is empty and says which emptiness it is — and
           one that could not be read at all, which draws the strip and no
           columns, since a diff of nothing is two blank halves saying nothing.
           The colours to check are the line grounds and the underline on the
           characters that actually moved, in both themes and both densities. -->
      <div :style="{ height: '220px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <DiffView path="src/git.rs" :head="diffHead" :work="diffWork" />
      </div>
      <div :style="{ height: '160px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <DiffView path="notes/todo.txt" head="" :work="diffNew" missing-at-head />
      </div>
      <!-- The same conflicted file as the editor frame above, against the HEAD
           it was merged into. This is the frame that proves the cascade: in the
           working tree's pane those lines already carry `cm-changedLine`, so if
           the conflict grounds are missing here and present up there, the theme
           rule has lost a specificity argument to the diff. -->
      <div :style="{ height: '280px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <DiffView path="src/api/axios.js" :head="CONFLICT_HEAD" :work="CONFLICT_TEXT" />
      </div>
      <div :style="{ height: '100px', display: 'flex', border: 'var(--border-w) solid var(--border)' }">
        <DiffView path="assets/logo.png" notice="Binary file — not shown." />
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Terminal</div>
      <!-- The session arrives as a prop; its output comes from the store, which
           the mock backend answers out of terminalFixture.js. Height is a token
           multiple, not a pixel number: the terminal fills whatever height it
           is given. -->
      <div
        :style="{
          display: 'flex',
          height: 'calc(var(--space-9) * 6)',
          border: 'var(--border-w) solid var(--border)'
        }"
      >
        <TerminalView :session-id="GALLERY_SESSION" />
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Agents</div>
      <!-- 252px is the left panel's shipped width, so what truncates here
           truncates in the app. Tall enough for all seven rows at the
           comfortable row height: the point of this section is seeing every
           caption at once, and a scrollbar would hide the last of them. -->
      <div :style="{ width: '252px', height: '224px', border: 'var(--border-w) solid var(--border)' }">
        <!-- No capability is handed to the list at all: whether a session can
             be told to clear its conversation is that session's harness's
             answer, so each row above carries its own. The Codex row is in the
             fixture for exactly this — open the row menu on it and on any row
             above it, and the two sentences of Clear session can be read side
             by side, which is the state the panel could not draw at all while
             one flag stood for the whole project. -->
        <AgentList :rows="agentRows" :active-id="2" :pinned="AGENT_PINS" />
      </div>
      <!-- What that second row's run has taken, drawn where it actually
           appears: the right column at its shipped 340px, padded by
           --panel-pad the way the inspector column is, so the negative margin
           the list reaches back out with lands on the real edge. 340 is the
           outer width — box-sizing is border-box tree-wide — and the padding is
           a token rather than a number, because it is 12px comfortable and 8px
           compact and a literal could only be right at one of them. The third
           row has no title, which is what an id the tracker has not caught up
           with looks like. -->
      <div :style="{ width: '340px', padding: '0 var(--panel-pad)' }">
        <ClaimedTasks :tasks="CLAIMED" selected-id="smetana-9je" @select="() => {}" />
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Sessions</div>
      <!-- The right column's Sessions tab, at the 340px that column ships at,
           so a title that ellipsises here ellipsises in the app. No padding
           around it, unlike ClaimedTasks above: these rows run to the edges of
           the column and carry their own rule between them, and inset rows
           would draw the separator somewhere it never appears.

           The frame stands for the panel this list sits in — where the column
           ends is not this component's business, and without a frame the rows
           would float in the page with nothing to say where the list stops.
           Nothing of it doubles a rule any more: the separator belongs to the
           row below it, so the first row draws none against the frame's top
           edge and the last draws none against its bottom one. -->
      <div :style="{ width: '340px', border: 'var(--border-w) solid var(--border)' }">
        <SessionRow
          v-for="(session, index) in GALLERY_SESSIONS"
          :key="session.id"
          :session="session"
          :now="GALLERY_SESSION_NOW"
          :separated="index > 0"
          :expanded="openSessions.includes(session.id)"
          :copy-state="sessionCopyStateFor(session.id)"
          :copy-noun="sessionCopyNounFor(session.id)"
          @toggle="toggleSession"
          @action="onSessionAction"
        />
      </div>
      <!-- The three states of an opened card the list above cannot show at
           once. The first is a session nobody said anything in: its prompt
           block carries the sentence that stands in for a first prompt, which
           is the only place that string appears. The second is a card frozen
           while a delete runs against it — every row of the menu greyed and the
           two launching buttons with them, and no reason under either, since a
           freeze is a moment rather than a refusal. That is the state the app
           draws between Delete and the row leaving the list. The third is a project set to an agent that
           cannot pick a session up by id: both launching verbs are refused for
           the whole project rather than for this session, and the card says
           which of the refusals it is — two lines here, since the harness is
           asked two questions and answers each in its own words. The other
           refusal — a working directory that has gone — is the third card of
           the list above, where the fixture carries `cwdExists: false` and the
           one reason the two verbs share is written once. -->
      <div :style="{ width: '340px', border: 'var(--border-w) solid var(--border)' }">
        <SessionRow :session="GALLERY_SESSIONS[4]" :now="GALLERY_SESSION_NOW" expanded />
        <SessionRow
          :session="GALLERY_SESSIONS[3]"
          :now="GALLERY_SESSION_NOW"
          separated
          expanded
          busy
        />
        <SessionRow
          :session="GALLERY_SESSIONS[5]"
          :now="GALLERY_SESSION_NOW"
          :can-resume="false"
          :can-fork="false"
          separated
          expanded
        />
      </div>
      <!-- What the tab draws for a project whose disk holds no transcript at
           all — a missing `~/.claude/projects` among them, which is an ordinary
           outcome rather than a failure and must not read as one. -->
      <div :style="{ width: '340px', border: 'var(--border-w) solid var(--border)' }">
        <EmptyState
          compact
          icon="terminal"
          title="No sessions yet"
          description="Claude Code sessions run in this project will appear here."
        />
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Git</div>
      <!-- 252px is the left panel's shipped width, so what truncates here
           truncates in the app, and each frame is wrapped in the same Panel the
           sidebar puts around it — the refresh button lives in that header
           rather than inside GitPanel, exactly as it does in DesktopApp.vue.

           Every frame is a flex container rather than a plain block, and that
           is what makes the height it declares real: `Panel` carries no height
           of its own (`shell/Panel.vue`) and stretches to a flex parent, which
           is how DesktopApp gives it one. In a plain block it grows to its
           content instead, `GitPanel`'s `height: 100%` resolves against nothing,
           and the changes list — the one thing a tall frame is here to show
           scrolling — spills out over the frame below it.

           Four frames, because the three empty states are the point of this
           component and each says something different: a repository with files
           in it, a clean one, a folder that holds no repository at all, and a
           machine with no git on it. The last one is the only one a person can
           act on, and it names what was looked for.

           These are also where **⌘F inside the panel** can be tried: press it
           with the focus anywhere in one of these frames and the Branches
           caption becomes the filter field, unfolding the section first if it
           was folded. The other half of that rule — the command palette
           answering the same chord everywhere outside the panel — is only
           checkable in the app itself, since this page draws no palette. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <template #actions>
              <IconButton icon="refresh-cw" label="Refresh git" size="sm" />
            </template>
            <GitPanel
              :repos="REPOS"
              selected="/Users/you/dev/smetana"
              :tree="{ branch: 'feat/worktree-rename', detached: null, changes: CHANGES }"
              :branches="BRANCHES"
            />
          </Panel>
        </div>
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <template #actions>
              <IconButton icon="refresh-cw" label="Refresh git" size="sm" />
            </template>
            <!-- A repository with nothing uncommitted and one branch: the two
                 empty states that are not failures, side by side. -->
            <GitPanel
              :repos="[REPOS[0]]"
              selected="/Users/you/dev/smetana"
              :tree="CLEAN_TREE"
              :branches="[{ name: 'main', current: true }]"
            />
          </Panel>
        </div>
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <GitPanel :repos="[]" :tree="null" />
          </Panel>
        </div>
        <!-- The one thing this panel says about a repository it is **not**
             drawing: a folder somebody cloned into the project, which a
             configured `[project].repos` cannot grow to hold. What to check —
             that the caption and the names are rows of exactly the height of
             the repository rows above them, that the gear sits inside the row
             in the compact density too, and that the block stays quiet enough
             not to read as two more repositories. Every other frame here has
             nothing unlisted, which is what the block looks like in a project
             set up properly: nothing at all. -->
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <template #actions>
              <IconButton icon="refresh-cw" label="Refresh git" size="sm" />
            </template>
            <GitPanel
              :repos="REPOS"
              :unlisted="['newrepo', 'vendor-fork']"
              selected="/Users/you/dev/smetana"
              :tree="CLEAN_TREE"
              :branches="[{ name: 'main', current: true }]"
            />
          </Panel>
        </div>
        <!-- The one live frame: its folds, its section heights and its branch
             folders are held here the way the app holds them — the first two in
             `settings.layout` and the third under the project — so the
             chevrons, the two separators and the headings inside the branch
             list can all actually be worked. It is taller than its
             neighbours because a drag needs somewhere to go — the four above are
             sized to catch what a short panel does to the captions, and this one
             is sized to catch what a drag does at all.

             **The name filter is live here and only here**, since the query is
             this panel's own state: press the `search` button in the Branches
             caption and the row becomes a field with the focus in it. Type
             `git-panel`, and what to check is that the caption's count is gone,
             the tab labels read `2 of 8` and `Origin 0`, the list has flattened
             past its folders, and the highlight sits under the two matches.
             Press `Origin` and the query holds, with the labels swapping to
             `Local 2` and `0 of 5` over the no-match state. Then
             `Esc` once to empty the field, `Esc` again to close it — and the
             list comes back scrolled where it was, with exactly the folders that
             were open still open.

             **The keyboard is live here and, for the ring, only here.** The
             branch list is a `role="tree"` with one tab stop in it: Tab into it
             and the focus lands on the current branch, the first row; the
             vertical arrows walk the list, `ArrowRight` opens the heading under
             the cursor and `ArrowLeft` closes it or goes out to it, Enter is
             the double click (nothing is listening for a checkout in this
             frame, so what it must do here is *nothing at all* on the row with
             the tick), and `Shift+F10` opens the same menu the right click
             does, at the row's own bottom-left corner. `⌘F` anywhere in this
             frame opens the filter field, which is the panel taking that chord
             inside itself.

             **The ring is what this frame is for.** The branch box scrolls
             (`overflow: auto`, no padding) and a row is flush with all three of
             its edges, so a ring drawn outside the row would be cut away on the
             left, the right and — on the first row — the top. It is pulled
             inside instead: what to check is four whole sides, on the current
             branch over `--surface-selected` where its own `--border` rules sit
             a pixel away, and again on a row far enough down that arrowing to
             it scrolls the box, since `.focus()` puts that row flush against
             the leading edge. Both themes and both densities, and nothing may
             move by a pixel as the ring appears.

             **That second half rests on this frame's own geometry**, which is
             the one thing about it that has to be said out loud: the height
             here and `LONG_BRANCHES` between them leave the branch box shorter
             than its rows — measured at 86px of box against 196px of rows — so
             arrowing down the list actually scrolls it. Shorten the fixture or
             the frame and the check goes quiet rather than failing, which is
             exactly what happened to the frame this note was moved off. -->
        <div :style="{ display: 'flex', width: '252px', height: '420px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <template #actions>
              <IconButton icon="refresh-cw" label="Refresh git" size="sm" />
            </template>
            <GitPanel
              :repos="REPOS"
              selected="/Users/you/dev/smetana"
              :tree="{ branch: 'feat/worktree-rename', detached: null, changes: CHANGES }"
              :branches="LONG_BRANCHES"
              :remote="ORIGIN_BRANCHES"
              :sections="gitFolds"
              :branch-folders="gitFolders"
              :remote-folders="remoteFolders"
              :branch-tab="gitBranchTab"
              @toggle="toggleGitSection"
              @toggle-folder="gitFolders = $event"
              @toggle-remote-folder="remoteFolders = $event"
              @branch-tab="gitBranchTab = $event"
              @resize="resizeGitSection"
            />
          </Panel>
        </div>
        <!-- The tab row itself, which is the whole of this task, parked on
             `Origin` so both sides are on the page at once without anybody
             having to press anything.

             What to check. The row is one control row, the same height as the
             tab rows the side columns draw — deliberately **not** the height of
             the caption above it or of the branch rows below it, since it is
             `SegmentedTabs` and sizes itself from `--control-h-sm` and its own
             padding. Its height is measured rather than asserted, so what
             matters is what that measurement buys: the sections below still
             stop on whole rows, with no half row peeking out from under a fold,
             in both densities. The hairline under the row is the one
             `SectionHeader` draws, at one pixel and not two. The active segment
             is a fill and never a colour change, and the inactive one is
             legible rather than invisible on both themes. And the count in the
             caption is the count of the list underneath: five here, against the
             eight local branches the frame beside this one counts. -->
        <div :style="{ display: 'flex', width: '252px', height: '420px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <template #actions>
              <IconButton icon="refresh-cw" label="Refresh git" size="sm" />
            </template>
            <GitPanel
              :repos="REPOS"
              selected="/Users/you/dev/smetana"
              :tree="CLEAN_TREE"
              :branches="LONG_BRANCHES"
              :remote="ORIGIN_BRANCHES"
              branch-tab="origin"
              :remote-folders="['feature']"
            />
          </Panel>
        </div>
        <!-- And the same tab with a run going, where the tab row is deliberately
             the one thing not dimmed with the rows: choosing which side of the
             repository to look at is reading, exactly as unfolding a heading
             is. Under it, on the Origin tab as on the Local one, is the freeze
             strip — the verdict is one verdict and covers both sides, since a
             checkout writes the working tree whichever list it was pressed
             in.

             It is also the frame for the focus ring under that strip: the first
             row sits directly beneath it, muted and inert, and the ring around
             it has to be whole and legible there too — a frozen row still takes
             the keyboard, since reading a list is not writing to it. The
             tooltip over the row opens with the focus as well as with the
             pointer, which is the sentence saying why nothing can be pressed. -->
        <div :style="{ display: 'flex', width: '252px', height: '300px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <GitPanel
              :repos="REPOS"
              selected="/Users/you/dev/smetana"
              :tree="CLEAN_TREE"
              :branches="LONG_BRANCHES"
              :remote="ORIGIN_BRANCHES"
              :branch-tab="gitRunTab"
              :remote-folders="['feature']"
              :actions="RUN_GOING"
              @branch-tab="gitRunTab = $event"
            />
          </Panel>
        </div>
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <GitPanel
              :repos="[]"
              :tree="null"
              :error="{ kind: 'noGit', message: 'Smetana looked for git on your PATH and found nothing.' }"
            />
          </Panel>
        </div>
      </div>
      <!-- The Branches tab row with its three verbs at the right end of it,
           which is the one place in the app the remote can be reached from —
           and the states are the branch the repository is *on*, since that is
           what the two arrows are about. The check beside them is about the
           repository and is in every frame, including the ones where both
           arrows are gone. What to check: that the three buttons and the two
           tabs share one row 252 pixels wide without either half being crowded
           out, that the row is still the height `SegmentedTabs` set on its own
           (the sections below still stop on whole rows, in both densities),
           that a refused button is legible rather than invisible, and that its
           reason opens on hover from the wrapper around it rather than from the
           disabled control itself. The caption above now carries a chevron, a
           word and a count and nothing else. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <!-- The everyday pair: three commits waiting and nothing of ours to
             send, so Pull is live and Push is refused. Both are icon-only, so
             the count lives in the tooltip and in `aria-label` — hover the
             arrow to read `Pull 3`; nothing of it is drawn beside the glyph. -->
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <GitPanel
              :repos="REPOS"
              selected="/Users/you/dev/smetana"
              :tree="{ branch: 'feat/worktree-rename', detached: null, changes: CHANGES }"
              :branches="onBranch('feat/worktree-rename')"
              :tracking="TRACKING"
            />
          </Panel>
        </div>
        <!-- Level with the remote: both verbs refused, each in its own words,
             and the check between them the only thing left to press. This is
             the ordinary state of a repository nobody else has pushed to, and
             the frame the third button exists for. -->
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <GitPanel
              :repos="REPOS"
              selected="/Users/you/dev/smetana"
              :tree="{ branch: 'feature/smetana-8ok-git-panel-branches', detached: null, changes: CHANGES }"
              :branches="onBranch('feature/smetana-8ok-git-panel-branches')"
              :tracking="TRACKING"
            />
          </Panel>
        </div>
        <!-- The check while its answer is out: the glyph is `loader-circle` at
             `--attn-live`, turning, and the button is refused until it lands —
             the one spinner this panel has, in the idiom the branch rows
             already use over a write. The two verbs beside it stay live: a
             fetch freezes no row. -->
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <GitPanel
              :repos="REPOS"
              selected="/Users/you/dev/smetana"
              :tree="{ branch: 'main', detached: null, changes: CHANGES }"
              :branches="onBranch('main')"
              :tracking="TRACKING"
              fetching
            />
          </Panel>
        </div>
        <!-- Diverged: both live, each naming its own number on hover, and the
             row for the branch they are about carries the same two marks —
             which is where the number is actually on screen. -->
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <GitPanel
              :repos="REPOS"
              selected="/Users/you/dev/smetana"
              :tree="{ branch: 'main', detached: null, changes: CHANGES }"
              :branches="onBranch('main')"
              :tracking="TRACKING"
            />
          </Panel>
        </div>
        <!-- A branch nobody has pushed — the ordinary state of one cut in this
             very panel. Push is named "Publish branch" — in its tooltip and to
             a screen reader, since the glyph is the same arrow — and Pull is
             refused: there is nothing there yet to pull from. -->
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <GitPanel
              :repos="REPOS"
              selected="/Users/you/dev/smetana"
              :tree="{ branch: 'spike', detached: null, changes: CHANGES }"
              :branches="onBranch('spike')"
              :tracking="TRACKING"
            />
          </Panel>
        </div>
        <!-- A run going: both arrows refused by the same verdict that mutes the
             rows, and both say so in `gitActions.js`'s own sentence. The check
             is live beside them, which is that rule drawn rather than merely
             stated: it writes remote-tracking refs and touches neither the
             tree nor the index, so a batch mid-merge has nothing to lose by it.

             Under the row is the strip that says it once for the eye, on
             `--status-running-bg` with the same sentence and ` · read only` on
             the end. What to check: that it is `--control-h-sm` and not a row,
             that its text clips with an ellipsis rather than wrapping to a
             second line, that the sections below still stop on whole rows with
             it there (it is inside the measured wrapper), and that the per-row
             tooltip still opens — the strip is for the eye and the tooltip for
             the pointer, and both are wanted. -->
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <GitPanel
              :repos="REPOS"
              selected="/Users/you/dev/smetana"
              :tree="{ branch: 'main', detached: null, changes: CHANGES }"
              :branches="onBranch('main')"
              :tracking="TRACKING"
              :actions="RUN_GOING"
            />
          </Panel>
        </div>
        <!-- A detached HEAD draws neither arrow: there is no branch for an
             upstream to be about, and two dead controls say less than the row
             does without them. The check stays — asking the remote what it has
             is a question about the repository, and a detached HEAD has not
             stopped it being one. And the first row of the list is the plate:
             `HEAD · a1b2c3d`, on the current block's surface, inert. -->
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <GitPanel
              :repos="REPOS"
              selected="/Users/you/dev/smetana"
              :tree="{ branch: null, detached: 'a1b2c3d', changes: CHANGES }"
              detached="a1b2c3d"
              :branches="onBranch(null)"
              :tracking="TRACKING"
            />
          </Panel>
        </div>
      </div>
      <!-- The two lists on their own, at the same width: what a row does with a
           long path, a detached HEAD and a rename's second path is easier to
           check without a panel around it. The last frame is git's own refusal,
           shown untouched — the person reading it knows git. -->
      <!-- The caption on its own, in its three states, and then the fourth
           frame: two of them stacked, the lower one `divided`, which is the
           only way to see what the rule between two blocks actually looks like.
           All of them fold, so what the chevron does is checkable here without
           a panel around it; the folded one keeps its count on purpose. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <SectionHeader
            label="Branches"
            :count="9"
            :open="headerFolds.withCount"
            @toggle="headerFolds.withCount = !headerFolds.withCount"
          />
        </div>
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <SectionHeader
            label="Changes"
            :open="headerFolds.bare"
            @toggle="headerFolds.bare = !headerFolds.bare"
          />
        </div>
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <SectionHeader
            label="Repositories"
            :count="3"
            :open="headerFolds.folded"
            @toggle="headerFolds.folded = !headerFolds.folded"
          />
        </div>
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <SectionHeader label="Repositories" :count="3" :open="false" />
          <SectionHeader divided label="Changes" :count="7" :open="false" />
        </div>
        <!-- The `actions` slot, which is why the row is a wrapper around the
             caption rather than the caption itself: the controls are a sibling
             of a `<button>` and not its children, and both halves have to sit
             inside one `--row-h`. Beside it the same caption with nothing in
             the slot, so what the gutter does to the count is visible in one
             glance.

             The two buttons here are a fixture: the Git panel's own Fetch,
             Pull and Push moved down into the Branches tab row, and what fills
             the slot in the app now is the Branches caption's single `search`
             button. The pair is kept because a caption with *two* controls is
             what the gutter arithmetic is about and nothing in the app draws
             one. Do not delete it as dead. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <SectionHeader
            label="Branches"
            :count="9"
            :open="headerFolds.withActions"
            @toggle="headerFolds.withActions = !headerFolds.withActions"
          >
            <template #actions>
              <IconButton icon="arrow-down" label="Pull 2" size="sm" />
              <IconButton icon="arrow-up" label="Push 1" size="sm" />
            </template>
          </SectionHeader>
          <SectionHeader divided label="Branches" :count="9" :open="true" />
        </div>
        <!-- The row as a field, which is the other half of the filter: with
             `searching` set the caption `<button>` is not drawn at all, so
             there is nothing to fold by — deliberately, since the row somebody
             is typing into must not also be the row that folds the section away
             under them.

             The plate inside it is a **fixture** of `GitPanel`'s own, which
             owns the query and everything about it; what this frame is for is
             the swap. What to check: the row is exactly the height of the
             caption beside it in both densities, the plate reaches both edges
             of the panel, and the `x` sits where the `search` button of the
             frame above sits. Type into it — the field is live here, and the
             caret and the placeholder are the two things worth looking at on
             both themes. Then Tab: **there is no ring on the input at all**,
             and what says the caret is here is the plate, which steps from
             `--surface-raised` to `--surface-active` end to end, glyph and `x`
             included. Tab again and the plate drops back to `--surface-raised`
             while the `x` takes `base.css`'s own ring — the two states are the
             pair worth checking together, and the dark theme is the one to
             check them in: the step is smallest there (1.198:1 against 1.338:1
             light), and `--surface-hover` was drawn here until its dark step
             measured 1.090:1 and could not be seen at the default font size.
             Compact is the density to check the height in. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <SectionHeader label="Branches" :count="9" searching>
            <template #editor>
              <div
                :style="{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 'var(--space-2)',
                  flex: 1,
                  minWidth: 0,
                  height: '100%',
                  padding: '0 var(--space-3) 0 var(--space-5)',
                  background: headerSearchFocused ? 'var(--surface-active)' : 'var(--surface-raised)',
                  transition: 'var(--transition-control)'
                }"
              >
                <Icon name="search" :size="12" :style="{ flex: 'none', color: 'var(--text-muted)' }" />
                <input
                  v-model="headerSearch"
                  type="text"
                  placeholder="Filter branches"
                  aria-label="Filter branches"
                  :style="{
                    flex: 1,
                    minWidth: 0,
                    height: '100%',
                    border: 'none',
                    outline: 'none',
                    background: 'transparent',
                    color: 'var(--text-primary)',
                    font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
                  }"
                  @focus="headerSearchFocused = true"
                  @blur="headerSearchFocused = false"
                />
                <Button variant="ghost" size="sm" icon="x" aria-label="Clear filter" />
              </div>
            </template>
          </SectionHeader>
          <SectionHeader divided label="Branches" :count="9" :open="true" />
        </div>
      </div>
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <RepoList :repos="REPOS" selected="/Users/you/dev/smetana" />
        </div>
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <!-- Live: right-click any row for its menu, and press Shift+F10 on a
               focused one for the same panel at the row's own corner. The
               fixture holds the two rows the refusals are about — the untracked
               directory and the deleted file — so both can be tried here. -->
          <ChangeList :changes="CHANGES" selected="src/stores/vcs.js" />
        </div>
        <!-- The same rows with no gesture in front of them. What has to be read
             here: six labels and three separators at the widths a 440 ceiling
             allows, the `file` glyph in the gutter beside the four that were
             already registered, and the refusal sentences whole rather than
             clipped. `Discard changes` is last, alone under the third
             separator, and it is the one row drawn in the danger tone — the
             only thing this menu does that loses work. -->
        <ContextMenu :items="CHANGE_MENU" :width="CHANGE_MENU_W" />
        <ContextMenu :items="FOLDER_CHANGE_MENU" :width="CHANGE_MENU_W" />
        <ContextMenu :items="GONE_CHANGE_MENU" :width="CHANGE_MENU_W" />
        <!-- The discard's own two refusals. On the left the conflicted row,
             where the reason is a suffix on the label and the five rows above
             are untouched; on the right a run holding the project, where
             `frozen`'s sentence is a caption over a group of one and every row
             above it is still live. Check that the red of the danger row is
             gone in both — a refused row is grey, not a quieter red. -->
        <ContextMenu :items="CONFLICTED_CHANGE_MENU" :width="CHANGE_MENU_W" />
        <ContextMenu :items="HELD_CHANGE_MENU" :width="CHANGE_MENU_W" />
        <!-- The commit box in its several states, at the panel's own width.
             Live first: type into it and the button comes alive with the count of
             what it would take, press the sparkle and the fixture message
             arrives, and drag the separator under the field to make it taller —
             double click hands back the two rows it ships at. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <CommitBox
            v-model="commitDraft"
            :changes="CHANGES.length"
            branch="feat/worktree-rename"
            :rows="commitRows"
            @commit="commitDraft = ''"
            @suggest="commitDraft = 'feat: add a commit box to the Git panel'"
            @resize="commitRows = $event"
          />
        </div>
        <!-- Nothing written yet, which is the state it opens in: the button is
             dead and says why on a tooltip rather than leaving somebody to work
             it out. And a detached HEAD beside it, where the field still invites
             a commit and has no branch to name — the placeholder's other
             half. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <CommitBox :changes="6" branch="develop" />
        </div>
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <CommitBox :changes="6" />
        </div>
        <!-- The agent thinking, and a run going. Two different questions, and
             the frame is here to show that they answer differently: the spinner
             stands in the sparkle's own box so nothing moves, and a run holds
             the commit while leaving the sparkle live, because asking for a
             message reads and writes nothing. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <CommitBox model-value="fix: keep the tick on the row" :changes="2" suggesting />
        </div>
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <CommitBox
            model-value="fix: keep the tick on the row"
            :changes="2"
            :actions="RUN_GOING"
            :suggest-error="{ kind: 'noAgent', message: 'Smetana looked for claude on your PATH and found nothing.' }"
          />
        </div>
        <!-- A repository git left mid-merge. `Resolve conflicts` sits on its
             own row directly above the commit button, full width and
             secondary — the commit is what this box is for, and two primary
             buttons stacked in one column is a choice nobody made. It is gated
             by nothing: pressing it opens a dialog and writes nothing. What to
             check beside it is that neither button loses its row height under
             the compact density and that neither overflows the box. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <CommitBox :changes="6" branch="main" :conflicts="3" @resolve-conflicts="() => {}" />
        </div>
        <!-- The plain list. **Not** the frame to check the keyboard in: this
             one is a plain 252px block with no scroller and no listeners, so
             the focus ring draws whole here where the app clips it, and the two
             fold arrows emit into nothing. Both belong on the live `GitPanel`
             frame above, which has the box that scrolls and the state to fold. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList :branches="BRANCHES" />
        </div>
        <!-- The branch the repository is on, lifted to the top out of the order
             recency put it in and out of the folder its name puts it in: it is
             last here and in a `feature/` heading that is folded, and it is
             still the first row, drawing its whole name.

             What to check is the plate it sits on: `--surface-selected` with a
             rule above and a rule below in `--border`, the tree under it on the
             canvas, and both rules **inside** the row's own height — the row
             below must not be pushed a pixel down, and the row itself must be
             exactly as tall as every other row here. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            :branches="[
              { name: 'main', current: false },
              { name: 'develop', current: false },
              { name: 'feature/smetana-8ok.5-branch-folders', current: true }
            ]"
            :folders="[]"
          />
        </div>
        <!-- The same list against its upstreams, which is every state a row can
             be in: behind (`↓3` in orange), ahead (`↑2`, neutral), both at
             once, level with the remote, and a branch nobody has pushed, which
             has no record and draws nothing at all. **No name takes the
             colour** — that is the whole of what changed here, and a repository
             where every branch is behind is why. What to check is that the
             marks do not push a long name into an ellipsis it did not have
             before, and that the orange is legible on both themes. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList :branches="BRANCHES" :tracking="TRACKING" />
        </div>
        <!-- And with the deleted upstream and the unpushed branch in the list,
             neither of which is orange: there is nothing to pull into either,
             and a colour on them would send somebody to a button that
             refuses. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            :branches="[
              { name: 'main', current: true },
              { name: 'spike', current: false },
              { name: 'old', current: false }
            ]"
            :tracking="TRACKING"
          />
        </div>
        <!-- A run going over the marks: the rows are muted and inert. The
             counts keep their own token whatever the row does — they are a fact
             about the remote and not an offer — and the names are the same
             colour as the names beside them, here as everywhere. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList :branches="BRANCHES" :tracking="TRACKING" :actions="RUN_GOING" />
        </div>
        <!-- The same list with a run going: every row inert, the current one
             still readable, and the reason on a tooltip over whichever row the
             pointer is on. The sentence is `gitActions.js`'s own, computed here
             from a run in the shape `runs.js` holds one, so the frame cannot
             drift from the rule. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList :branches="BRANCHES" :actions="RUN_GOING" />
        </div>
        <!-- The folders, live: press a heading and it opens. Nothing was chosen
             here, so the folder the current branch is in starts open and the
             others start folded — which is the state a repository is in the
             first time this panel is looked at. The current branch itself is
             the first row rather than a row inside that open heading, so what
             the seed is worth here is the branches beside it. `fix` holds a
             folder of its own, so the indentation of a second level is
             checkable here too.

             The folds are wired, so this is also where the two horizontal
             arrows can be seen doing something: walk down to a heading and
             press `ArrowRight` to open it and `ArrowLeft` to close it, and
             `ArrowLeft` from a leaf to go out to the heading above it. The ring
             is not what this frame shows — there is no scroller here to clip
             it; that is the live `GitPanel` frame's job. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            :branches="FOLDER_BRANCHES"
            :folders="branchFolders"
            @toggle-folder="branchFolders = $event"
          />
        </div>
        <!-- Every folder open, which is the same list at its tallest: what to
             check here is that a leaf is drawn without the prefix its siblings
             all share, and that the two names with no slash in them are still
             where recency put them rather than swept under a heading. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList :branches="FOLDER_BRANCHES" :folders="['feature', 'fix', 'fix/legacy']" />
        </div>
        <!-- The Origin tab, live and open. What to check: every branch
             `origin` has is here, the two that this repository also has drawn
             with `git-branch` and the three it does not with `cloud`, at the
             same size and in the same colour so the names stay in one column.
             `main` is the branch the repository is on and carries the tick on
             the right. There is no star and no `↓N` on any row, and no `origin`
             heading over them — the tab is what says where these come from.
             Press `feature` to fold it; right-click a row for the two items it
             has, which are `Check out from origin` on a cloud row and `Switch
             to this branch` on a branch one. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            tab="origin"
            :branches="[
              { name: 'main', current: true },
              { name: 'develop', current: false },
              { name: 'fix/legacy/depot-import', current: false }
            ]"
            :remote="ORIGIN_BRANCHES"
            :remote-folders="remoteFolders"
            @toggle-remote-folder="remoteFolders = $event"
          />
        </div>
        <!-- The same tab as it ships: every folder folded, which is what an
             empty `remoteBranchFolders` means, with a run going over it. The
             headings are live like every other heading here — unfolding is
             reading — and the rows, once unfolded, are muted and inert with the
             local ones, since checking one out writes the working tree either
             way. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            tab="origin"
            :branches="FOLDER_BRANCHES"
            :remote="ORIGIN_BRANCHES"
            :remote-folders="foldedRemote"
            :actions="RUN_GOING"
            @toggle-remote-folder="foldedRemote = $event"
          />
        </div>
        <!-- The tab's own empty state, which is a repository with no remote, one
             whose `origin` is empty, and the moment while the store's single
             remote list is about another repository. Never a blank area — and
             deliberately not the local tab's, which is beside it a few frames
             down.

             The one of those three cases a person can act on carries the one
             control this component draws outside a row: `Fetch`, which leaves
             as an event and goes dead while an answer is already out. The
             second frame is that state. What to check is that the button sits
             under `EmptyState` rather than inside it, centred, and that neither
             the title nor the sentence wraps into the button in the compact
             density. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList tab="origin" :branches="FOLDER_BRANCHES" :remote="[]" @fetch="() => {}" />
        </div>
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList tab="origin" :branches="FOLDER_BRANCHES" :remote="[]" fetching />
        </div>
        <!-- The filter's result, which is the same list with everything about
             its arrangement taken away: no folders, no headings, no three
             surfaces — flat on the canvas, in the order the tab already used.

             What to check. The prefix up to and including the last slash is
             muted and the tail is in ink, so the half that identifies a branch
             is still the loud half with no heading above it. The matched run
             sits on `--selection-bg` and changes nothing else — no weight, no
             colour — and the row does not move under it. A hit is still a row:
             the marked one keeps its star (and its tick, since it is also the
             branch the repository is on), `fix/legacy/depot-import` keeps its
             `↓2`, and both answer a double click and a right-click menu. The
             name truncates at the **end** here rather than in the middle, which
             is the one place `splitName` is deliberately not applied. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            :branches="FOLDER_BRANCHES"
            :tracking="FOLDER_TRACKING"
            :favorites="[FILTER_FAVORITE]"
            :query="FILTER_QUERY"
            :hits="FILTER_HITS"
            :other-hits="0"
          />
        </div>
        <!-- The case a substring over the **whole** name exists for: the query
             runs across the slash, so the highlight lands inside the muted
             prefix. What to check is that the prefix stays muted underneath it —
             the ground changes and nothing else — and that `feat/kick` finding
             nothing where `kick` finds two is the same rule seen from the other
             side. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            :branches="FOLDER_BRANCHES"
            :tracking="FOLDER_TRACKING"
            :query="PREFIX_QUERY"
            :hits="PREFIX_HITS"
            :other-hits="0"
          />
        </div>
        <!-- The same filter on the Origin tab. What to check: the two glyphs
             still choose themselves — `main` has a local twin and draws
             `git-branch` with the tick, the other two are only on `origin` and
             draw `cloud` — and there is no star and no `↓N` on this side, under
             a filter as without one. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            tab="origin"
            :branches="ORIGIN_LOCAL"
            :remote="ORIGIN_BRANCHES"
            :query="ORIGIN_QUERY"
            :hits="ORIGIN_HITS"
            :other-hits="0"
          />
        </div>
        <!-- Nothing matched, in both of its sentences, and the difference
             between them is the whole reason the other tab's count is a prop.
             First: the other side has some, so the state says how many and where
             to go — without it a filter would be hiding nine branches behind a
             true sentence. Second: nothing anywhere matched, so the query is
             named back, since what there is to fix is what was typed. The button
             under each does what the `x` in the field does — clears and closes,
             one act — and it is drawn under `EmptyState` rather than through a
             slot on it, exactly as the Origin tab's Fetch is. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            :branches="FOLDER_BRANCHES"
            :query="NO_MATCH_QUERY"
            :hits="[]"
            :other-hits="9"
          />
        </div>
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            tab="origin"
            :branches="FOLDER_BRANCHES"
            :remote="ORIGIN_BRANCHES"
            :query="NO_MATCH_QUERY"
            :hits="[]"
            :other-hits="0"
          />
        </div>
        <!-- The heading's own mark, which is the whole reason it exists: every
             folder is folded, `fix/legacy/depot-import` is behind, and both
             headings above it carry a bare `↓` — no number, since the count
             beside it is already a number about the same heading. Unfold `fix`
             and the mark moves down to `legacy` with the branch it is about. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            :branches="FOLDER_BRANCHES"
            :folders="foldedTracking"
            :tracking="FOLDER_TRACKING"
            @toggle-folder="foldedTracking = $event"
          />
        </div>
        <!-- And with a run going, where the headings are deliberately the one
             thing not dimmed: unfolding is reading, and a heading greyed out
             beside rows that are greyed out because they cannot be pressed
             would be saying something untrue about itself. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList :branches="FOLDER_BRANCHES" :folders="['feature']" :actions="RUN_GOING" />
        </div>
        <!-- The favourites, live: right-click any row and the menu offers to
             mark it or to unmark it. Three are marked here — one inside the
             `fix/` folder, one with no slash in it, and the branch the
             repository is on.

             What to check. The block at the top is the current branch and then
             the marked ones **in the order the branch list arrived in, which is
             not the order the fixture writes them**: the constant lists
             `develop` first and it has to draw last of the three, under
             `fix/holiday-curb…` and `feature/smetana-8ok.5…`. The current
             branch is marked too and is still **one** row, the first, carrying
             the star. The star sits where `git-branch` sits on every other row,
             so the names all start at the same x — the easiest way to see it is
             to unmark a row and watch that nothing moves sideways — and it is
             filled in `--text-secondary` and **no hue at all**, which is what
             leaves `--git-modified` meaning one thing in this section.

             And the three surfaces: the current branch on its own plate between
             two rules, the marked ones under it on `--surface` with one
             `--border-subtle` hairline under the **last** of them, the tree
             below on the canvas. Mark and unmark a row and watch the middle
             block and its hairline grow and shrink with it. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            :branches="FOLDER_BRANCHES"
            :folders="favoriteFolders"
            :favorites="favoriteBranches"
            @toggle-folder="favoriteFolders = $event"
            @favorite="favoriteBranches = $event"
          />
        </div>
        <!-- The same marks with every folder open, which is where the second
             half of the rule can be seen: a marked branch is gone from the
             folder it was in, so `fix` says one where it said two, and
             `feature/` — whose only remaining member was marked too — is **not
             drawn at all**, heading, chevron, count and everything. That last
             one is why `feature/smetana-8ok.5-branch-folders` is in the
             fixture; without it no folder is emptied and this frame shows only
             half of what its caption claims. The marked names are whole up top
             and the leaves below are not. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            :branches="FOLDER_BRANCHES"
            :folders="['feature', 'fix', 'fix/legacy']"
            :favorites="favoriteBranches"
          />
        </div>
        <!-- A name nothing in this repository is called, which is the ordinary
             state of a project whose repositories have different branches: it
             draws no row and changes nothing, and the top of the list is the
             current branch's plate alone, with no favourite block between it
             and the tree. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList :branches="BRANCHES" :favorites="['nothing-is-called-this']" />
        </div>
        <!-- The fourth of the panel's empty states, which no `GitPanel` frame
             can reach: a folder git can see nothing in has no branch to list,
             and the section is gated on there being a repository, so this is
             the only place it can be looked at. An `EmptyState` rather than the
             line of prose it used to be, `compact` because the box around it in
             the app is capped at a handful of rows. No button under it: a first
             commit is not something this panel can make. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList :branches="[]" />
        </div>
        <!-- A detached HEAD, where there is no current branch for the rule to
             lift and the component draws a plate instead: `HEAD · <short sha>`
             in the same mono, on the current block's surface between the
             current block's two rules, with the tick in the box at the end of
             the row. What to check — that it is exactly one row tall, that it
             answers nothing at all (no hover, no menu on a right click, no
             double click), and that the tree under it starts on the canvas the
             way it does under a real current branch. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            detached="a1b2c3d"
            :branches="[
              { name: 'main', current: false },
              { name: 'develop', current: false },
              { name: 'feature/smetana-8ok.5-branch-folders', current: false }
            ]"
            :folders="[]"
          />
        </div>
        <!-- The middle truncation, which needs a name too long for 252px to
             show at all. Two of these are marked, so three rows here draw a
             **whole** name: the last twelve characters are held whole and the
             ellipsis falls in the middle, which is what makes
             `…l-validation` and `…-geocode-precision` tell the two apart. The
             leaf under the `feature/` heading is not cut in the middle — it is
             already a tail, and the heading above it carries the prefix. What
             to check is that the tail never wraps, that the ellipsis lands
             between the two spans rather than inside the tail, and that the
             `↓12` on the current branch does not push the tail off the row. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList
            :branches="[
              { name: 'feat/nxc-204-kickbox-email-validation', current: true },
              { name: 'fix/holiday-curb-w78w-warehouse-geocode-precision', current: false },
              { name: 'main', current: false },
              { name: 'feature/smetana-8ok.5-branch-folders', current: false }
            ]"
            :favorites="['fix/holiday-curb-w78w-warehouse-geocode-precision', 'main']"
            :tracking="{
              'feat/nxc-204-kickbox-email-validation': {
                upstream: 'origin/feat/nxc-204-kickbox-email-validation',
                ahead: 0,
                behind: 12,
                gone: false
              }
            }"
            :folders="[]"
          />
        </div>
        <!-- A checkout in flight: the pressed row spins in place of its mark
             and the rest of the list goes inert, since a second press would ask
             git to work in a tree git is already working in. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList :branches="BRANCHES" :busy="{ op: 'checkout', branch: 'develop' }" />
        </div>
        <!-- And a merge in flight, which spins in the same one box as a
             checkout does: the row holds a single glyph at a time — the tick,
             or whichever of the three operations is running — so there is
             nothing to keep apart and no second width to hold.

             The merge and the rebase are in the row's right-click menu now, so
             what to check here is that a row draws its name, its mark and
             nothing else, and that a secondary click anywhere on it opens the
             panel `branchMenu.js` builds. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList :branches="BRANCHES" :busy="{ op: 'merge', branch: 'main' }" />
        </div>
        <!-- The compare window's left half, at the width that window gives it.
             The switch above the rows is live: press either position and read
             both labels in full. What to check on the rows is that the status
             letter, the file icon and the name line up with the change list two
             frames up — one file has to look the same in both — and that the
             rename names where it came from on its own row. -->
        <div :style="{ height: '260px', width: 'var(--panel-right-w)', border: 'var(--border-w) solid var(--border)' }">
          <CompareList
            :files="COMPARE_FILES"
            :mode="compareMode"
            selected="src/stores/vcs.js"
            @update:mode="compareMode = $event"
          />
        </div>
        <!-- Two branches with nothing between them, which is an ordinary answer
             and its own sentence: the window says something else entirely when
             the comparison could not be made at all, and the two must not read
             as the same emptiness. -->
        <div :style="{ height: '260px', width: 'var(--panel-right-w)', border: 'var(--border-w) solid var(--border)' }">
          <CompareList :files="[]" />
        </div>
        <div :style="{ display: 'flex', width: '252px', height: '160px', border: 'var(--border-w) solid var(--border)' }">
          <GitPanel
            :style="{ flex: 1, minWidth: 0 }"
            :repos="[REPOS[0]]"
            selected="/Users/you/dev/smetana"
            :tree="null"
            :error="{ kind: 'git', message: 'fatal: not a git repository (or any of the parent directories): .git' }"
          />
        </div>
        <!-- git's refusal of a **checkout**, which is a different failure from
             the two beside it: the panel read everything perfectly and git
             declined to switch. Drawn under the branch section rather than
             inside it, and the eight branches are the point of the frame — the
             section is capped at six rows, so a refusal drawn inside that
             scroller was entirely below the fold in exactly this, the ordinary,
             case. -->
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <GitPanel
            :style="{ flex: 1, minWidth: 0 }"
            :repos="[REPOS[0]]"
            selected="/Users/you/dev/smetana"
            :tree="CLEAN_TREE"
            :branches="LONG_BRANCHES"
            :write-error="CHECKOUT_REFUSED"
          />
        </div>
        <!-- The same block for a refused **merge**, which is the whole reason
             the title is keyed on the refusal's own `op`: one block for the
             three writes, and a message reading "did not switch branch" over
             this one would name an operation nobody asked for. git's message
             here runs to several lines, which is what the pre-wrapped mono
             block is for. -->
        <div :style="{ display: 'flex', width: '252px', height: '340px', border: 'var(--border-w) solid var(--border)' }">
          <GitPanel
            :style="{ flex: 1, minWidth: 0 }"
            :repos="[REPOS[0]]"
            selected="/Users/you/dev/smetana"
            :tree="CLEAN_TREE"
            :branches="BRANCHES"
            :write-error="MERGE_REFUSED"
          />
        </div>
        <!-- The same failure with the repository list empty, which is the
             shape a refusal from `vcs_repos` would take. The list's own "No
             repositories here" must not be what a person reads then: it states
             a folder was read and found bare, and this one was not read at
             all. -->
        <div :style="{ display: 'flex', width: '252px', height: '160px', border: 'var(--border-w) solid var(--border)' }">
          <GitPanel
            :style="{ flex: 1, minWidth: 0 }"
            :repos="[]"
            :tree="null"
            :error="{ kind: 'io', message: 'the repositories of this project could not be listed' }"
          />
        </div>
      </div>
      <!-- The conflict dialog, which is the only thing in this section that is
           not a panel. Five frames, because every one of these states is
           reachable in a second and none of them can be looked at any other
           way: a merge, a rebase — the branches swap sides in the sentence, and
           getting that backwards would send an agent the wrong way round — a
           rebase whose onto no git process can name, an abort in flight, and an
           abort git refused.

           The thing to check in every one of them: there **is** a close cross
           in the corner now, in both ways of opening this dialog alike, because
           the panel draws `Resolve conflicts` for as long as the tree is
           conflicted and a closed dialog is no longer a lost state. And in the
           fourth frame: the sentence ends on the branch with no trailing
           preposition and no empty mono span after it. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <div :style="{ position: 'relative', width: '560px', height: '420px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <ConflictModal
            v-bind="CONFLICT"
            :open="true"
            @resolve="() => {}"
            @abort="() => {}"
            @close="() => {}"
          />
        </div>
        <div :style="{ position: 'relative', width: '560px', height: '420px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <ConflictModal
            v-bind="{ ...CONFLICT, op: 'rebase' }"
            :open="true"
            @resolve="() => {}"
            @abort="() => {}"
            @close="() => {}"
          />
        </div>
        <!-- A rebase whose onto is unknown, which is what the panel holds for
             every rebase it did not start itself: `name-rev` answers
             `undefined` for a HEAD detached part-way through one, and the only
             other source is a file under `.git/` the module does not read. The
             sentence has to end on the branch. -->
        <div :style="{ position: 'relative', width: '560px', height: '420px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <ConflictModal
            v-bind="{ ...CONFLICT, op: 'rebase', theirs: null }"
            :open="true"
            @resolve="() => {}"
            @abort="() => {}"
            @close="() => {}"
          />
        </div>
        <div :style="{ position: 'relative', width: '560px', height: '420px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <ConflictModal
            v-bind="CONFLICT"
            :open="true"
            busy
            @resolve="() => {}"
            @abort="() => {}"
            @close="() => {}"
          />
        </div>
        <!-- git refusing the abort itself, drawn inside the dialog: the abort
             was pressed here, so a message anywhere else would sit under the
             dialog it is about. -->
        <div :style="{ position: 'relative', width: '560px', height: '460px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <ConflictModal
            v-bind="CONFLICT"
            :open="true"
            :error="{ kind: 'git', message: 'fatal: There is no merge to abort (MERGE_HEAD missing).' }"
            @resolve="() => {}"
            @abort="() => {}"
            @close="() => {}"
          />
        </div>
      </div>
      <!-- The branch picker, which is the review window's list of branches and
           the one control in this section that is deliberately **not** a
           popover: it sits in the flow, so a window whose height is computed
           from its content cannot clip it. The frame is 720 wide because that
           is what the review window is, and the first thing to check in it is
           `feature/smetana-4nsa-remote-branches-repo` at the top of the list —
           whole, in one line, with its meta still on the row.

           The rest of what this frame is for: the list stops at nine rows and
           scrolls inside itself, so the footer stays visible in both densities
           and at `--ui-scale` 1.2; the two toggles in the filter row are a
           radio pair, so exactly one of them is ever lit and pressing the lit
           one does nothing at all; the list holds one side at a time, `origin/`
           in a muted prefix under the cloud when the cloud is the lit one; and
           the arrows move the highlight with the row pulled back into view at
           either end, Enter takes it, Escape reports a close.

           The frame opens with `main` picked on the local side, so pressing the
           cloud is also where the selection bar being on no row is visible —
           the pick is a name and a side together, and `main` is not
           `origin/main`. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <div :style="{ width: '720px' }">
          <BranchPicker
            :branches="PICKER_BRANCHES"
            :repos="6"
            :fetched-at="PICKER_NOW - 120"
            :now="PICKER_NOW"
            :selected="pickerChoice.name"
            :selected-origin="pickerChoice.origin"
            :side="pickerSide"
            scope="Applies to 6 repositories"
            @select="pickerChoice = $event"
            @side="pickerSide = $event"
            @close="() => {}"
          />
        </div>
        <!-- The same component told nothing it does not have to be: no
             repository count, no fetch time, and one branch in the list with no
             timestamp of its own. Every meta line here has to come out shorter
             by a piece rather than carrying `NaN`, `Invalid Date` or a gap
             between two separators — and the footer's right-hand end is empty,
             which is the ordinary state when nobody has said what the choice
             applies to.

             It opens on the origin side, which is where a missing fetch time is
             visible at all: every row reads `origin` and nothing after it. -->
        <div :style="{ width: '720px' }">
          <BranchPicker
            :branches="PICKER_BRANCHES.slice(0, 3)"
            :now="PICKER_NOW"
            :side="pickerSideBare"
            @side="pickerSideBare = $event"
            @close="() => {}"
          />
        </div>
        <!-- At a side panel's width, which is not where this component is used
             and is where its one wrapping rule is visible: a name wider than
             the list wraps and takes the row's height with it, since the row
             declares a floor rather than a height. Nothing is ellipsised
             anywhere in this component, deliberately.

             The narrowest the filter row is ever asked to be, which is where the
             two toggles have to be checked for fitting beside the field, the
             counter and the cross — in compact and at `--ui-scale` 1.2 as
             well. -->
        <div :style="{ width: 'var(--panel-right-w)' }">
          <BranchPicker
            :branches="PICKER_BRANCHES.slice(0, 4)"
            :repos="6"
            :fetched-at="PICKER_NOW - 120"
            :now="PICKER_NOW"
            selected="main"
            :side="pickerSideNarrow"
            scope="1 repository"
            @side="pickerSideNarrow = $event"
            @close="() => {}"
          />
        </div>
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Projects</div>
      <!-- The rail is what the left column opens with: one 28×28 tile per
           project, the state dot in the corner, and the dashed tile that adds
           one. Nothing else on this page draws two hues on an 8px circle, which
           is the exception the tooltip's words are the price of — hover a tile
           and read the third segment.

           Right-click a tile: every one of them opens the project's three
           actions at the pointer, and the two that only mean anything for the
           project the window is pointed at are greyed with the reason in the
           row on every other. The setup item is the frames' second job — it
           reads "Set up" where there is no configuration and "Set up again"
           where there is one, damaged or not. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <div :style="{ display: 'flex', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <ProjectRail
            :projects="galleryProjects"
            active-path="/Users/you/dev/smetana"
            :states="galleryProjectStates"
            :branches="{ '/Users/you/dev/smetana': 'develop' }"
            can-add-agent
            configured
          />
          <!-- The panel beside it, so the header, the segmented tab row and the
               footer are checked against the rail they sit next to and at the
               236px the column ships at. -->
          <div :style="{ width: '236px' }">
            <Panel
              title="smetana"
              subtitle="develop · 1 running"
              side="left"
              toggle-label="Hide projects"
            >
              <template #marks>
                <Tooltip label="Not set up for runs">
                  <Icon name="triangle-alert" :size="12" :style="{ color: 'var(--status-failed-fg)' }" />
                </Tooltip>
              </template>
              <template #actions>
                <IconButton icon="refresh-cw" label="Refresh files" size="sm" />
              </template>
              <div :style="{ padding: 'var(--panel-pad)', fontSize: 'var(--text-sm)', color: 'var(--text-muted)' }">
                The tab row and its content are DesktopApp.vue&apos;s, not the panel&apos;s.
              </div>
              <template #footer>
                <div
                  :style="{
                    font: 'var(--weight-regular) var(--text-2xs)/var(--leading-snug) var(--font-mono)',
                    color: 'var(--text-muted)',
                    wordBreak: 'break-all'
                  }"
                >
                  /Users/you/dev/smetana
                </div>
              </template>
            </Panel>
          </div>
        </div>
        <!-- The same panel folded, which is what dragging the separator past the
             left minimum leaves. The rail is not drawn beside it in the app —
             two rails would be two rails — and the vertical title follows the
             header's rule: a subtitle means this is a name, so it is not
             uppercased down the strip. -->
        <div :style="{ display: 'flex', height: '200px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="smetana" subtitle="develop · 1 running" side="left" collapsed />
        </div>
        <!-- A rail longer than it is tall, which is where it starts to scroll,
             and the one with a project that has no bd tracker in it: that mark
             has nowhere to go on a tile, so it is the fourth segment of the
             tooltip and nothing else. -->
        <div :style="{ display: 'flex', height: '200px', border: 'var(--border-w) solid var(--border)' }">
          <ProjectRail
            :projects="[
              ...galleryProjects,
              { path: '/Users/you/dev/tracker-notes', name: 'tracker-notes', tracked: false },
              { path: '/Users/you/dev/archive', name: 'archive', tracked: true },
              { path: '/Users/you/dev/scratch', name: 'scratch', tracked: true }
            ]"
            active-path="/Users/you/notes"
            :states="galleryProjectStates"
          />
        </div>
        <!-- An empty rail: no projects yet, only the place for one. -->
        <div :style="{ display: 'flex', height: '120px', border: 'var(--border-w) solid var(--border)' }">
          <ProjectRail :projects="[]" />
        </div>
      </div>

      <div :style="headStyle">Project tile</div>
      <!-- The four states side by side, out of the rail, on the rail's own
           ground: selected, and the three the dot can be in. The dot is cut out
           of `--surface-sunken`, so a tile drawn on any other surface would
           show the ring as a colour rather than as a gap. -->
      <div
        :style="{
          display: 'flex',
          gap: 'var(--space-5)',
          alignItems: 'center',
          padding: 'var(--space-5)',
          background: 'var(--surface-sunken)',
          border: 'var(--border-w) solid var(--border)'
        }"
      >
        <ProjectTile
          :project="{ path: '/Users/you/dev/smetana', name: 'smetana', tracked: true }"
          active
          state="live"
          state-label="1 running"
          branch="develop"
        />
        <ProjectTile
          :project="{ path: '/Users/you/dev/holiday-curb', name: 'holiday-curb', tracked: true }"
          state="loud"
          state-label="1 waiting on you"
          branch="main"
        />
        <ProjectTile
          :project="{ path: '/Users/you/dev/beads-viewer', name: 'beads-viewer', tracked: true }"
          state="live"
          state-label="2 running"
          branch="main"
        />
        <ProjectTile
          :project="{ path: '/Users/you/notes', name: 'notes', tracked: false }"
          state="idle"
          state-label="idle"
        />
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Dropdown, and the branch picker built on it</div>
      <!-- Click them. The panel, the filter and the list are Dropdown's; the
           branch picker adds "New branch" and the naming state, and none of
           those can be reached by a prop, which is why both are here on their
           own and not only inside the dialog. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <!-- The same panel without the two things only a branch field needs:
             this is what the rest of the run dialog uses. -->
        <div :style="{ width: '220px' }">
          <Dropdown
            v-model="choice"
            :options="[
              { value: 'ready', label: 'Autopilot' },
              { value: 'running', label: 'Crew' },
              { value: 'done', label: 'Solo' }
            ]"
          />
        </div>
        <!-- The agent picker the settings window draws, built the way that
             window builds it: one row per harness this build ships, named by
             Rust through `agents::catalogue` and `stores/agents.js`. Nothing
             here is written out, so a harness added in Rust appears in this
             page too.

             It used to draw Codex as a row the list named and could not pick,
             which is what demonstrated a `disabled` option with a note beside
             it. That limit is gone; the mode field below still shows two. -->
        <div :style="{ width: '220px' }">
          <Dropdown
            v-model="pickedAgent"
            :options="agents.map((row) => ({ value: row.id, label: row.label }))"
          />
        </div>
        <!-- The same flag against captions and a filter, and two things worth
             looking at. The field opens on Solo, which cannot be picked: no row
             carries the check, since what is held is said by the field and the
             list says what can be set. And "Later" is drawn nowhere at all — a
             caption whose whole group is unavailable is a heading over nothing,
             exactly as one filtered down to nothing is. Type in the filter and
             the same rule prunes "Available". -->
        <div :style="{ width: '220px' }">
          <Dropdown
            v-model="pickedMode"
            searchable
            search-label="Search modes"
            :options="[
              { header: true, label: 'Available' },
              { value: 'ready', label: 'Autopilot' },
              { value: 'running', label: 'Crew', disabled: true, note: 'Not supported yet' },
              { header: true, label: 'Later' },
              { value: 'done', label: 'Solo', disabled: true, note: 'Not supported yet' }
            ]"
          />
        </div>
        <div :style="{ width: '320px' }">
          <BranchSelect
            v-model="pickedBranch"
            v-model:create="branchIsNew"
            :branches="everywhere('main', 'staging', 'feature/runs-project-config', 'fix/tooltip-clipping', 'release/7')"
          />
        </div>
        <div :style="{ width: '320px' }">
          <!-- A branch some of the project's repositories are missing, against
               the field above it, whose branches are all everywhere and which
               therefore draws no glyph at all. Here there is an info glyph after
               the name, and holding the pointer over it says "may be created
               where it is missing" — a permission rather than a plan, since a
               run cuts the branch only in the repositories the task touches and
               this dialog does not know which those are. The two sentences stay
               apart: "will be created" on its own would say the branch is
               nowhere, which is the very thing that sent a run out to cut a
               `develop` that already had its own history in four repositories.
               The other half is fact rather than permission and stays words,
               inside the panel where the row has a line to itself: open it for
               "not in admin, extension".

               The name it opens on is a long one on purpose. That is what the
               glyph is for — the phrase used to stand in the field beside the
               name, and the two filled it between them — so this is the field
               to look at for the name giving way rather than the glyph. -->
          <BranchSelect
            v-model="partialBranch"
            :branches="[
              { name: 'develop', missing_in: [] },
              { name: 'main', missing_in: [] },
              { name: 'feature/runs-project-config', missing_in: ['extension'] },
              { name: 'release/7', missing_in: ['admin', 'extension'] },
              { name: 'spike/auth', missing_in: ['frontend', 'admin', 'extension'] }
            ]"
          />
        </div>
        <div :style="{ width: '320px' }">
          <!-- Captions and per-row notes. Deliberately long enough to scroll,
               since the caption rows are what `reveal` has to walk past: the
               cursor is brought into view by index into this list's children, so
               a row that is not a sibling of the options would desync it. -->
          <Dropdown
            v-model="groupedBranch"
            mono
            searchable
            search-label="Search branches"
            :options="[
              { header: true, label: 'Everywhere' },
              { value: 'develop', label: 'develop' },
              { value: 'main', label: 'main' },
              { value: 'staging', label: 'staging' },
              { header: true, label: 'Not everywhere' },
              { value: 'release/7', label: 'release/7', note: 'not in admin, extension' },
              { value: 'spike/auth', label: 'spike/auth', note: 'not in frontend, admin, extension' },
              { value: 'hotfix/2026-08', label: 'hotfix/2026-08', note: 'not in extension' },
              { value: 'chore/deps', label: 'chore/deps', note: 'not in backend' },
              { value: 'wip/editor', label: 'wip/editor', note: 'not in admin' }
            ]"
          />
        </div>
        <!-- The same list in a field too narrow for its longest note. The
             example above is wide enough that nothing ever overflows, so it
             cannot show which of the two gives way — and that is exactly the
             thing worth looking at: the branch name has to survive whole and
             the note has to clip. -->
        <div :style="{ width: '210px' }">
          <Dropdown
            v-model="narrowBranch"
            mono
            searchable
            search-label="Search branches"
            :options="[
              { header: true, label: 'Everywhere' },
              { value: 'develop', label: 'develop' },
              { header: true, label: 'Not everywhere' },
              { value: 'release/7', label: 'release/7', note: 'not in admin, extension' },
              { value: 'spike/auth', label: 'spike/auth', note: 'not in frontend, admin, extension' }
            ]"
          />
        </div>
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Run bar</div>
      <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-5)', alignItems: 'flex-start' }">
        <RunBar :run="runFixture({ kind: 'preflight' })" @stop="() => {}" />
        <RunBar :run="runFixture({ kind: 'working', iteration: 2 }, { batches: 3 })" @stop="() => {}" />
        <!-- Stopping is a state of its own on screen: the batch in flight is
             still going, and a bar that went on saying "Batch 3" would read as
             the button having done nothing. -->
        <RunBar :run="runFixture({ kind: 'working', iteration: 2 }, { batches: 3, stopping: true })" @stop="() => {}" />
        <!-- A batch running smaller than was asked for, because the allowance
             is low. The reduction is qualified working, not a state of its own
             — hence a field on the run, the way `stopping` is. -->
        <RunBar :run="runFixture({ kind: 'working', iteration: 2 }, { batches: 3, reduced: 78 })" @stop="() => {}" />
        <!-- Neither working nor over: the allowance is spent and the run is
             waiting for it, which is why the glyph is a third silhouette and
             the stop button is still there. The reset line is the harness's own
             sentence, passed through untouched — the app never parses it. -->
        <RunBar
          :run="runFixture({ kind: 'paused', pct: 92, resets: 'Aug 11 at 5:59pm (Europe/Moscow)' })"
          @stop="() => {}"
        />
        <!-- The same pause where the harness said nothing about a reset. A bare
             line would read as a hang. -->
        <RunBar :run="runFixture({ kind: 'paused', pct: 92, resets: null })" @stop="() => {}" />
        <!-- Two runs paused on the same reading, the way the footer draws them:
             the subscription is one per machine, so the sentence about it is
             written once. The first carries the words and both buttons; the
             second is the same state with `speaks` false — the pause glyph and
             its own Stop, and nothing else. Drawn as a row rather than stacked,
             because what is being checked is that the pair reads as one
             statement and one silent neighbour.

             The gap is `--space-4` and not this section's own `--space-5`,
             because it stands in for a real one: in the app the segments sit in
             `StatusFooter`'s status slot, whose row is spaced at `--space-4`.
             A wider one here would draw the pair further apart than the footer
             ever does and hide exactly what this cell is for. -->
        <div :style="{ display: 'flex', gap: 'var(--space-4)', alignItems: 'center' }">
          <RunBar
            :run="runFixture({ kind: 'paused', pct: 90, resets: 'Sep 1 at 6pm (Europe/Moscow)' })"
            @stop="() => {}"
            @release="() => {}"
          />
          <RunBar
            :run="runFixture({ kind: 'paused', pct: 90, resets: 'Sep 1 at 5:59pm (Europe/Moscow)' })"
            :speaks="false"
            @stop="() => {}"
            @release="() => {}"
          />
        </div>
        <!-- The other pause: the batch before this one died on an allowance
             that is still spent (`usage::held`). Word for word the same
             sentence, and no "Run anyway" — the button would work and the
             session it let through would die at once. -->
        <RunBar
          :run="runFixture({ kind: 'paused', pct: 96, resets: 'Sep 1 at 6pm (Europe/Moscow)', spent: true })"
          @stop="() => {}"
          @release="() => {}"
        />
        <RunBar :run="runFixture({ kind: 'stopped', reason: { kind: 'queue_empty' } })" />
        <RunBar :run="runFixture({ kind: 'stopped', reason: { kind: 'no_progress' } })" />
        <RunBar :run="runFixture({ kind: 'stopped', reason: { kind: 'crashed', attempts: 5 } })" />
        <!-- Beside `crashed` and beside `no_progress` on purpose: these three
             are the endings that send somebody somewhere, and this one has to
             be told from both at a glance. Every session exited cleanly and
             none of them did anything, so the count is the detail line. -->
        <RunBar :run="runFixture({ kind: 'stopped', reason: { kind: 'nothing_done', batches: 3 } })" />
        <!-- Somebody's own doing, like a stop, and quiet for that reason — but
             a different act, and the line is where the two are told apart. -->
        <RunBar :run="runFixture({ kind: 'stopped', reason: { kind: 'session_removed' } })" />
        <!-- Nobody was watching and the agent asked something anyway. The
             question takes the detail line, since what it asked is what decides
             whether somebody goes and answers it in the terminal. -->
        <RunBar
          :run="
            runFixture({
              kind: 'stopped',
              reason: { kind: 'needs_answer', question: 'Do you trust the contents of this directory?' }
            })
          "
        />
        <!-- The project would not come up, and the detail line is the whole of
             what somebody can act on: without it this reads as "Could not start
             into develop", a sentence naming the target branch, which had
             nothing to do with the tool that was missing. -->
        <RunBar
          :run="
            runFixture({
              kind: 'stopped',
              reason: {
                kind: 'preflight',
                detail: '`docker compose -f backend/docker-compose.yml up -d` exited 127: sh: docker: command not found'
              }
            })
          "
        />
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Run report</div>
      <!-- The frame is given a box and has to fill it exactly: the document
           paints its own ground, so anything of ours showing through is a strip
           of the wrong colour. Two boxes rather than one, because the second is
           the case that matters — a buffer still loading, or one that failed to
           read, hands the component an empty string, and what shows then is the
           host's own token ground rather than whatever sits behind the centre
           column. Height is a token multiple for the same reason the terminal's
           is: the frame fills whatever it is given. -->
      <div
        :style="{
          display: 'flex',
          height: 'calc(var(--space-9) * 8)',
          border: 'var(--border-w) solid var(--border)'
        }"
      >
        <ReportView :html="REPORT_HTML" :theme="theme" />
      </div>
      <div
        :style="{
          display: 'flex',
          height: 'calc(var(--space-9) * 2)',
          border: 'var(--border-w) solid var(--border)'
        }"
      >
        <ReportView html="" :theme="theme" />
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Agent output</div>
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <div :style="{ width: '360px' }">
          <ChatMessage role="user" time="14:02">Rename the worktree when the branch changes.</ChatMessage>
          <ChatMessage author="claude-1" time="14:03" streaming>
            Looking at the collision in
            <CodeBlock
              language="rust"
              filename="src/worktree.rs"
              :start-line="118"
              code="fn rename(&mut self, name: &str) -> Result<()> {
    // collides with an existing worktree
    let path = self.root.join(name);
}"
            />
          </ChatMessage>
        </div>
        <div :style="{ width: '360px', display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }">
          <ToolCall name="read_file" args="src/tabs.rs" duration="12ms" result="ok" expanded />
          <ToolCall name="cargo_test" args="--workspace" state="running" duration="2m 04s" />
          <ToolCall name="git_push" args="wt/bd-a1b2" state="error" result="exit 101" />
          <CodeBlock
            diff
            filename="src/tabs.rs"
            code="+let name = branch.replace('/', '-');
-let name = branch.to_string();
~let path = root.join(&name);"
          />
        </div>
        <div :style="{ width: '360px' }">
          <LogView :lines="logLines" :height="220" stream-state="paused" :follow="false" />
        </div>
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Settings window</div>
      <!-- Every tab of the settings window, side by side rather than behind a
           tab bar: this harness is for seeing every component at once, and a
           tab strip here would hide all but one behind a click. The values
           are local refs — in the app they arrive from the main window and go
           back to it as events, and neither end exists here. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', flexWrap: 'wrap', alignItems: 'flex-start' }">
        <div :style="{ width: '380px' }">
          <GeneralSettings
            :theme="galleryTheme"
            :ui-font-size="galleryUiFont"
            :autostart-supported="galleryAutostartSupported"
            :autostart-enabled="galleryAutostartEnabled"
            :restore-geometry="galleryRestoreGeometry"
            :updates-auto-check="galleryUpdatesAutoCheck"
            :notification-run-finished="galleryRunSound"
            :notification-needs-attention="galleryNeedsSound"
            :notification-show-report="galleryShowReport"
            :notification-only-when-unfocused="galleryOnlyWhenUnfocused"
            @update:theme="galleryTheme = $event"
            @update:ui-font-size="galleryUiFont = $event"
            @update:autostart-enabled="galleryAutostartEnabled = $event"
            @update:restore-geometry="galleryRestoreGeometry = $event"
            @update:updates-auto-check="galleryUpdatesAutoCheck = $event"
            @update:notification-run-finished="galleryRunSound = $event"
            @update:notification-needs-attention="galleryNeedsSound = $event"
            @update:notification-show-report="galleryShowReport = $event"
            @update:notification-only-when-unfocused="galleryOnlyWhenUnfocused = $event"
          />
        </div>
        <div :style="{ width: '380px' }">
          <GitSettings
            :auto-fetch="galleryGitAutoFetch"
            :remove-worktrees="galleryRemoveWorktrees"
            @update:auto-fetch="galleryGitAutoFetch = $event"
            @update:remove-worktrees="galleryRemoveWorktrees = $event"
          />
        </div>
        <div :style="{ width: '380px' }">
          <EditorSettings
            :font-size="galleryEditorFont"
            :word-wrap="galleryEditorWordWrap"
            @update:font-size="galleryEditorFont = $event"
            @update:word-wrap="galleryEditorWordWrap = $event"
          />
        </div>
        <!-- Wider than the 380 px the other settings cells take, and it has to
             be: this tab now holds a row asking for a 48ch control, which is
             wider than that cell, so at 380 px the harness would only ever show
             the row giving way rather than the row as the window draws it. -->
        <div :style="{ width: '560px' }">
          <AgentSettings
            :agent="galleryAgent"
            :model="galleryAgentModel"
            :agent-roles="galleryAgentRoles"
            :agent-language="galleryAgentLanguage"
            :task-language="galleryTaskLanguage"
            :commit-language="galleryCommitLanguage"
            :report-language="galleryReportLanguage"
            :agent-prompt="galleryAgentPrompt"
            :usage="galleryAgentUsage"
            @update:agent-role="galleryChooseRole($event)"
            @update:agent-language="galleryAgentLanguage = $event"
            @update:task-language="galleryTaskLanguage = $event"
            @update:commit-language="galleryCommitLanguage = $event"
            @update:report-language="galleryReportLanguage = $event"
            @update:agent-prompt="galleryAgentPrompt = $event"
          />
        </div>
        <!-- The same tab with Show run report off on the General tab, which is
             the other state the Report language row has: the control is drawn
             and cannot be pressed, and the description names the reason and the
             tab the switch is on. The chosen language is still handed in and
             still stands, which is what says the switch shuts the row rather
             than the setting.

             It carries the other state of the Run limits rows as well — Pause a
             run at turned off — because off is the value most likely to be
             drawn wrongly and this is what makes it visible without editing
             `settings.json`. Take fewer tasks at stays on its shipped 75, so
             the two shapes sit side by side in one cell. -->
        <!-- Wider than the 380 px the other settings cells take, and it has to
             be: this tab now holds a row asking for a 48ch control, which is
             wider than that cell, so at 380 px the harness would only ever show
             the row giving way rather than the row as the window draws it. -->
        <div :style="{ width: '560px' }">
          <AgentSettings
            :agent="galleryAgent"
            :model="galleryAgentModel"
            :agent-roles="galleryAgentRoles"
            :agent-language="galleryAgentLanguage"
            :task-language="galleryTaskLanguage"
            :commit-language="galleryCommitLanguage"
            :report-language="galleryReportLanguage"
            :show-report="false"
            :subscription-pause-at="0"
            :usage="galleryAgentUsage"
          />
        </div>
        <!-- The subscription block in its other shapes, the way the Storage tab
             below is drawn in three: an agent that does not answer the question
             at all, one that was asked and could not, half a reading, and a
             probe still out. The first is the one with a layout of its own —
             the Refresh button is gone rather than disabled — the third is the
             one row a half-read allowance draws, and the last is the only place
             the disabled button can be looked at. The three rows above them are
             along for the ride; the block is what these are for. -->
        <div :style="{ width: '560px' }">
          <AgentSettings agent="codex" :usage="galleryAgentUsageUnsupported" />
        </div>
        <div :style="{ width: '560px' }">
          <AgentSettings :usage="galleryAgentUsageUnreadable" />
        </div>
        <div :style="{ width: '560px' }">
          <AgentSettings :usage="galleryAgentUsageHalf" />
        </div>
        <div :style="{ width: '560px' }">
          <AgentSettings busy />
        </div>
        <div :style="{ width: '380px' }">
          <KanbanSettings
            :columns="galleryKanbanColumns"
            :always-show="galleryKanbanAlwaysShow"
            :interval="galleryKanbanInterval"
            :unlimited="galleryKanbanUnlimited"
            :board-columns="galleryBoardColumns"
            @update:columns="galleryKanbanColumns = $event"
            @update:always-show="galleryKanbanAlwaysShow = $event"
            @update:interval="galleryKanbanInterval = $event"
            @update:unlimited="galleryKanbanUnlimited = $event"
          />
        </div>
        <!-- The Storage tab in the three states worth looking at: something to
             delete, with the count and the size a person reads before pressing;
             nothing to delete, where the button is dead and the description
             says which of the empties it is; and a board that could not be
             read, where the button is dead because nothing can vouch for a
             single file. Nothing is behind any of them here — pressing emits
             and the harness does nothing, which is the whole point of a
             presentational component. -->
        <div :style="{ width: '380px' }">
          <StorageSettings :survey="gallerySurvey" />
        </div>
        <div :style="{ width: '380px' }">
          <StorageSettings :survey="galleryEmptySurvey" :cleaned="galleryCleaned" />
        </div>
        <div :style="{ width: '380px' }">
          <StorageSettings :survey="galleryNoBoardSurvey" />
        </div>
        <!-- About with nobody to ask about updates, which is what a browser
             sees and what this file is opened in: no row at all, rather than a
             row saying nothing. -->
        <div :style="{ width: '380px' }">
          <AboutSettings version="0.1.0" />
        </div>
        <!-- And the six states of the update machine, in the order a person
             meets them. The last of them is the refusal: `ready` with a run
             going somewhere, where the control stays live and the reason is in
             words underneath — a control that will not act and will not say why
             sends somebody to guess. -->
        <div
          v-for="(updateState, at) in galleryUpdateStates"
          :key="at"
          :style="{ width: '380px' }"
        >
          <AboutSettings version="0.1.0" :update-state="updateState" />
        </div>
        <div :style="{ width: '380px' }">
          <AboutSettings
            version="0.1.0"
            :update-state="{ kind: 'ready', version: '0.2.0' }"
            :update-refusal="galleryUpdateRefusal"
          />
        </div>
        <div :style="{ width: '380px' }">
          <SettingsRow label="A row on its own" description="Label, one line of explanation, and whatever control the setting needs.">
            <Switch :model-value="switched" label="" @update:model-value="switched = $event" />
          </SettingsRow>
        </div>
        <!-- The group, both ways round: named, which is what a tab's own
             sections use, and headerless, which is what the Kanban tab's lists
             of columns use under a caption of their own. The row above the
             first group is there on purpose — the gap over a caption and the
             spine's two ends are the whole of what this component draws, and
             neither can be seen without something above it to be separated
             from. -->
        <div :style="{ width: '380px' }">
          <SettingsRow label="Ungrouped row" description="Above the first group, so the gap over the caption can be seen.">
            <Switch :model-value="switched" label="" @update:model-value="switched = $event" />
          </SettingsRow>
          <SettingsGroup label="A named group">
            <SettingsRow label="First in the group" description="The spine starts at this row's top edge.">
              <Switch :model-value="switched" label="" @update:model-value="switched = $event" />
            </SettingsRow>
            <SettingsRow label="Last in the group" description="And ends on this row's own bottom rule.">
              <Switch :model-value="switched" label="" @update:model-value="switched = $event" />
            </SettingsRow>
          </SettingsGroup>
          <SettingsGroup>
            <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-3)', padding: 'var(--space-4) 0' }">
              <Checkbox :model-value="checked" label="Headerless, for a list under a caption of its own" @update:model-value="checked = $event" />
              <Checkbox :model-value="!checked" label="Spine and indent, no caps caption" @update:model-value="checked = !$event" />
            </div>
          </SettingsGroup>
        </div>
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Overlays and states</div>
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <ContextMenu :items="menuItems" />
        <!-- The same rows a card's menu holds, built by the same rule, with the
             trigger a card draws: this is the one place the submenu, the
             keyboard walk and the flipping can be looked at without a board
             behind them.

             At the menu's own width — `MENU_W`, imported rather than written
             out, since the measurement has one home. The second one is the
             case that width exists for — the greyed Run row carries
             the whole of `scopeBusyReason`'s sentence, and at 200 it is
             ellipsised with no tooltip to recover it. Checking the fix means
             seeing it at the size the board actually draws. -->
        <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-4)', alignItems: 'flex-start' }">
          <MenuButton :items="CARD_MENU" :width="MENU_W" label="Actions for bd-a1b2" @select="() => {}" />
          <MenuButton :items="BUSY_CARD_MENU" :width="MENU_W" label="Actions for bd-77e0" @select="() => {}" />
          <MenuButton :items="PARKED_CARD_MENU" :width="MENU_W" label="Actions for bd-29j1" @select="() => {}" />
          <MenuButton :items="DONE_CARD_MENU" :width="MENU_W" label="Actions for bd-5f01" @select="() => {}" />
        </div>
        <!-- And one at the component's own default, which is what a caller with
             short verbs gets: the width is the caller's business, so both ends
             of it belong here. -->
        <MenuButton :items="menuItems" label="Worktree actions" @select="() => {}" />
        <!-- The pointer-anchored panel, which has no trigger to draw: these two
             boxes stand in for the rows it opens over in the branch list. The
             second is a menu whose every verb is refused, which is the state
             worth looking at — the reason is a caption above the group rather
             than a clause repeated on each row. -->
        <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-4)' }">
          <div :style="menuTargetStyle" @contextmenu.prevent="branchMenu?.open($event)">
            Right-click for a branch menu
          </div>
          <div :style="menuTargetStyle" @contextmenu.prevent="refusedBranchMenu?.open($event)">
            Right-click for a refused one
          </div>
          <PointerMenu ref="branchMenu" :items="BRANCH_MENU" :width="280" @select="() => {}" />
          <PointerMenu ref="refusedBranchMenu" :items="REFUSED_BRANCH_MENU" :width="280" @select="() => {}" />
          <!-- The same rows, drawn straight into a `ContextMenu` with no
               gesture in front of them — `fileMenuItems` two groups up has the
               same pair for the same reason. The two boxes above are the whole
               interaction and are the only place the panel's placement and
               flipping can be tried; they are also behind a right-click, which
               is a gesture an automated pass cannot reliably raise and a person
               has to remember to make. So the rows themselves get a frame that
               is simply on the page: the two glyphs this task added (`star`,
               `trash-2`) sit in the gutter with the four that were already
               there, the labels have to fit `:width="280"` without an ellipsis,
               and the separators have to land in three places rather than two.
               Live, from the rule itself, so it cannot drift from what the menu
               actually offers. -->
          <ContextMenu :items="BRANCH_MENU" :width="280" />
          <!-- And the marked state of the same menu, which is the one row whose
               text depends on something: `Remove from favourites` is the longer
               of the two labels and is what the width has to hold. -->
          <ContextMenu :items="MARKED_BRANCH_MENU" :width="280" />
        </div>
        <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }">
          <Toast tone="warning" title="claude-1 needs you" description="bd-a1b2 · worktree name collision · 4m" />
          <Toast tone="error" title="claude-2 failed" description="exit 101 in wt/bd-3c9d" />
          <Toast tone="success" title="bd-12cd done" description="+41 −1 · 2h 14m" />
          <!-- What the Git panel says after a merge git carried through, and
               the one place the phrase can be seen by eye at all:
               `mockBackend.js` refuses every git write on purpose, so a browser
               can never produce this toast for real. Both halves are drawn —
               what came in, and the answer when nothing did, which is the case
               the whole feature exists for. The minus is U+2212. -->
          <Toast
            tone="success"
            title="Merged feature/x"
            description="3 commits · 7 files · +41 −12"
          />
          <Toast
            tone="success"
            title="Nothing to merge"
            description="feature/x is already in main"
          />
        </div>
        <!-- The bell's panel, both ways round: what it looks like holding
             something, and what it says holding nothing. Drawn inline here
             rather than hanging off a corner — placement belongs to whoever
             opens it, the same split ContextMenu and MenuButton keep. -->
        <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }">
          <NotificationPanel :items="galleryNotifications" />
          <NotificationPanel />
        </div>
        <div :style="{ width: '360px' }">
          <NotificationCard :notification="galleryNotifications[0]" />
        </div>
        <div :style="{ width: '220px' }">
          <Skeleton :lines="4" :height="10" />
        </div>
        <EmptyState title="No board yet" description="Connect a tracker to pull tasks, or create the first task locally." icon="columns-3" />
        <EmptyState tone="error" title="Tracker unreachable" description="bd exited 101." />
        <!-- The `detail` slot: what the failing thing said, in mono under the
             sentence about it, never wrapped. The line is deliberately longer
             than the slot's own 420px cap, so what this entry shows is the
             ellipsis — which is the whole of the feature and the part a caller
             cannot see any other way. -->
        <EmptyState
          tone="error"
          title="bd is failing"
          description="The tracker cannot be read. Repairing takes a copy of .beads first."
        >
          <template #detail>bd list --all -n 0 --json exited with code 1: failed to open store: schema version 41 is older than 53</template>
          <template #action>
            <div :style="{ display: 'flex', gap: 'var(--space-3)' }">
              <Button variant="primary" size="sm">Repair tracker</Button>
              <Button variant="ghost" size="sm">Ask an agent</Button>
            </div>
          </template>
        </EmptyState>
        <!-- The other half of that pair, and the reason it is drawn beside it:
             the two used to be one state, so a failing bd and a folder the
             operating system refuses both said "bd is failing" and both offered
             a database migration for it.

             All three forms, because which one a person sees depends on where
             their project is and what they are running, and none of the three is
             reachable from the other two by looking. The copy is
             `views/folderAccess.js`, kept in one place so these entries and the
             app cannot come to say different things; only the first has a
             button. The detail line is the path that was actually refused. -->
        <EmptyState v-bind="folderRefusedNotice('reset')">
          <template #detail>no permission to read /Users/you/Desktop/Projects/smetana</template>
          <template #action>
            <Button variant="primary" size="sm">Reset and restart</Button>
          </template>
        </EmptyState>
        <EmptyState v-bind="folderRefusedNotice('full-disk-access')">
          <template #detail>no permission to read /Users/you/Library/Mobile Documents/smetana</template>
        </EmptyState>
        <EmptyState v-bind="folderRefusedNotice('unavailable')">
          <template #detail>no permission to read /home/you/projects/smetana/.beads</template>
        </EmptyState>
      </div>
      <div :style="{ position: 'relative', height: '220px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <Modal title="Discard worktree?" description="wt/bd-a1b2 has 3 uncommitted files and 1 agent still running.">
          <div :style="{ fontSize: 'var(--text-sm)', color: 'var(--text-secondary)' }">
            The branch feat/worktree-rename stays; only the working tree is removed.
          </div>
          <template #footer>
            <Button variant="ghost">Cancel</Button>
            <Button variant="danger">Discard</Button>
          </template>
        </Modal>
      </div>
      <!-- What a thumbnail in the new-task dialog's images strip opens, which
           is a window of its own in the app (`views/ImageWindow.vue`). Framed
           the way the dialogs above it are: the viewer covers the nearest
           positioned ancestor, so the frame is what stands in here for that
           window. A picture larger than the frame is the state worth looking
           at — fitted whole, nothing cropped off it, and no scrollbar
           anywhere. -->
      <div :style="{ position: 'relative', height: '320px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <ImageViewer :url="WIDE_ATTACHMENT.url" :name="WIDE_ATTACHMENT.name" @close="() => {}" />
      </div>
      <!-- The other end of the same rule, on the eight-pixel fixture the strip
           draws: a picture smaller than the frame is left at its own size
           rather than blown up to fill it. -->
      <div :style="{ position: 'relative', height: '220px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <ImageViewer :url="ATTACHMENTS[0].url" :name="ATTACHMENTS[0].name" @close="() => {}" />
      </div>
    </section>
  </div>
</template>
