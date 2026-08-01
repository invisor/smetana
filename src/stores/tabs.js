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

/* Путь → { text, original, mtime, error, saveError, stale }.
   text/original различаются ровно тогда, когда вкладка грязная.
   error — отказ чтения ({ kind, message }); тогда text пуст и правки нет.
   saveError — отказ записи ({ kind, message }). Разделены намеренно: error
   значит «файла нет как текста», и потому запирает поле (readOnly у вкладки и
   у редактора), а отказ записи оставляет и текст, и право его править —
   иначе набранное оказалось бы заперто ровно в тот момент, когда его не
   удалось сохранить. Гаснет при первой удавшейся записи.
   stale — файл уехал на диске под грязной вкладкой; сама метка тут не нужна,
   нужен только факт, а свежую метку принесёт keepMine или reloadTab. */
export const buffers = reactive(new Map())

const project = () => settings.project

export const isDirty = (path) => {
  const buffer = buffers.get(path)
  /* Ошибка не делает буфер чистым. У файла, который не удалось открыть,
     text и original пусты, и сравнение само даёт false; а вот текст, который
     человек успел набрать до отказа чтения, обязан считаться несохранённым —
     иначе вкладку закроют, не спросив, и он пропадёт молча. */
  return !!buffer && buffer.text !== buffer.original
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

async function load(path, { force = false } = {}) {
  buffers.set(path, { text: '', original: '', mtime: 0, error: null, saveError: null, stale: false })
  try {
    const file = await readFile(path)
    /* Пока файл читался, вкладку могли закрыть или сменить проект. Класть
       содержимое в буфер, которого уже нет, значило бы воскресить вкладку. */
    if (!buffers.has(path)) return
    const current = buffers.get(path)
    /* Пока файл читался, в буфер могли напечатать. Содержимое с диска не
       должно стирать набранное: забираем только метку времени, текст остаётся
       человеку, вкладка остаётся грязной. force приходит от reloadTab — там
       перезапись и есть то, о чём попросили. */
    if (!force && current.text !== current.original) {
      buffers.set(path, { ...current, mtime: file.mtime })
      return
    }
    buffers.set(path, {
      text: file.text,
      original: file.text,
      mtime: file.mtime,
      error: null,
      saveError: null,
      stale: false
    })
  } catch (error) {
    if (!buffers.has(path)) return
    const current = buffers.get(path)
    /* Тот же случай, что и в ветке успеха: пока файл читался, в буфер могли
       напечатать. Отказ чтения — не повод выбросить набранное руками, поэтому
       текст остаётся, а ошибка просто прикладывается к нему. force приходит от
       reloadTab: там человек сам попросил забыть свои правки. */
    if (!force && current.text !== current.original) {
      buffers.set(path, { ...current, error })
      return
    }
    buffers.set(path, { text: '', original: '', mtime: 0, error, saveError: null, stale: false })
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

  /* Место временной вкладки занимает только другая временная. Двойной клик
     открывает постоянную рядом: слот превью принадлежит тому файлу, который
     сейчас просматривают, и клик по третьему файлу не должен его выселять. */
  const previewAt =
    !permanent && state.previewTab ? state.openTabs.indexOf(state.previewTab) : -1
  if (previewAt !== -1) {
    /* Замена на том же месте: ряд не должен перестраиваться от того, что
       человек просматривает файлы один за другим. Временная вкладка никогда
       не бывает грязной, поэтому спрашивать не о чем. */
    buffers.delete(state.openTabs[previewAt])
    state.openTabs.splice(previewAt, 1, path)
  } else {
    state.openTabs.push(path)
  }

  /* Постоянная вкладка не отменяет чужое превью: если временной была вкладка
     другого файла, она такой и остаётся. Обнулять `previewTab` тут значило бы
     снимать курсив со вкладки, к которой человек не прикасался. */
  if (!permanent) state.previewTab = path
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
    /* Метку берём в момент выполнения, а не постановки в очередь: первая из
       двух записей подряд уже сдвинула её на диске, и метка, захваченная при
       постановке, дала бы `stale` на собственное сохранение. Текст, наоборот,
       захвачен при постановке — сохраняем то, что человек попросил. */
    const before = buffers.get(path)
    if (!before || before.error) return
    try {
      const mtime = await writeFile(path, text, before.mtime)
      const current = buffers.get(path)
      if (!current) return
      /* original ставим равным тому, что записали, а не текущему тексту:
         человек мог продолжить печатать, пока запись летела, и его новые
         правки обязаны остаться грязными. */
      buffers.set(path, { ...current, original: text, mtime, saveError: null, stale: false })
    } catch (error) {
      const current = buffers.get(path)
      if (!current) return
      if (error.kind === 'stale') {
        /* Ничего не записано и ничего не потеряно. Показываем полоску и ждём
           решения: перечитать или оставить своё. */
        buffers.set(path, { ...current, stale: true })
      } else {
        /* Отказ записи — не отказ чтения. В `error` живёт «файл не удалось
           открыть», и оно делает поле нередактируемым; для неудавшегося
           сохранения это заперло бы набранный текст: ни поправить, ни
           сохранить заново. */
        buffers.set(path, { ...current, saveError: error })
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
  await load(path, { force: true })
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

/* Кто спрашивает про несохранённое.
 *
 * Вопрос задаёт вид (модалка живёт в DesktopApp.vue), а поводов для него три:
 * закрытие вкладки, переключение проекта и закрытие окна. Два последних
 * приходят из сторов, которые про интерфейс ничего не знают, — поэтому вид
 * оставляет здесь свою функцию, а сторы её зовут.
 *
 * Обещание такое: вернуть true значит «можно продолжать», false — «человек
 * передумал». Никто не поставил обработчик — продолжаем: молча терять правки
 * плохо, но запирать приложение из-за незарегистрированного вида хуже.
 */
let ask = null

export function onUnsaved(handler) {
  ask = handler
}

export async function confirmUnsaved(paths = dirtyPaths.value) {
  if (!paths.length) return true
  if (!ask) return true
  return ask(paths)
}
