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

export const basename = (path) => path.split('/').filter(Boolean).pop() ?? path

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

/* setProject стоит около двух секунд, и ничто не мешает щёлкнуть по второй
   строке (или удалить проект), пока едет первая. Без этой метки победил бы
   тот ответ, что вернулся последним, — не тот клик, что был последним, — и
   общий finally снял бы trackerState.switching посреди чужого перелёта.
   switchTo/addProject/removeProject проверяют её первым делом и молча ничего
   не делают, если переезд уже идёт: доска на это время и так показывает
   скелет, человеку видно, что работа идёт, а вторая попытка ничего не
   добавляет. Поднимается и снимается через try/finally, чтобы отказ
   (например, setProject упал) не запер список навсегда. */
let moving = false

/* Общая часть переезда: раскладку нового проекта приносит settings_load,
   задачи — tracker_set_project. Вызывающие сами отвечают за флаг moving и за
   flushPending() состояния уходящего проекта до вызова. */
async function moveTo(path) {
  settings.activeProject = path
  await loadSettings(path)
  await setProject(path)
  refreshProbes()
}

/* Переезд. Порядок важен: сначала дописываем состояние уходящего проекта,
   потом забираем раскладку нового, и только потом просим доску — она стоит
   около двух секунд, и всё это время экран показывает скелет. */
export async function switchTo(path) {
  if (path === settings.activeProject || moving) return
  moving = true
  try {
    await flushPending()
    await moveTo(path)
  } finally {
    moving = false
  }
}

export async function addProject() {
  if (moving) return
  let picked = null
  try {
    picked = await open({ directory: true, multiple: false, title: 'Add project' })
  } catch (err) {
    console.error('[projects] выбрать папку не удалось:', err)
    return
  }
  if (!picked) return
  /* Диалог сам стоит сколько угодно — переезд мог начаться и кончиться, пока
     человек выбирал папку, так что флаг проверяется заново после возврата,
     не только на входе в функцию. */
  if (moving) return

  moving = true
  try {
    if (!settings.openProjects.includes(picked)) settings.openProjects.push(picked)
    if (picked === settings.activeProject) return
    await flushPending()
    await moveTo(picked)
  } finally {
    moving = false
  }
}

/* Удаление из списка. Состояние проекта остаётся в файле настроек и вернётся,
   если открыть его снова. Активным становится следующий, а для последней
   строки — предыдущий; опустевший список оставляет окно без проекта, и это
   нормальное состояние. */
export async function removeProject(path) {
  if (moving) return
  moving = true
  try {
    /* Как и в switchTo: состояние уходящего проекта обязано лечь на диск до
       того, как список поменяется, иначе четырёхсотмиллисекундный дебаунс
       перезапишет несохранённую правку уже урезанным списком. */
    await flushPending()
    const at = settings.openProjects.indexOf(path)
    if (at === -1) return
    settings.openProjects.splice(at, 1)
    delete probes[path]

    if (path !== settings.activeProject) return

    const next = settings.openProjects[at] ?? settings.openProjects[at - 1] ?? null
    await moveTo(next)
  } finally {
    moving = false
  }
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
