<script setup>
/* Desktop app shell — three-column agent control room: scope bar, tab bar,
   kanban, task inspector and live log.

   The core moment this screen is built for: you come back after two hours and
   read, in three seconds, what finished, what stalled, and what is waiting for
   you. Hence the loud budget — exactly one card and one callout shout here. */
import { computed, onMounted, ref, watch, watchEffect } from 'vue'
import ScopeIndicator from '../components/shell/ScopeIndicator.vue'
import Panel from '../components/shell/Panel.vue'
import TabBar from '../components/shell/TabBar.vue'
import FileTree from '../components/files/FileTree.vue'
import KanbanBoard from '../components/kanban/KanbanBoard.vue'
import StatusBadge from '../components/status/StatusBadge.vue'
import Button from '../components/core/Button.vue'
import Input from '../components/core/Input.vue'
import Select from '../components/core/Select.vue'
import NewTaskModal from '../components/kanban/NewTaskModal.vue'
import EmptyState from '../components/core/EmptyState.vue'
import Toast from '../components/overlays/Toast.vue'
import LogView from '../components/agent/LogView.vue'
import {
  boardColumns,
  closeIssue,
  createIssue,
  initTracker,
  issueById,
  reopenIssue,
  toUiStatus,
  trackerState,
  updateIssue
} from '../stores/tracker.js'
import { settings } from '../stores/settings.js'
import { agents, inspector, logLines, scope, tabs, tree } from './desktopAppData.js'

const props = defineProps({
  theme: { type: String, default: 'dark' },
  density: { type: String, default: 'comfortable' }
})

// Both switches live on the document root: every token is defined against them.
watchEffect(() => {
  const el = document.documentElement
  el.setAttribute('data-theme', props.theme)
  el.setAttribute('data-density', props.density)
})

/* Всё, что переживает перезапуск, живёт в настройках: панели — в layout,
   выбор внутри проекта — в project. Локальные ref остались только у того,
   что относится к текущему моменту: лог, модалка, черновик заголовка. */
const layout = settings.layout
const project = settings.project

/* FileTree ждёт карту «путь → открыт», а на диске лежит список раскрытых
   каталогов: в файле, который читают глазами, список честнее карты из
   одних true. */
const expanded = computed(() => Object.fromEntries(project.expanded.map((path) => [path, true])))

/* The sidebar holds three views of the same worktree, one at a time: its files,
   its git state, the agents working in it. */
const SIDE_TABS = [
  { id: 'files', label: 'Files' },
  { id: 'git', label: 'Git' },
  { id: 'agents', label: 'Agents' }
]
const hoveredSideTab = ref(null)
onMounted(initTracker)
const follow = ref(true)
const streamState = ref('streaming')
const logQuery = ref('')

/* bd gives a new task the one status it has for them — open, which the board
   calls ready. So that column, and only it, carries the "+": a plus over any
   other column would promise a placement the tracker cannot make. */
const ADD_TO = 'ready'
const newTaskOpen = ref(false)
const creating = ref(false)

const selectedIssue = computed(() => (project.selectedTask ? issueById(project.selectedTask) : null))

const submitNewTask = async (issue) => {
  creating.value = true
  try {
    const created = await createIssue(issue)
    newTaskOpen.value = false
    project.selectedTask = created.id
  } catch {
    // сообщение уже лежит в trackerState.lastError
  } finally {
    creating.value = false
  }
}

/* Renaming must not fire a ~2s tracker write per keystroke. Keep a local
   draft while the field has focus and commit it only on Enter or blur, and
   only when it actually differs from what is stored. While the field is not
   being edited, a title changed elsewhere (watcher delta, another write)
   still has to show up here — that's the second watcher below. */
const titleDraft = ref('')
const titleEditing = ref(false)

watch(
  () => project.selectedTask,
  () => {
    titleEditing.value = false
    titleDraft.value = selectedIssue.value?.title ?? ''
  }
)

watch(
  () => selectedIssue.value?.title,
  (nextTitle) => {
    if (!titleEditing.value) titleDraft.value = nextTitle ?? ''
  }
)

/* Пока приложение было закрыто, задачу могли закрыть и убрать из трекера.
   Восстанавливать выбор, которого больше нет, нельзя: инспектор показал бы
   пустоту, а файл продолжал бы хранить мусор. Ждём готовности трекера —
   до неё "не нашлось" ничего не значит. */
watch(
  () => [trackerState.ready, trackerState.issues.size],
  () => {
    if (trackerState.ready && project.selectedTask && !issueById(project.selectedTask)) {
      project.selectedTask = null
    }
  },
  { immediate: true }
)

const commitTitle = () => {
  const issue = selectedIssue.value
  if (!issue) return
  const next = titleDraft.value.trim()
  if (next && next !== issue.title) {
    updateIssue(issue.id, { title: next }).catch(() => {})
  } else {
    titleDraft.value = issue.title
  }
}

const blurTitle = () => {
  titleEditing.value = false
  commitTitle()
}

const setSelectedStatus = (status) => updateIssue(project.selectedTask, { status }).catch(() => {})
const closeSelected = () => closeIssue(project.selectedTask).catch(() => {})
const reopenSelected = () => reopenIssue(project.selectedTask).catch(() => {})

/* bd's own status names (open/in_progress/…) are identifiers, not prose —
   the picker still has to send them, but it must read as a sentence-case
   phrase, not a slug. toUiStatus gives the design system's name for the
   reserved ones; custom bd statuses pass through unchanged and get the same
   treatment so they stay readable too. */
const statusLabel = (name) => {
  const words = toUiStatus(name).replace(/[-_]+/g, ' ')
  return words.charAt(0).toUpperCase() + words.slice(1)
}
const statusOptions = computed(() =>
  trackerState.columns.map((c) => ({ value: c.name, label: statusLabel(c.name) }))
)

/* What the tracker's health means where the board would be. The generic
   "No board yet — connect a tracker" is wrong for a folder without .beads:
   there is nothing to connect to and creating a task there fails. Each state
   says what it is and what to do about it, and all of them stay quiet — this
   is information, not an emergency, and the loud budget belongs to the card
   that is waiting on you. The diagnostic text from Rust goes to the console,
   not here. */
const HEALTH_NOTICE = {
  'not-a-beads-repo': {
    icon: 'folder-git-2',
    title: 'No tracker here',
    description:
      'No .beads directory in this folder or any folder above it. Open the app from a project that bd tracks.'
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

/* Only when there is nothing else to show: a failing bd is no reason to hide
   the tasks that were already read. */
const healthNotice = computed(() => {
  if (trackerState.health.state === 'ok') return null
  if (boardColumns.value.some((column) => column.tasks.length)) return null
  return HEALTH_NOTICE[trackerState.health.state] ?? HEALTH_NOTICE.error
})

const toggleDir = (path) => {
  const at = project.expanded.indexOf(path)
  if (at === -1) project.expanded.push(path)
  else project.expanded.splice(at, 1)
}
const toggleStream = () => {
  streamState.value = streamState.value === 'streaming' ? 'paused' : 'streaming'
}

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
   keeps the panel's name and the button that brings it back. */
const leftStyle = computed(() => ({
  flex: '0 0 auto',
  width: layout.leftCollapsed ? '32px' : '252px',
  display: 'flex',
  minWidth: 0
}))
/* Panel scrolls its slot as one block; the worktree line and the tab row have
   to stay put, so only what is under them scrolls. */
const sidebarStyle = {
  display: 'flex',
  flexDirection: 'column',
  height: '100%',
  minHeight: 0
}
/* 10px uppercase mono: a label, not a sentence */
const microHeader = (topRule = false) => ({
  display: 'flex',
  alignItems: 'center',
  height: 'var(--tab-h)',
  flex: '0 0 auto',
  padding: '0 var(--space-5)',
  borderTop: topRule ? 'var(--border-w) solid var(--border-subtle)' : undefined,
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
})
/* The sidebar's own tab row: same height and top accent as the document tabs,
   but micro type, because these are section names and not open files. */
const sideTabBar = {
  display: 'flex',
  alignItems: 'stretch',
  height: 'var(--tab-h)',
  flex: '0 0 auto',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const sideTabStyle = (tab, last) => {
  const active = project.sideTab === tab.id
  return {
    flex: 1,
    minWidth: 0,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
    letterSpacing: 'var(--tracking-caps)',
    textTransform: 'uppercase',
    color: active ? 'var(--text-primary)' : 'var(--text-muted)',
    background: active
      ? 'var(--surface-raised)'
      : hoveredSideTab.value === tab.id
        ? 'var(--surface-hover)'
        : 'transparent',
    boxShadow: active ? 'inset 0 2px 0 0 var(--text-primary)' : 'none',
    borderRight: last ? undefined : 'var(--border-w) solid var(--border-subtle)',
    cursor: 'default',
    transition: 'var(--transition-control)'
  }
}
const agentRow = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}
/* needs-you keeps its triangle silhouette here too — colour is never alone */
const needsYouMark = {
  width: 0,
  height: 0,
  borderLeft: '5px solid transparent',
  borderRight: '5px solid transparent',
  borderBottom: '8px solid var(--attn-loud)'
}
const runningMark = { width: '8px', height: '8px', borderRadius: '50%', background: 'var(--attn-live)' }

const centerStyle = { flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }

/* Collapsed, the column is the same 32px rail AppShell reserves for one. */
const rightStyle = computed(() => ({
  flex: '0 0 auto',
  width: layout.rightCollapsed ? '32px' : '340px',
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
const calloutStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  padding: 'var(--card-pad)',
  background: 'var(--status-needs-you-bg)',
  border: 'var(--border-w) solid var(--attn-loud)',
  borderRadius: 'var(--radius-3)'
}
const calloutLabel = {
  font: 'var(--weight-semibold) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--status-needs-you-fg)'
}
const blocksLine = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)',
  color: 'var(--status-blocked-fg)'
}
/* the hatch is the dependency signature, reused at swatch size */
const hatchSwatch = {
  width: '16px',
  height: '8px',
  borderRadius: 'var(--radius-1)',
  backgroundImage: 'repeating-linear-gradient(135deg,var(--hatch-blocked) 0 1.5px,transparent 1.5px 4px)'
}

const questionParts = computed(() => inspector.question.split(inspector.collidesWith))
</script>

<template>
  <div :style="rootStyle">
    <ScopeIndicator v-bind="scope" />

    <div :style="bodyStyle">
      <!-- left: worktree files and the agents working in it -->
      <div :style="leftStyle">
        <Panel
          title="Projects"
          side="left"
          :collapsed="layout.leftCollapsed"
          :style="{ flex: 1, minWidth: 0 }"
          @toggle="layout.leftCollapsed = !layout.leftCollapsed"
        >
          <div :style="sidebarStyle">
            <div :style="microHeader()">{{ scope.worktree }}</div>
            <div role="tablist" :style="sideTabBar">
              <div
                v-for="(t, i) in SIDE_TABS"
                :key="t.id"
                role="tab"
                :aria-selected="project.sideTab === t.id"
                :tabindex="project.sideTab === t.id ? 0 : -1"
                :style="sideTabStyle(t, i === SIDE_TABS.length - 1)"
                @click="project.sideTab = t.id"
                @mouseenter="hoveredSideTab = t.id"
                @mouseleave="hoveredSideTab = null"
              >
                {{ t.label }}
              </div>
            </div>
            <div :style="{ flex: 1, minHeight: 0, overflow: 'auto' }">
              <FileTree
                v-if="project.sideTab === 'files'"
                :nodes="tree"
                :expanded="expanded"
                :selected-path="project.selectedPath ?? undefined"
                @toggle="toggleDir"
                @select="project.selectedPath = $event"
              />
              <EmptyState
                v-else-if="project.sideTab === 'git'"
                compact
                icon="git-branch"
                title="Git is not connected"
                description="Changes and branch state will live here. Nothing in the app reads git yet."
              />
              <template v-else>
                <div v-for="a in agents" :key="a.name" :style="agentRow">
                  <span :style="a.state === 'needs-you' ? needsYouMark : runningMark" />
                  <span>{{ a.name }}</span>
                  <span :style="{ color: 'var(--text-muted)' }">{{ a.task }}</span>
                  <span :style="{ flex: 1 }" />
                  <span :style="{ color: a.state === 'needs-you' ? 'var(--attn-loud)' : 'var(--text-muted)' }">
                    {{ a.elapsed }}
                  </span>
                </div>
              </template>
            </div>
          </div>
        </Panel>
      </div>

      <!-- centre: tabs over the board -->
      <div :style="centerStyle">
        <TabBar :tabs="tabs" :active-id="project.activeTab" @select="project.activeTab = $event" />
        <NewTaskModal
          :open="newTaskOpen"
          :busy="creating"
          :status="ADD_TO"
          @close="newTaskOpen = false"
          @submit="submitNewTask"
        />
        <EmptyState v-if="healthNotice" v-bind="healthNotice" />
        <KanbanBoard
          v-else
          :columns="boardColumns"
          :selected-id="project.selectedTask"
          :add-to="ADD_TO"
          @select="project.selectedTask = $event"
          @add="newTaskOpen = true"
        />
      </div>

      <!-- right: the task that is waiting on you, and its live output -->
      <div :style="rightStyle">
        <Panel
          title="Task &amp; output"
          side="right"
          :collapsed="layout.rightCollapsed"
          :style="{ flex: 1, minWidth: 0 }"
          @toggle="layout.rightCollapsed = !layout.rightCollapsed"
        >
          <div :style="inspectorBody">
            <template v-if="selectedIssue">
              <div :style="{ display: 'flex', alignItems: 'center', gap: 'var(--space-4)' }">
                <span :style="{ font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)', color: 'var(--text-muted)' }">
                  {{ selectedIssue.id }}
                </span>
                <StatusBadge :status="toUiStatus(selectedIssue.status)" size="sm" />
              </div>
              <Input
                v-model="titleDraft"
                @focusin="titleEditing = true"
                @focusout="blurTitle"
                @keydown.enter="commitTitle"
              />
              <Select :model-value="selectedIssue.status" :options="statusOptions" @update:model-value="setSelectedStatus" />
              <div :style="{ display: 'flex', gap: 'var(--space-4)' }">
                <Button v-if="selectedIssue.status !== 'closed'" variant="secondary" size="sm" @click="closeSelected">
                  Close
                </Button>
                <Button v-else variant="secondary" size="sm" @click="reopenSelected">Reopen</Button>
              </div>
            </template>

            <template v-else>
              <div :style="{ display: 'flex', alignItems: 'center', gap: 'var(--space-4)' }">
                <span :style="{ font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)', color: 'var(--text-muted)' }">
                  {{ inspector.id }}
                </span>
                <StatusBadge :status="inspector.status" size="sm" />
              </div>

              <div :style="{ fontSize: 'var(--text-md)', lineHeight: 'var(--leading-snug)', textWrap: 'pretty' }">
                {{ inspector.title }}
              </div>

              <div :style="calloutStyle">
                <div :style="calloutLabel">waiting on you · {{ inspector.waitingFor }}</div>
                <div :style="{ fontSize: 'var(--text-sm)' }">
                  {{ questionParts[0]
                  }}<span :style="{ fontFamily: 'var(--font-mono)' }">{{ inspector.collidesWith }}</span
                  >{{ questionParts[1] }}
                </div>
                <div :style="{ display: 'flex', gap: 'var(--space-4)' }">
                  <Button variant="primary" size="sm">Overwrite</Button>
                  <Button variant="secondary" size="sm">Pick new name</Button>
                </div>
              </div>

              <div :style="blocksLine">
                <span :style="hatchSwatch" />
                blocks {{ inspector.blocksDownstream }} downstream tasks
              </div>
            </template>

            <LogView
              :lines="logLines"
              :stream-state="streamState"
              :follow="follow"
              v-model:query="logQuery"
              :height="260"
              @toggle-follow="follow = !follow"
              @toggle-stream="toggleStream"
            />
          </div>
        </Panel>
      </div>
    </div>

    <div v-if="trackerState.lastError" :style="{ position: 'fixed', right: 'var(--space-6)', bottom: 'var(--space-6)', zIndex: 'var(--z-toast)' }">
      <Toast tone="error" :title="trackerState.lastError.title" :description="trackerState.lastError.description"
             @close="trackerState.lastError = null" />
    </div>
  </div>
</template>
