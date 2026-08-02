<script setup>
/* Desktop app shell — three-column agent control room: scope bar, tab bar,
   kanban, task inspector and live log.

   The core moment this screen is built for: you come back after two hours and
   read, in three seconds, what finished, what stalled, and what is waiting for
   you. Hence the loud budget — exactly one card and one callout shout here. */
import { computed, onMounted, onUnmounted, ref, watch, watchEffect } from 'vue'
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
import Modal from '../components/overlays/Modal.vue'
import Toast from '../components/overlays/Toast.vue'
import LogView from '../components/agent/LogView.vue'
import ProjectList from '../components/shell/ProjectList.vue'
import Skeleton from '../components/core/Skeleton.vue'
import IconButton from '../components/core/IconButton.vue'
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
import { agents, inspector, logLines, scope } from './desktopAppData.js'
import {
  basenameOf,
  fileErrorText,
  filesState,
  isStubPath,
  listDir,
  refreshDirs,
  saveErrorText,
  setRoot,
  statFiles,
  treeNodes
} from '../stores/files.js'
import {
  activeBuffer,
  buffers,
  closeTab,
  confirmUnsaved,
  discardTabs,
  isDirty,
  keepMine,
  markGone,
  markStale,
  onUnsaved,
  openFile,
  promote,
  reloadTab,
  restoreTabs,
  saveTab,
  saveTabs,
  setText,
  tabList
} from '../stores/tabs.js'
import FileEditor from '../components/files/FileEditor.vue'

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
   одних true. Set нужен дереву, чтобы знать, куда спускаться. */
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

/* Дерево и вкладки открываются вместе с проектом. Активный проект к этому
   моменту уже прочитан настройками — App.vue ждёт loadSettings до того, как
   вообще нарисует этот вид.

   Переезд на другой проект делает ровно то же самое (moveTo в projects.js), и
   он может начаться, пока этот проход ещё в awaitʼах — щелчком по строке
   списка. Тогда доделывать нечего: сверяемся с активным проектом после каждого
   ожидания и уходим, если он сменился. Побеждает переезд, а не тот, кто начал
   раньше. */
onMounted(async () => {
  const opened = activePath.value
  if (!opened) return
  setRoot(opened)
  await listDir('')
  if (activePath.value !== opened) return
  await Promise.all(project.expanded.map((dir) => listDir(dir)))
  if (activePath.value !== opened) return
  await restoreTabs()
})
/* Приложение существует ради сценария «вернулся через два часа и смотрю, что
   изменилось», — значит, момент возврата фокуса и есть тот момент, когда надо
   догнать диск. Вотчера у файлов нет намеренно: вторая вотчер-подсистема в
   Rust со своим жизненным циклом и отчётом об ошибках дороже, чем этот проход.

   Чистая вкладка перечитывается молча: терять нечего, а показывать устаревший
   текст часами — хуже. Грязная получает полоску и ждёт решения.

   Вкладка с отказом чтения тоже проходит через этот проход, а не пропускается:
   из `error` нет другого выхода — поле заперто, запись отказана, а кнопка
   «Reload» живёт под полоской, которой у этой вкладки нет. Файл, который
   удалили и тут же вернули (обычное дело рядом с агентом), иначе остался бы
   мёртвым до перезапуска. */
const catchUp = async () => {
  if (!activePath.value) return
  refreshDirs(['', ...project.expanded])

  const open = [...project.openTabs]
  if (!open.length) return
  /* Пока метки ехали, проект могли переключить: буферы уже чужие, и трогать их
     нельзя. Тот же приём, что в listDir. */
  const root = filesState.root
  const stats = await statFiles(open)
  if (filesState.root !== root) return

  for (const stat of stats) {
    const buffer = buffers.get(stat.path)
    if (!buffer || buffer.loading) continue
    if (stat.mtime === null) {
      /* Файла нет. Под грязной вкладкой это ещё не приговор — набранное цело,
         и решение за человеком; под чистой вкладка молча показывала бы
         содержимое того, чего нет. */
      if (isDirty(stat.path)) markStale(stat.path)
      else markGone(stat.path)
      continue
    }
    /* Файл на месте. Буфер с отказом чтения перечитывается в любом случае:
       его метка осталась нулевой, и сравнивать её не с чем — но набранный до
       отказа текст перезаписывать без спроса нельзя, поэтому грязная вкладка
       получает полоску, а не диск. */
    if (!buffer.error && stat.mtime === buffer.mtime) continue
    if (isDirty(stat.path)) markStale(stat.path)
    else reloadTab(stat.path)
  }
}

onMounted(() => window.addEventListener('focus', catchUp))
onUnmounted(() => window.removeEventListener('focus', catchUp))

const initing = ref(false)
const initHere = async () => {
  initing.value = true
  try {
    await initActive()
  } finally {
    initing.value = false
  }
}
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
   до неё "не нашлось" ничего не значит.

   Одной готовности мало: ready означает лишь "снимок пришёл", а снимок
   приходит и пустым — когда bd не нашлось, когда в каталоге нет .beads,
   когда первая синхронизация упала. В этих случаях "не нашлось" говорит не
   про задачу, а про трекер, и стирать выбор нельзя: дебаунс тут же унёс бы
   null на диск, и один запуск со сломанным bd — из Finder, например —
   потерял бы запомненную задачу навсегда. Поэтому спрашиваем и здоровье. */
watch(
  () => [trackerState.ready, trackerState.health.state, trackerState.issues.size],
  () => {
    if (
      trackerState.ready &&
      trackerState.health.state === 'ok' &&
      project.selectedTask &&
      !issueById(project.selectedTask)
    ) {
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

/* Only when there is nothing else to show: a failing bd is no reason to hide
   the tasks that were already read. */
const healthNotice = computed(() => {
  if (trackerState.health.state === 'ok') return null
  if (boardColumns.value.some((column) => column.tasks.length)) return null
  return HEALTH_NOTICE[trackerState.health.state] ?? HEALTH_NOTICE.error
})

/* Вкладка файла — это всё, что не chat и не kanban. Закрытого списка у центра
   нет и не будет: вкладки приносит проект. */
const fileTabActive = computed(
  () => project.activeTab !== 'chat' && project.activeTab !== 'kanban'
)

/* Cmd+S ловит окно, а не поле редактора. Клик по вкладке, по строке дерева или
   по любой кнопке уводит фокус из textarea, и обработчик на самом поле молча
   переставал бы работать — а человек в этот момент как раз и тянется к Cmd+S.
   Знает, есть ли что сохранять, вид, а не поле: сохранять есть что только на
   вкладке файла.

   Клавиша сверяется по `event.code` — это физическая клавиша, и она не зависит
   ни от раскладки, ни от Caps Lock. `event.key` при русской раскладке равен
   'ы', при Caps Lock — 'S', и сравнение с 's' промахивалось бы в обоих
   случаях: сохранения просто не было бы. */
const onSaveKey = (event) => {
  if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) return
  if (event.code !== 'KeyS') return
  /* Отменяем в любом случае: «сохранить страницу» вебвью здесь не к месту
     ни на доске, ни в чате. */
  event.preventDefault()
  if (fileTabActive.value) saveTab(project.activeTab)
}

onMounted(() => window.addEventListener('keydown', onSaveKey))
onUnmounted(() => window.removeEventListener('keydown', onSaveKey))

/* Порядок ветвей — от «файла нет как текста» к «файл есть, но с ним что-то
   случилось»: `error` запирает поле и объясняет пустоту, `stale` спрашивает
   решение и потому единственный несёт кнопки, `saveError` только сообщает.
   Тон у отказа записи тихий, как у остальных: правки на месте, поле осталось
   редактируемым, следующий Cmd+S — обычная попытка, а не восстановление. */
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

/* Раскрытие читает каталог, если его ещё не читали. Сворачивание не забывает
   прочитанное: раскрыть обратно должно быть мгновенно, а свежесть приносит
   проход по фокусу. */
const toggleDir = (path) => {
  const at = project.expanded.indexOf(path)
  if (at === -1) {
    project.expanded.push(path)
    if (!filesState.dirs.has(path)) listDir(path)
  } else {
    project.expanded.splice(at, 1)
  }
}

/* Строка «…N more» — не файл: за ней нет пути на диске, и открытая по ней
   вкладка попала бы в настройки и осталась бы там навсегда. Отсеиваем её здесь,
   в обоих обработчиках: дерево про заглушку не знает и знать не должно. */
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

/* Один вопрос на все несохранённые вкладки. Он встаёт в трёх местах, и во
   всех трёх ответ один и тот же, поэтому и модалка одна. */
const unsaved = ref(null)

onMounted(() =>
  onUnsaved(
    (paths) =>
      new Promise((resolve) => {
        /* Второй вопрос не имеет права осиротить первый: тот, кто ждёт ответа,
           получит «нет» и свернёт свою работу штатно, сняв свои флаги. Иначе
           `moving` в projects.js остался бы взведённым навсегда. */
        if (unsaved.value) unsaved.value.resolve(false)
        unsaved.value = { paths, resolve }
      })
  )
)

/* Ответ обязан разрешить обещание при любом исходе: модалки на экране уже нет,
   а тот, кто ждёт, держит взведённым свой флаг — `closing` в настройках,
   `moving` в проектах. Неразрешённое обещание здесь означает окно, которое не
   закрыть, и список проектов, который не переключить. */
const answerUnsaved = async (answer) => {
  const pending = unsaved.value
  unsaved.value = null
  if (!pending) return
  try {
    if (answer === 'cancel') return pending.resolve(false)
    if (answer === 'save') {
      await saveTabs(pending.paths)
      /* Запись могла не удаться — тогда вкладки остались грязными, и пропускать
         дальше нельзя: закрытие уничтожило бы текст, который просили сохранить.
         Полоска с причиной отказа уже на экране, человек решит сам. */
      if (pending.paths.some(isDirty)) return pending.resolve(false)
    } else {
      discardTabs(pending.paths)
    }
    pending.resolve(true)
  } catch (err) {
    /* Повторный resolve безвреден: обещание разрешается один раз. */
    console.error('[desktop] ответ о несохранённом не отработал:', err)
    pending.resolve(false)
  }
}

const onCloseTab = async (id) => {
  if (isDirty(id) && !(await confirmUnsaved([id]))) return
  closeTab(id)
}

/* Явное обновление дерева. Вотчера у файлов нет намеренно (см. спеку), и это
   вторая половина ответа на вопрос «а что там сейчас на диске» — первая
   срабатывает сама при возврате фокуса в окно. */
const refreshTree = () => refreshDirs(['', ...project.expanded])
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
/* Panel scrolls its slot as one block; the worktree line above and the tab row
   below have to stay put, so only what is between them scrolls. */
const sidebarStyle = {
  display: 'flex',
  flexDirection: 'column',
  height: '100%',
  minHeight: 0
}
/* The sidebar's own tab row: same height as the document tabs but micro type,
   because these are section names and not open files. It sits at the foot of
   the column, so the accent and the rule are the document tabs' mirrored —
   both on the outer edge, away from the content they reveal. */
const sideTabBar = {
  display: 'flex',
  alignItems: 'stretch',
  height: 'var(--tab-h)',
  flex: '0 0 auto',
  borderTop: 'var(--border-w) solid var(--border-subtle)'
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
    boxShadow: active ? 'inset 0 -2px 0 0 var(--text-primary)' : 'none',
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

/* Столбец тостов в углу. Пустым он ничего не занимает и ничего не
   перехватывает: без детей у него нулевой размер. */
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
    <!-- Both names come from the open project: a bar that is half true reads
         worse than one that is honestly fixture. Branch and the counters are
         still fixture — nothing in the app reads git yet. -->
    <ScopeIndicator
      v-bind="scope"
      :repo="activePath ? basename(activePath) : '—'"
      :worktree="activePath ? basename(activePath) : '—'"
    />

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
          <template #actions>
            <!-- Обновлять на вкладках Git и Agents нечего: там фикстура и
                 пустое состояние, и кнопка обещала бы работу, которой нет. -->
            <IconButton
              v-if="project.sideTab === 'files'"
              icon="refresh-cw"
              label="Refresh files"
              size="sm"
              @click="refreshTree"
            />
            <IconButton icon="plus" label="Add project" size="sm" @click="addProject" />
          </template>
          <div :style="sidebarStyle">
            <ProjectList
              :projects="projectRows"
              :active-path="activePath"
              @select="switchTo"
              @remove="removeProject"
            />
            <div :style="{ flex: 1, minHeight: 0, overflow: 'auto' }">
              <FileTree
                v-if="project.sideTab === 'files'"
                :nodes="tree"
                :expanded="expanded"
                :selected-path="project.selectedPath ?? undefined"
                @toggle="toggleDir"
                @select="onSelectFile"
                @open="onOpenFile"
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
          </div>
        </Panel>
      </div>

      <!-- centre: tabs over the board -->
      <div :style="centerStyle">
        <TabBar
          :tabs="tabList"
          :active-id="project.activeTab"
          @select="project.activeTab = $event"
          @close="onCloseTab"
          @promote="promote"
        />
        <NewTaskModal
          :open="newTaskOpen"
          :busy="creating"
          :status="ADD_TO"
          @close="newTaskOpen = false"
          @submit="submitNewTask"
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
        <!-- Вкладка файла: доска и чат к ней отношения не имеют. -->
        <!-- :key — своё поле на каждый файл. Без него textarea переживает смену
             вкладки вместе с прокруткой и кареткой прошлого файла, и открытый
             файл показывается с чужого места. -->
        <FileEditor
          v-if="fileTabActive"
          :key="project.activeTab"
          :path="project.activeTab"
          :model-value="activeBuffer?.text ?? ''"
          :read-only="!!activeBuffer?.error || !!activeBuffer?.loading"
          :notice="editorNotice"
          @update:model-value="setText(project.activeTab, $event)"
          @reload="reloadTab(project.activeTab)"
          @keep-mine="keepMine(project.activeTab)"
        />
        <EmptyState
          v-else-if="project.activeTab === 'chat'"
          icon="message-circle-question-mark"
          title="No chat yet"
          description="Talking to an agent from this window is not built. The board and the log are."
        />
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
            <Button variant="primary" size="sm" @click="addProject">Add project…</Button>
          </template>
        </EmptyState>
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

    <!-- Тосты живут в одном столбце: два фиксированных угла налезли бы друг на
         друга, и отказ трекера скрыл бы отказ диска ровно тогда, когда сломано
         и то, и другое. -->
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
    </div>
  </div>
</template>
