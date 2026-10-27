<script setup>
/* The working screens and the way between them: what the app is once the desktop is reachable.

   Four tabs - the board, the agents, the runs, the projects - and screens opened over them (a
   task, a conversation, the form for a new task). Where the person is, is one route
   (../nav.js, pure and tested); this component only draws it and moves it. A tapped
   notification moves it too: the target the core holds is turned into a route and spent.

   A tap on a board card opens the task sheet (./TaskSheet.vue) over the board, and this is where
   its rows are carried out: the rows come from ../taskSheet.js, the writes and the agent starts
   from ../taskActions.js, and the two confirmations are the desktop's own dialogs drawn over the
   screen. The sheet closes after an action that went through and stays open, with the tracker's
   sentence under the rows, after one that was refused. */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import DeleteTaskModal from '@src/components/kanban/DeleteTaskModal.vue'
import ReadyTaskModal from '@src/components/kanban/ReadyTaskModal.vue'
import { READY, needsReadyWarning, openQuestions } from '@src/components/kanban/parked.js'
import Button from '@src/components/core/Button.vue'
import { settings } from '@src/stores/settings.js'
import { boardColumns, issueById, trackerState } from '@src/stores/tracker.js'
import { runsState } from '@src/stores/runs.js'
import { conversationState } from '@src/stores/conversation.js'
import { basename } from '@src/paths.js'
import { back, canGoBack, current, open, routeForTarget, selectTab, START } from '../nav.js'
import { clearPushTarget, pushTarget } from '../pushTarget.js'
import { dismissRecommended } from '../bridge/index.js'
import { bridge } from '../bridge/status.js'
import { cardScope } from '@src/components/kanban/runScope.js'
import { sheetRows } from '../taskSheet.js'
import { refusal, remove, setStatus, startOn, toggleLock, writingIds } from '../taskActions.js'
import { openStore } from '../open.js'
import { refreshFromDesktop } from '../workspace.js'
import Shell from './Shell.vue'
import BoardScreen from './BoardScreen.vue'
import TaskScreen from './TaskScreen.vue'
import NewTaskScreen from './NewTaskScreen.vue'
import AgentsScreen from './AgentsScreen.vue'
import ConversationScreen from './ConversationScreen.vue'
import RunsScreen from './RunsScreen.vue'
import ProjectsScreen from './ProjectsScreen.vue'
import TaskSheet from './TaskSheet.vue'

const props = defineProps({
  banner: { type: Boolean, default: false },
  storeUrl: { type: String, default: '' }
})

const route = ref(START)
const screen = computed(() => current(route.value))

/* A notification was tapped. An unknown target leaves everything where it is; a known one is
   spent whether or not it moved us, so that the same tap is not answered twice. */
watch(
  () => pushTarget.value,
  (target) => {
    if (target === null) return
    const next = routeForTarget(target)
    if (next) route.value = next
    clearPushTarget()
  },
  { immediate: true }
)

/* There is no event for the desktop switching project, so the phone looks when the person comes
   back to it. */
const onVisible = () => {
  if (document.visibilityState === 'visible') {
    refreshFromDesktop().catch((err) => console.warn('[companion] could not refresh:', err))
  }
}
onMounted(() => document.addEventListener('visibilitychange', onVisible))
onBeforeUnmount(() => document.removeEventListener('visibilitychange', onVisible))

const TITLES = { board: 'Kanban', agents: 'Agents', runs: 'Runs', projects: 'Projects' }
const project = computed(() => (settings.activeProject ? basename(settings.activeProject) : ''))

const header = computed(() => {
  const s = screen.value
  switch (s.screen) {
    case 'task':
      return { title: 'Task', subtitle: s.id }
    case 'new-task':
      return { title: s.parent ? 'Follow-up task' : 'New task', subtitle: project.value }
    case 'run':
      return { title: 'Run', subtitle: s.scope.id }
    case 'conversation':
      return { title: s.driven ? 'Conversation' : 'Terminal session', subtitle: project.value }
    case 'runs':
      return { title: 'Runs', subtitle: route.value.project ? basename(route.value.project) : project.value }
    default:
      return { title: TITLES[s.screen] ?? 'Smetana', subtitle: s.screen === 'projects' ? '' : project.value }
  }
})

const go = (entry) => (route.value = open(route.value, entry))

/* The task sheet. `sheetId` is the card it was opened on; the card itself is read from the board
   on every draw, so a delta that moves it is seen, and a card that has gone closes the sheet. */
const sheetId = ref(null)
const sheetError = ref('')
const asking = ref(null)

const cardOf = (id) => {
  for (const column of boardColumns.value) {
    const found = column.tasks.find((task) => task.id === id)
    if (found) return found
  }
  return null
}
const sheetCard = computed(() => (sheetId.value ? cardOf(sheetId.value) : null))
watch(sheetCard, (card) => {
  if (sheetId.value && !card) closeSheet()
})

/* The run stores hold one project at a time, and the Runs screen may have pointed them at
   another one: the sheet answers about the project in front or about nothing. */
const runsHere = computed(() => runsState.project === settings.activeProject)
const rows = computed(() =>
  sheetCard.value
    ? sheetRows({
        card: sheetCard.value,
        issues: trackerState.issues,
        runs: runsHere.value ? runsState.runs : [],
        configState: runsHere.value ? runsState.config.state : '',
        status: bridge.status,
        busy: writingIds.has(sheetCard.value.id) || asking.value === sheetCard.value.id
      })
    : []
)

function openSheet(id) {
  sheetError.value = ''
  trackerState.lastError = null
  sheetId.value = id
}
function closeSheet() {
  sheetId.value = null
  sheetError.value = ''
}

/* After a write: close when it went through, say why when it was refused. A refusal with no
   sentence is the issue having gone, and there is nothing left to show. */
function settle(ok) {
  if (ok) return closeSheet()
  const why = refusal()
  if (why) sheetError.value = why
  else closeSheet()
}

/* The conversation an agent start lands in, the same place a new task's start goes. */
const intoConversation = (id) =>
  (route.value = { tab: 'agents', project: null, stack: [{ screen: 'conversation', id, driven: true }] })

async function startAgent(kind, id) {
  asking.value = id
  conversationState.lastError = null
  try {
    const session = await startOn(settings.activeProject, kind, id)
    if (session != null) {
      closeSheet()
      intoConversation(session)
    } else if (conversationState.lastError?.session === null && conversationState.lastError.text) {
      sheetError.value = conversationState.lastError.text
    } else closeSheet()
  } finally {
    asking.value = null
  }
}

const AGENT_KINDS = { 'ask-agent': 'editTask', resolve: 'resolveTask', fix: 'fixTask' }

/* Which confirmation is up: `{ kind: 'ready' | 'delete', id }`, or null. */
const confirming = ref(null)
const confirmed = computed(() => (confirming.value ? issueById(confirming.value.id) : null))
watch(confirmed, (issue) => {
  if (confirming.value && !issue) confirming.value = null
})

/* A write that was behind a confirmation has no sheet left to speak in, so the sheet comes back
   for a refusal. */
function afterConfirmed(id, ok) {
  if (ok) return
  const why = refusal()
  if (why && cardOf(id)) {
    openSheet(id)
    sheetError.value = why
  }
}

async function onAct({ kind, id, value }) {
  const issue = issueById(id)
  if (!issue) return closeSheet()
  sheetError.value = ''
  trackerState.lastError = null
  if (kind === 'view') {
    closeSheet()
    go({ screen: 'task', id })
  } else if (AGENT_KINDS[kind]) {
    await startAgent(AGENT_KINDS[kind], id)
  } else if (kind === 'follow-up') {
    closeSheet()
    go({ screen: 'new-task', parent: { id, title: issue.title } })
  } else if (kind === 'run') {
    closeSheet()
    go({ screen: 'run', scope: cardScope(trackerState.issues, id), title: issue.title })
  } else if (kind === 'lock') {
    settle(await toggleLock(id))
  } else if (kind === 'status') {
    if (needsReadyWarning(issue.status, value)) {
      closeSheet()
      confirming.value = { kind: 'ready', id }
    } else settle(await setStatus(id, value))
  } else if (kind === 'delete') {
    closeSheet()
    confirming.value = { kind: 'delete', id }
  }
}

async function confirmDelete() {
  const id = confirming.value?.id
  confirming.value = null
  if (id) afterConfirmed(id, await remove(id))
}
async function confirmReady() {
  const id = confirming.value?.id
  confirming.value = null
  if (id) afterConfirmed(id, await setStatus(id, READY))
}
async function resolveInstead() {
  const id = confirming.value?.id
  confirming.value = null
  if (id) await startAgent('resolveTask', id)
}

const modalLayer = { position: 'fixed', inset: 0, zIndex: 'var(--z-modal)' }

const bannerStyle = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 'var(--space-3)',
  padding: 'var(--space-3) var(--space-4)',
  background: 'var(--surface-raised)',
  borderBottom: 'var(--border-w) solid var(--border)',
  fontSize: 'var(--text-sm)',
  color: 'var(--text-secondary)'
}
</script>

<template>
  <Shell
    :title="header.title"
    :subtitle="header.subtitle"
    :tab="route.tab"
    :can-back="canGoBack(route)"
    :tabs="screen.screen !== 'new-task'"
    :fill="screen.screen === 'conversation'"
    @back="route = back(route)"
    @tab="(tab) => (route = selectTab(route, tab))"
  >
    <template #actions>
      <Button v-if="screen.screen === 'board'" size="sm" variant="primary" icon="plus" @click="go({ screen: 'new-task' })">
        New task
      </Button>
    </template>
    <div v-if="props.banner" :style="bannerStyle">
      <span>A newer version of the app is available.</span>
      <span>
        <Button size="sm" variant="ghost" @click="openStore(props.storeUrl)">Update</Button>
        <Button size="sm" variant="ghost" @click="dismissRecommended">Dismiss</Button>
      </span>
    </div>
    <BoardScreen v-if="screen.screen === 'board'" @sheet="openSheet" />
    <TaskScreen v-else-if="screen.screen === 'task'" :id="screen.id" @open="(id) => go({ screen: 'task', id })" />
    <NewTaskScreen
      v-else-if="screen.screen === 'new-task'"
      :parent="screen.parent ?? null"
      @started="intoConversation"
    />
    <AgentsScreen v-else-if="screen.screen === 'agents'" @open="go" />
    <ConversationScreen v-else-if="screen.screen === 'conversation'" :id="screen.id" :driven="screen.driven" />
    <RunsScreen v-else-if="screen.screen === 'runs'" :project="route.project" />
    <RunsScreen
      v-else-if="screen.screen === 'run'"
      :project="settings.activeProject"
      :scope="screen.scope"
      :title="screen.title"
      @started="route = selectTab(route, 'runs')"
    />
    <ProjectsScreen v-else-if="screen.screen === 'projects'" @switched="route = selectTab(route, 'board')" />
    <TaskSheet
      v-if="sheetCard"
      :card="sheetCard"
      :rows="rows"
      :error="sheetError"
      @close="closeSheet"
      @act="onAct"
    />
    <div v-if="confirming && confirmed" :style="modalLayer">
      <DeleteTaskModal
        v-if="confirming.kind === 'delete'"
        open
        :id="confirming.id"
        :task-title="confirmed.title"
        @close="confirming = null"
        @confirm="confirmDelete"
      />
      <ReadyTaskModal
        v-else
        open
        :id="confirming.id"
        :task-title="confirmed.title"
        :questions="openQuestions(confirmed.notes)"
        @close="confirming = null"
        @confirm="confirmReady"
        @resolve="resolveInstead"
      />
    </div>
  </Shell>
</template>
