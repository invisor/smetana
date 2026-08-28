<script setup>
/* Dev harness: renders every component in the library once, so a broken port
   shows up here rather than in the product. Not part of the shipped app —
   reachable at ?view=gallery. */
import { computed, ref, watchEffect } from 'vue'
import { orderColumns } from '../components/kanban/columnOrder.js'
import { branchMenuItems } from '../components/git/branchMenu.js'
import { fileMenuItems } from '../components/files/fileMenu.js'
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
  DeleteSessionModal,
  DeleteTaskModal,
  DependencyMark,
  DiffView,
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
  ScopeIndicator,
  SectionHeader,
  SegmentedTabs,
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
  UsageFooter,
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
(ready_to_merge).</p></div>
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
const partialBranch = ref('release/7')
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
  { id: 1, label: null, tasks: ['smetana-42'], state: 'needs-you', elapsed: '2h 14m' },
  /* A run holding several. Also the longest caption the list can be asked to
     draw, and therefore the one that says whether the elapsed time and the
     remove button still have room. */
  { id: 2, label: null, tasks: ['smetana-42', 'smetana-9je', 'smetana-hvw'], state: 'running', elapsed: '1h 02m' },
  { id: 3, label: 'Editing', tasks: ['smetana-8av'], state: 'running', elapsed: '41m' },
  { id: 4, label: 'Creating a task', tasks: [], state: 'running', elapsed: '3m' },
  { id: 5, label: 'Project setup', tasks: [], state: 'done', elapsed: '18m' },
  /* A bare agent, and also a run that has not claimed anything yet: the same
     caption, deliberately — it is an agent, and there is no work to name. */
  { id: 6, label: 'Agent', tasks: [], state: 'ready', elapsed: '2m' },
  /* An agent the worker has not answered about yet: the word in place of a
     time, and a remove button with nothing to remove. Captioned exactly as it
     will be once the session lands, so the handover moves nothing on screen.
     It lasts about a second in the app, which is exactly why it belongs here —
     the only place it can be looked at for longer than that. */
  { id: 'start-1', label: 'Creating a task', tasks: [], state: 'running', elapsed: 'starting', starting: true }
]

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

   Six rows and no two of them the same case: subagents and none, a title long
   enough to ellipsise, a session out of a worktree, one with no branch, and one
   with neither title nor last message — the row's two fallbacks, which appear
   nowhere else. Newest first, as the store sorts them: this page draws rows
   rather than the store, so the order is written out here by hand. */
const GALLERY_SESSION_NOW = Date.parse('2026-08-28T12:00:00Z')
const galleryAt = (ms) => new Date(GALLERY_SESSION_NOW - ms).toISOString()
const GALLERY_SESSIONS = [
  {
    id: '3a7e5b10-1c2d-4e3f-9a8b-7c6d5e4f3a2b',
    path: '/Users/you/.claude/projects/-Users-you-dev-smetana/3a7e5b10.jsonl',
    cwd: '/Users/you/dev/smetana',
    branch: 'develop',
    title: 'Why does the scope bar count dirty files it cannot see',
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
    branch: 'main',
    title:
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
    branch: 'feature/smetana-oln-sessions-tab-disk-history',
    title: 'Implement the front-end half of the sessions tab',
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
    branch: null,
    title: 'Check whether the sidecar digest matches the pinned release',
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
    branch: 'main',
    title: null,
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
    branch: 'staging',
    title: 'Port the branch list to the design system',
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

   `action` is answered with the same copy policy the app keeps — literally the
   same, `useCopyFeedback`, rather than a second writing of it — and with
   nothing at all for the four verbs that reach a desktop or a disk: a gallery
   has neither, and the menu opening, walking and closing is what there is to
   check here. */
const openSessions = ref([GALLERY_SESSIONS[1].id])

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

/* The caption on its own, in all three of the states it can be in: unfolded
   with a count, unfolded without one, and folded — which still carries its
   count, because somebody who folds the branches away is saying they do not
   want to read the list, not that they no longer want to know there are nine
   of them. */
const headerFolds = ref({ withCount: true, bare: true, folded: false, withActions: true })
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
const FILE_MENU = fileMenuItems({ target: 'file', canAttach: true, hasLiveAgent: true })
const ARMED_FILE_MENU = fileMenuItems({
  target: 'file',
  canAttach: true,
  hasLiveAgent: true,
  confirmingDelete: true
})

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
      <!-- The header that can move its whole column into the queue, and the same
           header with nothing to move: the button is drawn off the count, so an
           empty column carries none — the 0 beside it already says why. -->
      <div :style="{ display: 'flex', flexWrap: 'wrap', gap: 'var(--space-4)' }">
        <div :style="{ width: '176px' }">
          <ColumnHeader status="deferred" :count="12" :addable="false" promotable @promote="() => {}" />
        </div>
        <div :style="{ width: '176px' }">
          <ColumnHeader status="deferred" :count="0" :addable="false" promotable @promote="() => {}" />
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
      <!-- Both strips stand in a frame, and the frame is not decoration here.
           A thumbnail opens the image viewer, which covers the nearest
           positioned ancestor — the dialog's own scrim in the app. With nothing
           positioned around the strip there is no such ancestor short of the
           page, so a click in these two cells would put the viewer over the
           whole gallery. The box is what the dialog stands in for. -->
      <div :style="{ position: 'relative', width: '340px', height: '260px', padding: 'var(--space-5)', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <AttachmentStrip :items="ATTACHMENTS" @remove="() => {}" />
      </div>
      <!-- Past two rows the strip scrolls instead of growing: nothing bounds
           how many images are attached, and the dialog has no scrolling of its
           own to absorb them. -->
      <div :style="{ position: 'relative', width: '400px', height: '260px', padding: 'var(--space-5)', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
        <AttachmentStrip :items="MANY_ATTACHMENTS" @remove="() => {}" />
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
        <div :style="{ position: 'relative', width: '480px', height: '800px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <RunModal
            :open="true"
            :scope="{ kind: 'task', id: 'smetana-77', title: 'Fold the settings debounce into the store' }"
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
           has to survive is the name, the counters and the two buttons meeting
           in one row, and a box would answer at a width nobody uses.

           The counters are why there are five of them. Each is drawn only above
           zero, and an unknown number of uncommitted files — `null`, what
           `stores/vcs.js` hands over when the working tree could not be read —
           draws nothing at all, exactly as a clean tree does. The two look
           identical on screen on purpose, so the pair at the bottom is the only
           place the difference can be seen against its own props.

           The singulars are hover-only, being tooltips: point at the file and
           agent counters in the third bar for "1 uncommitted file" and "1 agent
           running", and at its bell for "1 notification".

           The three below them are the headline, which the five above draw
           none of — the empty case is the common one and has to be seen as the
           bar closing up rather than as a gap. -->
      <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }">
        <!-- A worktree with a branch checked out in it: `scopeName` draws the
             worktree, and the branch follows it after an @. Both counters
             plural, and a count on the bell. -->
        <ScopeIndicator
          repo="smetana"
          worktree="smetana-f69-scope-indicator"
          branch="feature/smetana-f69-scope-indicator"
          :dirty-count="7"
          :agents-active="3"
          :notifications="2"
        />
        <!-- No worktree, which is what the app itself passes today: `scopeName`
             falls back to the branch and there is no @ segment after it. -->
        <ScopeIndicator
          repo="holiday-curb"
          branch="develop"
          :dirty-count="12"
          :agents-active="2"
        />
        <!-- Ones, in all three places there is a noun to get wrong. -->
        <ScopeIndicator
          repo="smetana"
          branch="main"
          :dirty-count="1"
          :agents-active="1"
          :notifications="1"
        />
        <!-- Unknown rather than zero: no file glyph and no number, with the
             agents counter beside it to show the bar is otherwise alive. -->
        <ScopeIndicator
          repo="beads-viewer"
          branch="develop"
          :dirty-count="null"
          :agents-active="2"
        />
        <!-- A clean tree with nothing running: the same nothing as above from
             the opposite fact, and a bell with no badge on it. -->
        <ScopeIndicator
          repo="tracker-notes"
          branch="main"
          :dirty-count="0"
          :agents-active="0"
        />

        <!-- Nothing to say, said explicitly: the props are there and empty, and
             the bar between the branch and the counters closes up. This is the
             one to compare the two below against. -->
        <ScopeIndicator
          repo="tracker-notes"
          branch="main"
          headline=""
          :dirty-count="2"
          :agents-active="0"
        />
        <!-- Live: muted, no glyph, and beside the agents counter it is a
             sentence rather than a number. It carries a run segment as well,
             because in the app this sentence is only ever drawn beside one —
             this is the crowded case, and the one to narrow the window on: the
             sentence is what gives way, and the counters and the two buttons
             stay where they are. -->
        <ScopeIndicator
          repo="holiday-curb"
          branch="develop"
          headline="Run under way"
          headline-level="live"
          :dirty-count="4"
          :agents-active="2"
        >
          <template #status>
            <RunBar :run="runFixture({ kind: 'working', iteration: 2 }, { batches: 3 })" @stop="() => {}" />
          </template>
        </ScopeIndicator>
        <!-- Loud, which is the case the glyph exists for: this bar is one of
             the one or two on a screen allowed to shout, and the colour is
             never the only thing saying so. -->
        <ScopeIndicator
          repo="smetana"
          branch="feature/smetana-ec9-scope-bar-headline"
          headline="1 agent needs you"
          headline-level="loud"
          :dirty-count="1"
          :agents-active="3"
          :notifications="1"
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
            :dirty-count="4"
            :agents-active="2"
            :notifications="1"
          />
        </div>
        <!-- Windows and Linux: no decorations at all, so the bar draws the
             three buttons itself, after the gear. -->
        <ScopeIndicator
          repo="smetana"
          branch="main"
          window-chrome="buttons"
          :dirty-count="4"
          :agents-active="2"
          :notifications="1"
        />
        <!-- The same bar over a maximized window: the middle button alone
             changes, to `copy` and "Restore". -->
        <ScopeIndicator
          repo="smetana"
          branch="main"
          window-chrome="buttons"
          maximized
          :dirty-count="4"
          :agents-active="2"
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
          headline="Run under way"
          headline-level="live"
          :dirty-count="4"
          :agents-active="2"
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

           The first is the wait: type `date` and the counter is a spinner while
           the heading stays `Matching text` and the rows stay the text ones —
           the agent has ninety seconds, and a heading that moved first would
           spend them lying. The second is an answer: type `date` and the heading
           becomes `By meaning` with the agent's own two ids in its own order.
           The third is a refusal, standing where the empty state would, in the
           failed colour and in the words `OneshotError` wrote — the handoff
           draws no error state at all, which is a hole rather than a decision.

           `answered` with no ids at all is the fourth, and it is the one worth
           looking at hardest: the agent looked and named nothing, which is a
           legitimate answer and gets a sentence of its own rather than the text
           mode's, because nobody checked any substrings. Type `date` into it. -->
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
          <MenuButton icon="plus" label="New agent, terminal or task" :items="NEW_TAB_ITEMS" :width="180" />
        </template>
      </TabBar>

      <!-- The other tab row, and the one that is not a tab row of files: the
           segmented strip under a side panel's header, drawn here at the width
           a panel gives it. Live rather than fixed, since the fill under the
           active segment and the fill under the pointer are the whole of what
           it draws — press one, and hover the other. -->
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
          <template #footer><UsageFooter :usage="galleryUsage[0]" /></template>
        </AppShell>
      </div>

      <!-- The rest of what the strip has to draw, one under the other, since
           the state is an answer from Rust rather than a control anybody can
           reach from here. In order: a half the harness did not print, drawn as
           a dash beside the half it did; an agent that does not report this at
           all; a fresh week's real `0`, which is a number and not a dash;
           nothing asked yet, which names nobody; a probe on its way, which keeps
           the last numbers and says so in the hint; and `invoke` refusing, which
           is the channel rather than an answer and says which. Hover any of
           them — the hint is the whole of the reset times, of what a run would
           do, and of why there is nothing to read. -->
      <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-4)' }">
        <UsageFooter :usage="galleryUsage[1]" />
        <UsageFooter :usage="galleryUsage[2]" />
        <UsageFooter :usage="galleryUsage[3]" />
        <UsageFooter />
        <UsageFooter :usage="galleryUsage[0]" busy />
        <UsageFooter error="the worker is not answering" />
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
         agent or Delete on it, and the only way to reach the second half of
         this menu in a project whose first screen is nothing but folders. -->
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

    <!-- The two halves of the menu that write to disk, in the states they are
         hard to reach in a browser: the draft row, which in the app appears
         only after a pick, and Delete asking a second time, which needs a panel
         that has stayed open through one.

         The draft rows are drawn between ordinary tree rows on purpose — this
         section exists to check one thing, that the row is a place in the tree
         and not something floating over it. Height, indent per level and type
         have to match the rows above and below exactly, in both densities and
         both themes; a field built out of `Input` would be `--control-h` tall
         and would say so by pushing everything under it down. -->
    <section :style="sectionStyle">
      <div :style="headStyle">File tree: making and deleting</div>
      <div :style="rowStyle">
        <div :style="{ width: '240px', border: 'var(--border-w) solid var(--border)', borderRadius: 'var(--radius-3)' }">
          <FileTreeRow name="src" kind="dir" :depth="0" expanded />
          <FileTreeDraftRow kind="file" :depth="1" :focus-on-mount="false" />
          <FileTreeRow name="App.vue" kind="file" :depth="1" />
          <FileTreeRow name="main.js" kind="file" :depth="1" />
          <FileTreeDraftRow kind="dir" :depth="0" :focus-on-mount="false" />
          <FileTreeRow name="Cargo.toml" kind="file" :depth="0" selected />
        </div>
        <ContextMenu :items="FILE_MENU" :width="300" />
        <ContextMenu :items="ARMED_FILE_MENU" :width="300" />
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
        <AgentList :rows="agentRows" :active-id="2" />
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
      <!-- The two states of an opened card the list above cannot show at once.
           The first is a session nobody said anything in: its prompt block
           carries the sentence that stands in for a first prompt, which is the
           only place that string appears. The second is a card whose menu is
           frozen while a delete runs against it — every row greyed, which is
           the state the app draws for the moment between Delete and the row
           leaving the list. -->
      <div :style="{ width: '340px', border: 'var(--border-w) solid var(--border)' }">
        <SessionRow :session="GALLERY_SESSIONS[4]" :now="GALLERY_SESSION_NOW" expanded />
        <SessionRow
          :session="GALLERY_SESSIONS[3]"
          :now="GALLERY_SESSION_NOW"
          separated
          expanded
          busy
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
           act on, and it names what was looked for. -->
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
             is sized to catch what a drag does at all. -->
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
              :sections="gitFolds"
              :branch-folders="gitFolders"
              @toggle="toggleGitSection"
              @toggle-folder="gitFolders = $event"
              @resize="resizeGitSection"
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
      <!-- The Branches caption with its three buttons, which is the one place
           in the app the remote can be reached from — and the states are the
           branch the repository is *on*, since that is what the two verbs are
           about. The check beside them is about the repository and is in every
           frame, including the ones where both verbs are gone. What to check:
           that three buttons do not crowd the count out of a caption 152
           pixels wide, that a refused one is legible rather than invisible,
           and that its reason opens on hover from the wrapper around it rather
           than from the disabled control itself. -->
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
        <!-- A run going: both verbs refused by the same verdict that mutes the
             rows, and both say so in `gitActions.js`'s own sentence. The check
             is live beside them, which is that rule drawn rather than merely
             stated: it writes remote-tracking refs and touches neither the
             tree nor the index, so a batch mid-merge has nothing to lose by
             it. -->
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
        <!-- A detached HEAD draws neither verb: there is no branch for an
             upstream to be about, and two dead controls say less than the
             caption does without them. The check stays — asking the remote
             what it has is a question about the repository, and a detached
             HEAD has not stopped it being one. -->
        <div :style="{ display: 'flex', width: '252px', height: '260px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false" :style="{ flex: 1, minWidth: 0 }">
            <GitPanel
              :repos="REPOS"
              selected="/Users/you/dev/smetana"
              :tree="{ branch: null, detached: 'a1b2c3d', changes: CHANGES }"
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
             glance. -->
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
      </div>
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <RepoList :repos="REPOS" selected="/Users/you/dev/smetana" />
        </div>
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <ChangeList :changes="CHANGES" selected="src/stores/vcs.js" />
        </div>
        <!-- The commit box in its four states, at the panel's own width. Live
             first: type into it and the button comes alive with the count of
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
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList :branches="BRANCHES" />
        </div>
        <!-- The branch the repository is on, lifted to the top out of the order
             recency put it in and out of the folder its name puts it in: it is
             last here and in a `feature/` heading that is folded, and it is
             still the first row, drawing its whole name with the hairline under
             it. What to check is that the rule reads as a separator and not as
             a row of its own, and that the row below it is not pushed a pixel
             down by it. -->
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
             be in: behind (orange, `↓3`), ahead (`↑2` and no colour), both at
             once, level with the remote, and a branch nobody has pushed, which
             has no record and draws nothing at all. What to check is that the
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
        <!-- A run going over the marks: the rows are muted and the names give
             the colour up with them, since one name in orange over a panel
             nobody may press would be saying a press was possible. The counts
             keep their own token — they are a fact about the remote and not an
             offer. -->
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
             checkable here too. -->
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
        <!-- The fourth of the panel's empty sentences, which no `GitPanel`
             frame can reach: a folder git can see nothing in has no branch to
             list, and the section is gated on there being a repository, so this
             is the only place it can be looked at. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <BranchList :branches="[]" />
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
           not a panel. Four frames, because all four states are reachable in a
           second and none of them can be looked at any other way: a merge, a
           rebase — the branches swap sides in the sentence, and getting that
           backwards would send an agent the wrong way round — an abort in
           flight, and an abort git refused.

           The thing to check in every one of them: there is no close button in
           the corner and no third way out. That is the whole design, since a
           conflicted tree behind a closed dialog is a state the panel promises
           to show and cannot draw. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <div :style="{ position: 'relative', width: '560px', height: '420px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <ConflictModal v-bind="CONFLICT" :open="true" @resolve="() => {}" @abort="() => {}" />
        </div>
        <div :style="{ position: 'relative', width: '560px', height: '420px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <ConflictModal
            v-bind="{ ...CONFLICT, op: 'rebase' }"
            :open="true"
            @resolve="() => {}"
            @abort="() => {}"
          />
        </div>
        <div :style="{ position: 'relative', width: '560px', height: '420px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <ConflictModal v-bind="CONFLICT" :open="true" busy @resolve="() => {}" @abort="() => {}" />
        </div>
        <!-- git refusing the abort itself, drawn inside the dialog: there is
             no dismiss, so a message anywhere else is one nobody can see. -->
        <div :style="{ position: 'relative', width: '560px', height: '460px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <ConflictModal
            v-bind="CONFLICT"
            :open="true"
            :error="{ kind: 'git', message: 'fatal: There is no merge to abort (MERGE_HEAD missing).' }"
            @resolve="() => {}"
            @abort="() => {}"
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
        <!-- A row the list names and cannot pick, drawn as the agent picker in
             the settings window draws it: muted, a note beside it, the same
             height as the row above so the list keeps its rhythm, and
             `not-allowed` under the pointer. The arrows step straight over it
             and Enter takes Claude Code from either direction. -->
        <div :style="{ width: '220px' }">
          <Dropdown
            v-model="pickedAgent"
            :options="[
              { value: 'claude', label: 'Claude Code' },
              { value: 'codex', label: 'Codex', disabled: true, note: 'Not supported yet' }
            ]"
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
          <!-- A branch three of the project's repositories carry and one does
               not. The hint counts them and the row names them: "will be created"
               on its own would say the branch does not exist, which is the very
               thing that sent a run out to cut a `develop` that already had its
               own history in four repositories. -->
          <BranchSelect
            v-model="partialBranch"
            :branches="[
              { name: 'develop', missing_in: [] },
              { name: 'main', missing_in: [] },
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
        <RunBar :run="runFixture({ kind: 'stopped', reason: { kind: 'queue_empty' } })" />
        <RunBar :run="runFixture({ kind: 'stopped', reason: { kind: 'no_progress' } })" />
        <RunBar :run="runFixture({ kind: 'stopped', reason: { kind: 'crashed', attempts: 5 } })" />
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
        <div :style="{ width: '380px' }">
          <AgentSettings
            :agent="galleryAgent"
            :agent-language="galleryAgentLanguage"
            :task-language="galleryTaskLanguage"
            :commit-language="galleryCommitLanguage"
            :report-language="galleryReportLanguage"
            :usage="galleryAgentUsage"
            @update:agent="galleryAgent = $event"
            @update:agent-language="galleryAgentLanguage = $event"
            @update:task-language="galleryTaskLanguage = $event"
            @update:commit-language="galleryCommitLanguage = $event"
            @update:report-language="galleryReportLanguage = $event"
          />
        </div>
        <!-- The same tab with Show run report off on the General tab, which is
             the other state the Report language row has: the control is drawn
             and cannot be pressed, and the description names the reason and the
             tab the switch is on. The chosen language is still handed in and
             still stands, which is what says the switch shuts the row rather
             than the setting. -->
        <div :style="{ width: '380px' }">
          <AgentSettings
            :agent="galleryAgent"
            :agent-language="galleryAgentLanguage"
            :task-language="galleryTaskLanguage"
            :commit-language="galleryCommitLanguage"
            :report-language="galleryReportLanguage"
            :show-report="false"
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
        <div :style="{ width: '380px' }">
          <AgentSettings agent="codex" :usage="galleryAgentUsageUnsupported" />
        </div>
        <div :style="{ width: '380px' }">
          <AgentSettings :usage="galleryAgentUsageUnreadable" />
        </div>
        <div :style="{ width: '380px' }">
          <AgentSettings :usage="galleryAgentUsageHalf" />
        </div>
        <div :style="{ width: '380px' }">
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
        </div>
        <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }">
          <Toast tone="warning" title="claude-1 needs you" description="bd-a1b2 · worktree name collision · 4m" />
          <Toast tone="error" title="claude-2 failed" description="exit 101 in wt/bd-3c9d" />
          <Toast tone="success" title="bd-12cd done" description="+41 −1 · 2h 14m" />
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
      <!-- What a thumbnail in the new-task dialog's images strip opens. Framed
           the way the dialogs above it are, and for the same reason: the viewer
           covers the nearest positioned ancestor, so the frame is what stands in
           for the modal it sits over in the app. A picture larger than the frame
           is the state worth looking at — fitted whole, nothing cropped off it,
           and no scrollbar anywhere. -->
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
