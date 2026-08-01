/* Проекты окна: состав списка, активный и переезд между ними.
   Третий и последний файл во фронте, знающий про Tauri, — вместе с
   tracker.js и settings.js.

   Истина по составу списка живёт в настройках (их пишет только этот
   интерфейс), истина по доске — в bd. Поэтому переключение и состоит из двух
   половин: раскладку приносит settings_load, задачи — tracker_set_project. */
import { computed, reactive } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { flushPending, loadSettings, settings } from './settings.js'
import { initBd, probeProjects, setProject } from './tracker.js'

/* Путь → есть ли в нём .beads. Про активный проект то же самое говорит
   health, но про остальные строки узнать больше неоткуда. */
const probes = reactive({})

const basename = (path) => path.split('/').filter(Boolean).pop() ?? path

export const projectRows = computed(() =>
  settings.openProjects.map((path) => ({
    path,
    name: basename(path),
    /* Пока проверка не вернулась, считаем, что трекер есть: показать и убрать
       предупреждение хуже, чем показать его на полсекунды позже. */
    tracked: probes[path] ?? true
  }))
)

export const activePath = computed(() => settings.activeProject)

export async function refreshProbes() {
  const rows = await probeProjects([...settings.openProjects])
  for (const row of rows) probes[row.path] = row.tracked
}

/* Переезд. Порядок важен: сначала дописываем состояние уходящего проекта,
   потом забираем раскладку нового, и только потом просим доску — она стоит
   около двух секунд, и всё это время экран показывает скелет. */
export async function switchTo(path) {
  if (path === settings.activeProject) return
  await flushPending()
  settings.activeProject = path
  await loadSettings(path)
  await setProject(path)
  refreshProbes()
}

export async function addProject() {
  let picked = null
  try {
    picked = await open({ directory: true, multiple: false, title: 'Add project' })
  } catch (err) {
    console.error('[projects] выбрать папку не удалось:', err)
    return
  }
  if (!picked) return

  if (!settings.openProjects.includes(picked)) settings.openProjects.push(picked)
  await switchTo(picked)
}

/* Удаление из списка. Состояние проекта остаётся в файле настроек и вернётся,
   если открыть его снова. Активным становится следующий, а для последней
   строки — предыдущий; опустевший список оставляет окно без проекта, и это
   нормальное состояние. */
export async function removeProject(path) {
  const at = settings.openProjects.indexOf(path)
  if (at === -1) return
  settings.openProjects.splice(at, 1)
  delete probes[path]

  if (path !== settings.activeProject) return

  const next = settings.openProjects[at] ?? settings.openProjects[at - 1] ?? null
  settings.activeProject = next
  await loadSettings(next)
  await setProject(next)
}

/* bd init в активном каталоге. Ошибку глотаем: её уже показал Toast через
   trackerState.lastError. */
export async function initActive() {
  try {
    await initBd()
    await refreshProbes()
  } catch {
    /* сообщение уже лежит в trackerState.lastError */
  }
}

/* Первый запуск: настройки принесли активный проект, которого ещё нет в
   списке (его нашли по рабочему каталогу). Кладём — и список перестаёт быть
   пустым навсегда. */
export function adoptInitialProject() {
  const active = settings.activeProject
  if (active && !settings.openProjects.includes(active)) settings.openProjects.push(active)
  refreshProbes()
}
