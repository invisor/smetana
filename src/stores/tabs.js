/* Вкладки центра: порядок, временная, активная, буферы и грязнота.
   Про Tauri этот файл не знает — за диск отвечает files.js.

   Разделение проведено по сроку жизни: список вкладок переживает перезапуск и
   потому живёт в настройках, буферы не переживают и потому живут здесь. */
import { computed, reactive } from 'vue'
import { settings } from './settings.js'
import { basenameOf, fileErrorText, readFile, writeFile } from './files.js'

/* Закреплённые вкладки в настройках не хранятся: они есть всегда, стоят
   первыми и не закрываются. */
export const PINNED = [
  { id: 'chat', kind: 'pinned', label: 'Chat' },
  { id: 'kanban', kind: 'pinned', label: 'Kanban' }
]

/* Путь → { text, original, mtime, error, stale }.
   text/original различаются ровно тогда, когда вкладка грязная.
   error — отказ чтения ({ kind, message }); тогда text пуст и правки нет.
   stale — файл уехал на диске под грязной вкладкой; сама метка тут не нужна,
   нужен только факт, а свежую метку принесёт keepMine или reloadTab. */
export const buffers = reactive(new Map())

const project = () => settings.project

export const isDirty = (path) => {
  const buffer = buffers.get(path)
  return !!buffer && !buffer.error && buffer.text !== buffer.original
}

export const dirtyPaths = computed(() => project().openTabs.filter(isDirty))

export const tabList = computed(() => [
  ...PINNED,
  ...project().openTabs.map((path) => ({
    id: path,
    kind: path === project().previewTab ? 'preview' : 'file',
    label: basenameOf(path),
    dirty: isDirty(path),
    readOnly: !!buffers.get(path)?.error
  }))
])

export const activeBuffer = computed(() => buffers.get(project().activeTab) ?? null)

async function load(path) {
  buffers.set(path, { text: '', original: '', mtime: 0, error: null, stale: false })
  try {
    const file = await readFile(path)
    /* Пока файл читался, вкладку могли закрыть или сменить проект. Класть
       содержимое в буфер, которого уже нет, значило бы воскресить вкладку. */
    if (!buffers.has(path)) return
    buffers.set(path, {
      text: file.text,
      original: file.text,
      mtime: file.mtime,
      error: null,
      stale: false
    })
  } catch (error) {
    if (!buffers.has(path)) return
    buffers.set(path, { text: '', original: '', mtime: 0, error, stale: false })
  }
}

/* Открытие файла из дерева. Вся механика VS Code — здесь.

   Одиночный клик открывает временной вкладкой; следующий одиночный клик по
   другому файлу подставляется на её место, а не растит ряд. Двойной клик
   (permanent) открывает сразу постоянной — и закрепляет ту, что уже открыта
   временной. */
export function openFile(path, { permanent = false } = {}) {
  const state = project()
  const at = state.openTabs.indexOf(path)

  if (at !== -1) {
    if (permanent && state.previewTab === path) state.previewTab = null
    state.activeTab = path
    return
  }

  const previewAt = state.previewTab ? state.openTabs.indexOf(state.previewTab) : -1
  if (previewAt !== -1) {
    /* Замена на том же месте: ряд не должен перестраиваться от того, что
       человек просматривает файлы один за другим. Временная вкладка никогда
       не бывает грязной, поэтому спрашивать не о чем. */
    buffers.delete(state.openTabs[previewAt])
    state.openTabs.splice(previewAt, 1, path)
  } else {
    state.openTabs.push(path)
  }

  state.previewTab = permanent ? null : path
  state.activeTab = path
  load(path)
}

/* Двойной клик по вкладке. */
export function promote(path) {
  const state = project()
  if (state.previewTab === path) state.previewTab = null
  state.activeTab = path
}

export function closeTab(path) {
  const state = project()
  const at = state.openTabs.indexOf(path)
  if (at === -1) return
  state.openTabs.splice(at, 1)
  buffers.delete(path)
  if (state.previewTab === path) state.previewTab = null
  if (state.activeTab === path) {
    /* Активной становится соседняя справа, а для последней — слева; вкладок
       не осталось — доска. Так же ведёт себя removeProject со списком
       проектов. */
    state.activeTab = state.openTabs[at] ?? state.openTabs[at - 1] ?? 'kanban'
  }
}

/* Правка. Она же снимает временность — второй из двух способов закрепить
   вкладку, и именно он делает инвариант «временная никогда не грязная»
   истинным. */
export function setText(path, text) {
  const buffer = buffers.get(path)
  if (!buffer || buffer.error) return
  buffers.set(path, { ...buffer, text })
  const state = project()
  if (state.previewTab === path) state.previewTab = null
}

/* Записи выстроены в цепочку: одновременно в полёте всегда не больше одной.
   Два Cmd+S подряд иначе спорили бы за порядок, и вторая запись могла бы лечь
   на диск раньше первой. Тот же приём, что в settings.js. */
let chain = Promise.resolve()

export function saveTab(path) {
  const buffer = buffers.get(path)
  if (!buffer || buffer.error || !isDirty(path)) return chain
  const text = buffer.text
  chain = chain.then(async () => {
    try {
      const mtime = await writeFile(path, text, buffer.mtime)
      const current = buffers.get(path)
      if (!current) return
      /* original ставим равным тому, что записали, а не текущему тексту:
         человек мог продолжить печатать, пока запись летела, и его новые
         правки обязаны остаться грязными. */
      buffers.set(path, { ...current, original: text, mtime, stale: false })
    } catch (error) {
      const current = buffers.get(path)
      if (!current) return
      if (error.kind === 'stale') {
        /* Ничего не записано и ничего не потеряно. Показываем полоску и ждём
           решения: перечитать или оставить своё. */
        buffers.set(path, { ...current, stale: true })
      } else {
        buffers.set(path, { ...current, error })
      }
    }
  })
  return chain
}

/* Файл уехал под вкладкой — это заметил проход по фокусу окна. */
export function markStale(path) {
  const buffer = buffers.get(path)
  if (!buffer || buffer.error) return
  buffers.set(path, { ...buffer, stale: true })
}

/* «Перечитать»: содержимое с диска побеждает, правки уходят. */
export async function reloadTab(path) {
  if (!buffers.has(path)) return
  await load(path)
}

/* «Оставить моё»: полоска гаснет, но метка буфера догоняет диск — иначе
   следующий Cmd+S снова отказал бы `stale`, и выйти из этого было бы нельзя. */
export async function keepMine(path) {
  const buffer = buffers.get(path)
  if (!buffer) return
  try {
    const file = await readFile(path)
    buffers.set(path, { ...buffer, mtime: file.mtime, stale: false })
  } catch (error) {
    buffers.set(path, { ...buffer, error })
  }
}

/* Отказ от всего несохранённого — ответ «Не сохранять» в модалке. */
export function discardAll() {
  for (const path of project().openTabs) {
    const buffer = buffers.get(path)
    if (buffer) buffers.set(path, { ...buffer, original: buffer.text })
  }
}

export function saveAll() {
  return Promise.all(dirtyPaths.value.map(saveTab))
}

/* Переезд на другой проект: буферы старого проекта не должны пережить его
   ни на кадр. Список вкладок при этом не трогаем — он придёт из настроек
   нового проекта. */
export function resetTabs() {
  buffers.clear()
}

/* Восстановление после перезапуска или переключения проекта. Читаем всё, что
   было открыто; путь, чей файл не читается, выпадает из списка молча — ровно
   как выпадает задача, которой больше нет в трекере. */
export async function restoreTabs() {
  const state = project()
  const paths = [...state.openTabs]
  await Promise.all(paths.map(load))

  const gone = paths.filter((path) => buffers.get(path)?.error?.kind === 'notFound')
  if (!gone.length) return
  for (const path of gone) {
    const at = state.openTabs.indexOf(path)
    if (at !== -1) state.openTabs.splice(at, 1)
    buffers.delete(path)
    if (state.previewTab === path) state.previewTab = null
    if (state.activeTab === path) state.activeTab = 'kanban'
  }
}
