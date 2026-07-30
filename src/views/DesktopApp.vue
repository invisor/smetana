<script setup>
/* Desktop app shell — three-column agent control room: scope bar, tab bar,
   kanban, task inspector and live log.

   The core moment this screen is built for: you come back after two hours and
   read, in three seconds, what finished, what stalled, and what is waiting for
   you. Hence the loud budget — exactly one card and one callout shout here. */
import { computed, ref, watchEffect } from 'vue'
import ScopeIndicator from '../components/shell/ScopeIndicator.vue'
import TabBar from '../components/shell/TabBar.vue'
import FileTree from '../components/files/FileTree.vue'
import KanbanBoard from '../components/kanban/KanbanBoard.vue'
import StatusBadge from '../components/status/StatusBadge.vue'
import Button from '../components/core/Button.vue'
import LogView from '../components/agent/LogView.vue'
import {
  agents,
  columns,
  expanded as initialExpanded,
  inspector,
  logLines,
  scope,
  tabs,
  tree
} from './desktopAppData.js'

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

const expanded = ref({ ...initialExpanded })
const selectedPath = ref('src/tabs.rs')
const activeTab = ref('kanban')
/* No card starts selected: the inspector already shows bd-a1b2, and a selection
   border would take the loud amber edge away from the card that needs you. */
const selectedTask = ref(null)
const follow = ref(true)
const streamState = ref('streaming')
const logQuery = ref('')

const toggleDir = (path) => {
  expanded.value = { ...expanded.value, [path]: !expanded.value[path] }
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

const leftStyle = {
  flex: '0 0 auto',
  width: '252px',
  display: 'flex',
  flexDirection: 'column',
  background: 'var(--surface)',
  borderRight: 'var(--border-w) solid var(--border)',
  minWidth: 0
}
/* 10px uppercase mono: a label, not a sentence */
const microHeader = (topRule = false) => ({
  display: 'flex',
  alignItems: 'center',
  height: '30px',
  flex: '0 0 auto',
  padding: '0 var(--space-5)',
  borderTop: topRule ? 'var(--border-w) solid var(--border-subtle)' : undefined,
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
})
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

const rightStyle = {
  flex: '0 0 auto',
  width: '340px',
  display: 'flex',
  flexDirection: 'column',
  background: 'var(--surface)',
  borderLeft: 'var(--border-w) solid var(--border)',
  minWidth: 0
}
const inspectorBody = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-5)',
  padding: 'var(--panel-pad)',
  minWidth: 0,
  minHeight: 0,
  overflow: 'auto'
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
        <div :style="microHeader(true)">{{ scope.worktree }}</div>
        <div :style="{ flex: 1, minHeight: 0, overflow: 'auto' }">
          <FileTree
            :nodes="tree"
            :expanded="expanded"
            :selected-path="selectedPath"
            @toggle="toggleDir"
            @select="selectedPath = $event"
          />
          <div :style="microHeader()">Agents</div>
          <div v-for="a in agents" :key="a.name" :style="agentRow">
            <span :style="a.state === 'needs-you' ? needsYouMark : runningMark" />
            <span>{{ a.name }}</span>
            <span :style="{ color: 'var(--text-muted)' }">{{ a.task }}</span>
            <span :style="{ flex: 1 }" />
            <span :style="{ color: a.state === 'needs-you' ? 'var(--attn-loud)' : 'var(--text-muted)' }">
              {{ a.elapsed }}
            </span>
          </div>
        </div>
      </div>

      <!-- centre: tabs over the board -->
      <div :style="centerStyle">
        <TabBar :tabs="tabs" :active-id="activeTab" @select="activeTab = $event" />
        <KanbanBoard :columns="columns" :selected-id="selectedTask" @select="selectedTask = $event" />
      </div>

      <!-- right: the task that is waiting on you, and its live output -->
      <div :style="rightStyle">
        <div :style="microHeader()">Task &amp; output</div>
        <div :style="inspectorBody">
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
      </div>
    </div>
  </div>
</template>
