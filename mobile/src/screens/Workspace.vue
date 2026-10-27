<script setup>
/* The working screens and the way between them: what the app is once the desktop is reachable.

   Four tabs - the board, the agents, the runs, the projects - and screens opened over them (a
   task, a conversation, the form for a new task). Where the person is, is one route
   (../nav.js, pure and tested); this component only draws it and moves it. A tapped
   notification moves it too: the target the core holds is turned into a route and spent. */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import Button from '@src/components/core/Button.vue'
import { settings } from '@src/stores/settings.js'
import { basename } from '@src/paths.js'
import { back, canGoBack, current, open, routeForTarget, selectTab, START } from '../nav.js'
import { clearPushTarget, pushTarget } from '../pushTarget.js'
import { dismissRecommended } from '../bridge/index.js'
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
      return { title: 'New task', subtitle: project.value }
    case 'conversation':
      return { title: s.driven ? 'Conversation' : 'Terminal session', subtitle: project.value }
    case 'runs':
      return { title: 'Runs', subtitle: route.value.project ? basename(route.value.project) : project.value }
    default:
      return { title: TITLES[s.screen] ?? 'Smetana', subtitle: s.screen === 'projects' ? '' : project.value }
  }
})

const go = (entry) => (route.value = open(route.value, entry))

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
    <BoardScreen v-if="screen.screen === 'board'" @open="(id) => go({ screen: 'task', id })" />
    <TaskScreen v-else-if="screen.screen === 'task'" :id="screen.id" @open="(id) => go({ screen: 'task', id })" />
    <NewTaskScreen
      v-else-if="screen.screen === 'new-task'"
      @started="(id) => (route = { tab: 'agents', project: null, stack: [{ screen: 'conversation', id, driven: true }] })"
    />
    <AgentsScreen v-else-if="screen.screen === 'agents'" @open="go" />
    <ConversationScreen v-else-if="screen.screen === 'conversation'" :key="`${screen.driven}:${screen.id}`" :id="screen.id" :driven="screen.driven" @close="route = back(route)" />
    <RunsScreen v-else-if="screen.screen === 'runs'" :project="route.project" />
    <ProjectsScreen v-else-if="screen.screen === 'projects'" @switched="route = selectTab(route, 'board')" />
  </Shell>
</template>
