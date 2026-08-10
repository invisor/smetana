<script setup>
/* Dev harness: renders every component in the library once, so a broken port
   shows up here rather than in the product. Not part of the shipped app —
   reachable at ?view=gallery. */
import { computed, ref, watchEffect } from 'vue'
import { orderColumns } from '../components/kanban/columnOrder.js'
import {
  AgentList,
  AppShell,
  Assignee,
  AttachmentStrip,
  BranchSelect,
  Button,
  ChatMessage,
  Checkbox,
  ClaimedTasks,
  CodeBlock,
  ColumnHeader,
  ContextMenu,
  DependencyMark,
  Dropdown,
  DependencySpine,
  DraftInspector,
  EmptyState,
  FileEditor,
  FileTree,
  Icon,
  IconButton,
  Input,
  KanbanBoard,
  LogView,
  Modal,
  NewTaskModal,
  Panel,
  ProjectList,
  PromoteColumnModal,
  Select,
  RunBar,
  RunModal,
  SetupProjectModal,
  Skeleton,
  StatusBadge,
  StatusDot,
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
import { logLines } from './desktopAppData.js'
import { MOCK_TREE } from '../stores/mockBackend.js'
import { terminalState } from '../stores/terminals.js'

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
const choice = ref('running')
/* The branch picker on its own: its three states are reached by clicking, and
   the dialog around it is not what needs looking at. */
const pickedBranch = ref('staging')
const branchIsNew = ref(false)
const checked = ref(true)
const switched = ref(true)

/* The gallery's tab row is its own: in the app it comes from a store, while
   here we need a fixed set showing all four kinds at once. */
const tabs = [
  { id: 'terminal', kind: 'pinned', label: 'Agent' },
  { id: 'kanban', kind: 'pinned', label: 'Kanban' },
  { id: 'tabs.rs', kind: 'file', label: 'tabs.rs', dirty: true },
  { id: 'agent.rs', kind: 'preview', label: 'agent.rs' },
  {
    id: 'logo.png',
    kind: 'file',
    label: 'logo.png',
    readOnly: true,
    readOnlyHint: 'Binary file — not shown.'
  }
]

/* AgentList reads rows and activeId as props, unlike TerminalView below,
   which reads the store directly — so a plain local fixture is enough here. */
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

/* Two issues in bd's own shape: one that has everything the inspector can
   draw, and one that has almost nothing. The second is the case worth looking
   at — a panel that reads as a form with blank rows is the defect this section
   exists to catch. */
const FULL_ISSUE = {
  id: 'smetana-29j.11',
  title: 'Show the tracker state on a non-empty board too',
  status: 'in_progress',
  description:
    'The health notice renders only while the columns are empty, so a version mismatch is invisible exactly when there are cards — a person is looking at stale data with nothing to say so.',
  priority: 1,
  issue_type: 'bug',
  owner: 'merazent@gmail.com',
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

/* Reserved statuses plus generated ones, to show both halves of the algorithm. */
const statuses = [
  'blocked', 'ready', 'running', 'needs-you', 'done', 'failed',
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
         see `runnableTask`. */
      { status: 'ready', tasks: [{ id: 'bd-a1b2', title: 'Rename worktree when the branch changes', status: 'ready', type: 'bug', runnable: true }] },
      { status: 'running', tasks: [{ id: 'bd-3c9d', title: 'Virtualise the log list above 10k lines', status: 'running', type: 'feature', assignee: { kind: 'agent', name: 'claude-1' }, spawnedFrom: 'bd-7f31', runnable: true }] },
      { status: 'needs-you', tasks: [{ id: 'bd-7f31', title: 'Approve the migration plan', status: 'needs-you', type: 'epic', needsResponse: true, runnable: true }] },
      { status: 'done', tasks: [{ id: 'bd-12cd', title: 'Bump tauri to 2.1', status: 'done', type: 'chore' }] },
      /* Where a run files what it found, and the one column that carries the
         whole-column press. Its cards are not runnable: a run takes only what
         is already open, which is exactly what that button is for. */
      { status: 'deferred', tasks: [
        { id: 'bd-5a10', title: 'Resizer promises arrow keys it does not have', status: 'deferred', type: 'bug', spawnedFrom: 'bd-a1b2' },
        { id: 'bd-5a11', title: 'Vendor the mono subset for offline builds', status: 'deferred', type: 'chore' }
      ] }
    ],
    boardOrder.value
  )
)

/* Every glyph a column header can draw: bd's built-in vocabulary, the two
   reserved statuses no bd column carries but a custom one might, the two custom
   statuses that have a glyph of their own — `ready_to_merge` in bd's own
   spelling, to show `normalizeStatus` doing its half — and one on the end with
   no glyph, for the generic tag. `running` appears twice, because the spinner
   is the count's business: it turns over work and stands still over an empty
   column. */
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

/* TerminalView has no props of its own to feed a fixture through, unlike
   FileTree above — it reads the active session straight from the store.
   Pointing terminalState at session 1 makes it attach on mount, which the
   mock backend answers with terminalFixture.js's captured output. */
terminalState.activeId = 1

const sectionStyle = {
  display: 'flex', flexDirection: 'column', gap: 'var(--space-5)',
  padding: 'var(--space-6)', borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const headStyle = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)', textTransform: 'uppercase', color: 'var(--text-muted)'
}
const rowStyle = { display: 'flex', alignItems: 'center', gap: 'var(--space-5)', flexWrap: 'wrap' }
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
        <!-- Both, side by side. Select is still the library's control for a
             short list in an ordinary form; Dropdown is what the product uses
             where the panel has to carry something of its own or escape a
             scrolling ancestor. -->
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
            type="feature"
            :blocked-by="2"
            :blocked-by-ids="['bd-91ac', 'bd-4d2e']"
            spawned-from="bd-7f31"
            :assignee="{ kind: 'agent', name: 'claude-1' }"
          />
        </div>
        <div :style="{ width: '212px' }">
          <TaskCard id="bd-12cd" title="Bump tauri to 2.1" status="done" type="chore" />
        </div>
        <!-- Runnable, and not runnable now: the play stays where it was, grey,
             and carries the reason. A button that vanished while a run was
             going would look like the board had lost the feature. -->
        <div :style="{ width: '212px' }">
          <TaskCard
            id="bd-77e0"
            title="Fold the settings debounce into the store"
            status="ready"
            type="task"
            runnable
            run-blocked-reason="a run is already going in this project"
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
          @run-task="() => {}"
          @promote="() => {}"
          @reorder="boardOrder = $event"
        />
      </div>
      <!-- The same board with a run already going in the project: every play on
           it, the column header's included, is inactive and carries the reason
           rather than disappearing. -->
      <div :style="{ display: 'flex', height: '300px', border: 'var(--border-w) solid var(--border)' }">
        <KanbanBoard
          :columns="boardColumns"
          add-to="ready"
          run-from="ready"
          run-blocked-reason="a run is already going in this project"
          :reorderable="false"
          @select="() => {}"
          @add="() => {}"
          @run="() => {}"
          @run-task="() => {}"
        />
      </div>
      <!-- Tall enough for the whole dialog, footer included: a frame that
           clips it turns the one harness that would catch a broken modal into
           a picture of the top half. It grew from 400px with the images row;
           adding another row means measuring it again. -->
      <div :style="{ position: 'relative', height: '520px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
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
           halves nobody sees together in the app. -->
      <div :style="{ position: 'relative', height: '520px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
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
            :branches="['main', 'staging', 'feature/runs-project-config']"
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
            :branches="['main', 'staging']"
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
            :branches="['main', 'staging']"
            default-branch="main"
            :remembered="{ mode: 'solo' }"
            @close="() => {}"
            @confirm="() => {}"
          />
        </div>
        <!-- The damaged configuration, with the parser's own message in it —
             the caret line and its leading spaces are the point, so this
             fixture keeps them. Every field below the notice is disabled and so
             is Run; "Set up again" is not, because it is the way out. -->
        <div :style="{ position: 'relative', width: '480px', height: '800px', border: 'var(--border-w) solid var(--border)', overflow: 'hidden' }">
          <RunModal
            :open="true"
            :count="12"
            :branches="['main', 'staging']"
            default-branch="staging"
            :config-error="BROKEN_CONFIG"
            @close="() => {}"
            @confirm="() => {}"
            @setup="() => {}"
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
            :branches="['main', 'staging']"
            default-branch="main"
            live-check-blocked="Nothing here can drive a browser: Playwright's MCP server is not in the agent's configuration and the Claude in Chrome extension was not found in a Chrome profile."
            @close="() => {}"
            @confirm="() => {}"
          />
        </div>
      </div>
      <!-- Two of them: the panel draws only the fields an issue has, so the
           sparse case is a different component to look at, not the same one
           with less in it. -->
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start' }">
        <div :style="{ width: '320px' }">
          <TaskInspector
            :issue="FULL_ISSUE"
            ui-status="running"
            @status="() => {}"
            @delete="() => {}"
            @ask-agent="() => {}"
          />
        </div>
        <div :style="{ width: '320px' }">
          <TaskInspector
            :issue="SPARSE_ISSUE"
            ui-status="ready"
            @status="() => {}"
            @delete="() => {}"
            @ask-agent="() => {}"
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
      <div :style="headStyle">Shell</div>
      <TabBar :tabs="tabs" active-id="kanban" />
      <div :style="{ height: '160px', border: 'var(--border-w) solid var(--border)' }">
        <AppShell :height="160" :left-width="180" :right-width="180">
          <template #left>
            <Panel title="Files" side="left">
              <FileTree
                :nodes="MOCK_TREE['']"
                :expanded="{}"
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
      <div :style="headStyle">Terminal</div>
      <!-- TerminalView takes no props — it reads the active session from
           terminals.js itself, so the fixture has to go in through the
           store, the way terminalFixture.js already does for the app's own
           terminal tab. Height is a token multiple, not a pixel number:
           the terminal fills whatever height it is given. -->
      <div
        :style="{
          display: 'flex',
          height: 'calc(var(--space-9) * 6)',
          border: 'var(--border-w) solid var(--border)'
        }"
      >
        <TerminalView />
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
      <div :style="headStyle">Projects</div>
      <!-- ProjectList carries no header of its own — the surrounding Panel owns
           "Projects" and the "+" in its actions slot, so the demo wraps it the
           same way DesktopApp.vue does, to catch the pairing breaking too. -->
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
            <ProjectList
              :projects="[
                { path: '/Users/you/dev/smetana', name: 'smetana', tracked: true },
                { path: '/Users/you/notes', name: 'notes', tracked: false }
              ]"
              active-path="/Users/you/notes"
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
              { value: 'ready', label: 'On its own' },
              { value: 'running', label: 'With a lead' },
              { value: 'done', label: 'Plain, one task' }
            ]"
          />
        </div>
        <div :style="{ width: '320px' }">
          <BranchSelect
            v-model="pickedBranch"
            v-model:create="branchIsNew"
            :branches="['main', 'staging', 'feature/runs-project-config', 'fix/tooltip-clipping', 'release/7']"
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
      <div :style="headStyle">Overlays and states</div>
      <div :style="{ display: 'flex', gap: 'var(--space-6)', alignItems: 'flex-start', flexWrap: 'wrap' }">
        <ContextMenu :items="menuItems" />
        <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }">
          <Toast tone="warning" title="claude-1 needs you" description="bd-a1b2 · worktree name collision · 4m" />
          <Toast tone="error" title="claude-2 failed" description="exit 101 in wt/bd-3c9d" />
          <Toast tone="success" title="bd-12cd done" description="+41 −1 · 2h 14m" />
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
