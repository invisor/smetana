<script setup>
/* Dev harness: renders every component in the library once, so a broken port
   shows up here rather than in the product. Not part of the shipped app —
   reachable at ?view=gallery. */
import { computed, ref, watchEffect } from 'vue'
import { orderColumns } from '../components/kanban/columnOrder.js'
import { branchMenuItems } from '../components/git/branchMenu.js'
import { taskMenuItems } from '../components/kanban/taskMenu.js'
import { NEW_TAB_ITEMS } from '../components/shell/newTabMenu.js'
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
  ChatMessage,
  Checkbox,
  ClaimedTasks,
  CodeBlock,
  ColumnHeader,
  ConflictModal,
  ContextMenu,
  DependencyMark,
  DiffView,
  Dropdown,
  DependencySpine,
  DraftInspector,
  EditorSettings,
  EmptyState,
  FileEditor,
  FileTree,
  GeneralSettings,
  GitPanel,
  Icon,
  IconButton,
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
  ProjectList,
  PromoteColumnModal,
  RepoList,
  ScopeIndicator,
  SectionHeader,
  ReportView,
  Select,
  RunBar,
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
  TerminalView,
  Textarea,
  TypeBadge,
  Toast,
  ToolCall,
  Tooltip
} from '../components/index.js'
import { gitActions } from '../components/git/gitActions.js'
import { runNotification, storageNotification } from '../components/notifications/notifications.js'
import { logLines } from './desktopAppData.js'
import { MOCK_TREE } from '../stores/mockBackend.js'
/* The app's one link-opening path, bound to what the inspector raises. In
   a browser it is a new tab; in the app it is the person's own browser. */
import { openExternal } from '../stores/app.js'
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
code{font-family:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace}
.eyebrow{font-family:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:10px;letter-spacing:.07em;text-transform:uppercase;color:var(--text-muted);margin:0 0 8px}
h1{font-size:22px;font-weight:600;letter-spacing:-.006em;line-height:1.2;margin:0}
.meta{font-family:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:12px;color:var(--text-secondary);word-break:break-all;margin:8px 0 0}
.strip{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:8px}
.cell{background:var(--surface-raised);border:1px solid var(--border-subtle);border-radius:4px;
box-shadow:var(--shadow-raised);padding:10px;display:flex;flex-direction:column;gap:4px}
.cell-label{font-family:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:10px;letter-spacing:.07em;text-transform:uppercase;color:var(--text-muted)}
.cell-n{font-family:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:22px;font-weight:500;line-height:1.2;color:var(--text-primary)}
.cell-done{color:var(--status-done-fg)}
.cell-loud{color:var(--attn-loud)}
.cell-none{color:var(--text-muted)}
.sec{display:flex;align-items:baseline;gap:8px;border-bottom:1px solid var(--border);
padding-bottom:6px;margin:0 0 -8px;
font-family:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:10px;letter-spacing:.07em;text-transform:uppercase;font-weight:400;color:var(--text-secondary)}
.sec-n{color:var(--text-muted);letter-spacing:0}
.list{display:flex;flex-direction:column;gap:8px}
.card{background:var(--surface-raised);border:1px solid var(--border-subtle);border-radius:4px;
box-shadow:var(--shadow-raised);padding:16px;display:flex;flex-direction:column;gap:8px}
.card-parked{border-color:var(--status-needs-you-border)}
.card-batch{background:var(--surface);box-shadow:none}
.head{display:flex;align-items:center;gap:8px;flex-wrap:wrap}
.chip{font-family:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:12px;font-weight:500;background:var(--surface-sunken);border:1px solid var(--border-subtle);
border-radius:3px;padding:1px 6px;white-space:nowrap}
.badge{font-family:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:11px;border-radius:3px;padding:1px 6px;white-space:nowrap;border:1px solid}
.badge-done{background:var(--status-done-bg);color:var(--status-done-fg);border-color:var(--status-done-border)}
.badge-parked{background:var(--status-needs-you-bg);color:var(--status-needs-you-fg);
border-color:var(--status-needs-you-border)}
.batch-label{font-family:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:10px;letter-spacing:.07em;text-transform:uppercase;color:var(--text-secondary)}
.right{margin-left:auto;
font-family:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:11px;color:var(--text-muted)}
h3{margin:0;font-size:15px;font-weight:600;line-height:1.35}
.body{margin:0;color:var(--text-secondary)}
.body code{font-size:12px;color:var(--text-primary)}
.unknown{margin:0;color:var(--text-muted)}
.notice{background:var(--surface);border:1px solid var(--border-subtle);border-radius:4px;
padding:16px;color:var(--text-muted);margin:0}
.total{border-top:1px solid var(--border-strong);padding-top:12px;display:flex;align-items:baseline;gap:8px}
.total-label{font-family:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
font-size:10px;letter-spacing:.07em;text-transform:uppercase;color:var(--text-secondary)}
.total-n{margin-left:auto;
font-family:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,Consolas,"DejaVu Sans Mono",monospace;
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
<p class="body">Nothing odd, though <code>bd list</code> was slow to answer.</p></div>
<div class="card card-batch"><div class="head"><span class="batch-label">batch 2</span>
<span class="right">28m</span></div>
<p class="unknown">This batch left no account of itself.</p></div>
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
const tabs = computed(() => [
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
   type nobody chose. One has both fields set, the other neither. */
const FULL_DRAFT = {
  text:
    'The log view drops lines once it is past about ten thousand of them, and nothing says so — it just stops scrolling back. It should either keep them or say plainly that it stopped.',
  issueType: 'bug',
  priority: 1
}
const AUTO_DRAFT = {
  text: 'Vendor the latin subset of IBM Plex Mono so an offline build has a face to set identifiers in.',
  issueType: null,
  priority: null
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
  bdStatus: 'closed',
  runnable: true,
  runBlockedReason: '',
  busy: false
})
const BUSY_CARD_MENU = taskMenuItems({
  bdStatus: 'open',
  runnable: true,
  runBlockedReason: 'a run over task smetana-hth is already going',
  busy: true
})
/* The parked card, which is the only shape of this menu with five rows: the
   answer row on top and the play under it dead, since `runnableTask` in
   DesktopApp refuses a parked task for the same reason the Ready dialog asks
   about one. */
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
/* Local like the two above, and for the same reason: in the app this value
   comes from the main window and goes back to it as an event, and neither end
   exists here. Bound rather than left to its default so the switch actually
   moves when it is pressed — a control that does not respond is the one thing
   this page cannot be used to check. */
const galleryGitAutoFetch = ref(true)
const galleryAgent = ref('claude')
/* The Agents tab's two language pickers. Not both on English: the longest label
   either list holds is the one worth looking at, and a tab showing "English"
   twice would never draw it. */
const galleryAgentLanguage = ref('ru')
const galleryTaskLanguage = ref('zh-Hans')
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
  storageNotification('/Users/you/Projects/smetana', 62 * 1024 * 1024 + 700 * 1024, 50)
]

const sectionStyle = {
  display: 'flex', flexDirection: 'column', gap: 'var(--space-5)',
  padding: 'var(--space-6)', borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const headStyle = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)', textTransform: 'uppercase', color: 'var(--text-muted)'
}
const rowStyle = { display: 'flex', alignItems: 'center', gap: 'var(--space-5)', flexWrap: 'wrap' }

/* `PointerMenu` draws nothing at all until a secondary click gives it a point
   to hang off, so unlike every other component here it needs somewhere to be
   clicked. The box is the frame; the menu is what opens over it, at the
   pointer, and both branch cases are here because the refused one is where the
   caption above the greyed rows can be read. */
const branchMenu = ref(null)
const refusedBranchMenu = ref(null)
const BRANCH_MENU = branchMenuItems()
const REFUSED_BRANCH_MENU = branchMenuItems({ allowed: false })
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
          />
        </div>
        <div :style="{ width: '212px' }">
          <TaskCard id="bd-12cd" title="Bump tauri to 2.1" status="done" bd-status="closed" type="chore" />
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
          @select="() => {}"
          @add="() => {}"
          @run="() => {}"
          @task-action="() => {}"
          @promote="() => {}"
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
          @select="() => {}"
          @add="() => {}"
          @run="() => {}"
          @task-action="() => {}"
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
      <AttachmentStrip :items="ATTACHMENTS" @remove="() => {}" />
      <!-- Past two rows the strip scrolls instead of growing: nothing bounds
           how many images are attached, and the dialog has no scrolling of its
           own to absorb them. -->
      <div :style="{ width: '400px' }">
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
           where "How many at once" is inactive — a state behind two clicks is a
           state nobody checks, so it is on the page like every other one.

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
          <TaskInspector :issue="FULL_ISSUE" ui-status="running" @open="openExternal" />
        </div>
        <div :style="{ width: '320px' }">
          <TaskInspector :issue="SPARSE_ISSUE" ui-status="ready" @open="openExternal" />
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
           running", and at its bell for "1 notification". -->
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
      </div>
    </section>

    <section :style="sectionStyle">
      <div :style="headStyle">Shell</div>
      <TabBar :tabs="tabs" active-id="kanban">
        <!-- The row's second slot, inside the scrolling strip and right after
             the pinned tabs, which is where the app puts it: the control is
             about those tabs and has to stay beside them however many files are
             open. -->
        <template #afterPinned>
          <MenuButton icon="plus" label="New agent, terminal or task" :items="NEW_TAB_ITEMS" :width="180" />
        </template>
      </TabBar>
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
        </AppShell>
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
             arrives. -->
        <div :style="{ width: '252px', border: 'var(--border-w) solid var(--border)' }">
          <CommitBox
            v-model="commitDraft"
            :changes="CHANGES.length"
            branch="feat/worktree-rename"
            @commit="commitDraft = ''"
            @suggest="commitDraft = 'feat: add a commit box to the Git panel'"
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
             first time this panel is looked at. `fix` holds a folder of its own,
             so the indentation of a second level is checkable here too. -->
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
      <!-- ProjectList carries no header of its own — the surrounding Panel owns
           "Projects" and the "+" in its actions slot, so the demo wraps it the
           same way DesktopApp.vue does, to catch the pairing breaking too.

           Right-click the rows: every one of them opens the row's three actions
           at the pointer, and the two that are only meaningful for the project
           the window is pointed at are greyed with the reason in the row on
           every other. The setup item is the frames' second job here — it reads
           "Set up" where there is no configuration and "Set up again" where
           there is one, damaged or not — and the last frame is where the
           placement can be checked, since nothing else on this page opens a
           panel from a point rather than from a control. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <div :style="{ width: '252px', height: '220px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false">
            <template #actions>
              <IconButton icon="plus" label="Add project" size="sm" />
            </template>
            <ProjectList
              :projects="[
                { path: '/Users/you/dev/smetana', name: 'smetana', tracked: true },
                { path: '/Users/you/dev/beads-viewer', name: 'beads-viewer', tracked: true }
              ]"
              active-path="/Users/you/dev/smetana"
              can-add-agent
              needs-setup
            />
          </Panel>
        </div>
        <div :style="{ width: '252px', height: '220px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false">
            <template #actions>
              <IconButton icon="plus" label="Add project" size="sm" />
            </template>
            <!-- The one frame with a run configuration that reads. Nothing on
                 the row says so — a project that is set up has nothing to
                 report — so it is only visible in the right-click menu, where
                 this active row is the one that reads "Set up again". -->
            <ProjectList
              :projects="[
                { path: '/Users/you/dev/smetana', name: 'smetana', tracked: true },
                { path: '/Users/you/notes', name: 'notes', tracked: false }
              ]"
              active-path="/Users/you/notes"
              configured
            />
          </Panel>
        </div>
        <!-- Both marks on one row — a folder just added, with neither a tracker
             nor a run configuration. The two triangles are the same glyph in
             two colours, so this is the frame that shows whether position and
             pairing carry the difference on their own. -->
        <div :style="{ width: '252px', height: '220px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false">
            <template #actions>
              <IconButton icon="plus" label="Add project" size="sm" />
            </template>
            <ProjectList
              :projects="[{ path: '/Users/you/dev/scratch', name: 'scratch', tracked: false }]"
              active-path="/Users/you/dev/scratch"
              needs-setup
            />
          </Panel>
        </div>
        <!-- The damaged configuration, deliberately on an untracked folder so
             that its mark stands next to the tracker's. The two are the only
             pair on this row that both stand alone, so this is the frame that
             says whether the silhouettes carry the difference — a triangle and
             a page — or whether it was resting on hue after all. There is no
             gear beside the red one on purpose, and it must not read as a
             button that failed to render. -->
        <div :style="{ width: '252px', height: '220px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false">
            <template #actions>
              <IconButton icon="plus" label="Add project" size="sm" />
            </template>
            <ProjectList
              :projects="[{ path: '/Users/you/dev/holiday-curb', name: 'holiday-curb', tracked: false }]"
              active-path="/Users/you/dev/holiday-curb"
              can-add-agent
              config-broken
            />
          </Panel>
        </div>
        <div :style="{ width: '252px', height: '220px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false">
            <template #actions>
              <IconButton icon="plus" label="Add project" size="sm" />
            </template>
            <ProjectList :projects="[]" />
          </Panel>
        </div>
        <!-- A list past the fifth row, which is where it starts to scroll, and
             the frame the menu's placement is checked in. Right-click the last
             row a person can see: the panel is teleported to the body and fixed
             in window coordinates, so neither the list's own scroll container
             nor this frame may clip it, and it flips above the pointer when the
             window has no room below. Scroll the list under an open menu and it
             closes — the point it was opened at no longer names a row. -->
        <div :style="{ width: '252px', height: '220px', border: 'var(--border-w) solid var(--border)' }">
          <Panel title="Projects" side="left" :collapsible="false">
            <template #actions>
              <IconButton icon="plus" label="Add project" size="sm" />
            </template>
            <ProjectList
              :projects="[
                { path: '/Users/you/dev/smetana', name: 'smetana', tracked: true },
                { path: '/Users/you/dev/beads-viewer', name: 'beads-viewer', tracked: true },
                { path: '/Users/you/dev/holiday-curb', name: 'holiday-curb', tracked: true },
                { path: '/Users/you/dev/tracker-notes', name: 'tracker-notes', tracked: false },
                { path: '/Users/you/dev/scratch', name: 'scratch', tracked: true },
                { path: '/Users/you/dev/archive', name: 'archive', tracked: true }
              ]"
              active-path="/Users/you/dev/holiday-curb"
              can-add-agent
              configured
            />
          </Panel>
        </div>
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
      <!-- The four tabs of the settings window, side by side rather than behind
           a tab bar: this harness is for seeing every component at once, and a
           tab strip here would hide three of the four behind a click. The values
           are local refs — in the app they arrive from the main window and go
           back to it as events, and neither end exists here. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', flexWrap: 'wrap', alignItems: 'flex-start' }">
        <div :style="{ width: '380px' }">
          <GeneralSettings
            :theme="galleryTheme"
            :ui-font-size="galleryUiFont"
            :git-auto-fetch="galleryGitAutoFetch"
            @update:theme="galleryTheme = $event"
            @update:ui-font-size="galleryUiFont = $event"
            @update:git-auto-fetch="galleryGitAutoFetch = $event"
          />
        </div>
        <div :style="{ width: '380px' }">
          <EditorSettings :font-size="galleryEditorFont" @update:font-size="galleryEditorFont = $event" />
        </div>
        <div :style="{ width: '380px' }">
          <AgentSettings
            :agent="galleryAgent"
            :agent-language="galleryAgentLanguage"
            :task-language="galleryTaskLanguage"
            @update:agent="galleryAgent = $event"
            @update:agent-language="galleryAgentLanguage = $event"
            @update:task-language="galleryTaskLanguage = $event"
          />
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
        <div :style="{ width: '380px' }">
          <AboutSettings version="0.1.0" />
        </div>
        <div :style="{ width: '380px' }">
          <SettingsRow label="A row on its own" description="Label, one line of explanation, and whatever control the setting needs.">
            <Switch :model-value="switched" label="" @update:model-value="switched = $event" />
          </SettingsRow>
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

             At the card's own width, not the component's default. The second
             one is the case that width exists for — the greyed Run row carries
             the whole of `scopeBusyReason`'s sentence, and at 200 it is
             ellipsised with no tooltip to recover it. Checking the fix means
             seeing it at the size the board actually draws. -->
        <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-4)', alignItems: 'flex-start' }">
          <MenuButton :items="CARD_MENU" :width="424" label="Actions for bd-a1b2" @select="() => {}" />
          <MenuButton :items="BUSY_CARD_MENU" :width="424" label="Actions for bd-77e0" @select="() => {}" />
          <MenuButton :items="PARKED_CARD_MENU" :width="424" label="Actions for bd-29j1" @select="() => {}" />
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
    </section>
  </div>
</template>
