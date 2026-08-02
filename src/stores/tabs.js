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

/* Путь → { text, original, mtime, error, saveError, stale, loading }.
   text/original различаются ровно тогда, когда вкладка грязная.
   loading — первое чтение ещё не вернулось. Буфер уже отрисован, поле уже на
   экране, и без этого признака набранный до возврата чтения символ выглядел
   бы как правка поверх содержимого, которого никто не видел: пришедший с
   диска текст ушёл бы в мусор, а его метка легла бы в буфер как своя, и
   первый же Cmd+S заменил бы весь файл этим символом. Пока признак стоит,
   буфер не правится (setText) и не пишется (saveTab), а поле нередактируемо —
   так же ведёт себя VS Code.
   error — отказ чтения ({ kind, message }); правки нет, а text обычно пуст —
   кроме вкладки, под которой файл исчез уже открытым: там остаётся то, что
   успели прочитать.
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
     иначе вкладку закроют, не спросив, и он пропадёт молча.
     Загружающийся буфер по той же причине чист: text и original у него оба
     пусты, а править его до возврата чтения всё равно нельзя. */
  return !!buffer && buffer.text !== buffer.original
}

export const dirtyPaths = computed(() => project().openTabs.filter(isDirty))

/* Замок на вкладке говорит «править нельзя», и причину он обязан назвать свою:
   агентов приложение пока не спрашивает ни о чём, а запирает вкладку отказ
   чтения — двоичный файл, не UTF-8, слишком велик, исчез. Поэтому наружу едет
   не голый флаг, а текст.

   Первое чтение замка не получает, хотя поле на это время тоже не правится:
   оно длится миллисекунды, и замок, мигающий на каждом открытии файла, врал бы
   про постоянное свойство вкладки. */
const readOnlyHint = (buffer) => (buffer?.error ? fileErrorText(buffer.error) : null)

export const tabList = computed(() => [
  ...PINNED,
  ...project().openTabs.map((path) => {
    const hint = readOnlyHint(buffers.get(path))
    return {
      id: path,
      kind: path === project().previewTab ? 'preview' : 'file',
      label: basenameOf(path),
      dirty: isDirty(path),
      readOnly: !!hint,
      readOnlyHint: hint ?? undefined
    }
  })
])

export const activeBuffer = computed(() => buffers.get(project().activeTab) ?? null)

async function load(path, { force = false } = {}) {
  buffers.set(path, {
    text: '',
    original: '',
    mtime: 0,
    error: null,
    saveError: null,
    stale: false,
    loading: true
  })
  try {
    const file = await readFile(path)
    /* Пока файл читался, вкладку могли закрыть или сменить проект. Класть
       содержимое в буфер, которого уже нет, значило бы воскресить вкладку. */
    if (!buffers.has(path)) return
    const current = buffers.get(path)
    /* Ветка сторожит перечитывание, а не первичную загрузку: правки, набранной
       во время первого чтения, взяться неоткуда — `loading` не пускает её в
       буфер. Сюда попадает буфер, который уже жил и был грязным, когда его
       позвали читать заново. Содержимое с диска не должно стирать набранное:
       забираем только метку времени, текст остаётся человеку, вкладка остаётся
       грязной. force приходит от reloadTab — там перезапись и есть то, о чём
       попросили. */
    if (!force && current.text !== current.original) {
      buffers.set(path, { ...current, mtime: file.mtime, loading: false })
      return
    }
    buffers.set(path, {
      text: file.text,
      original: file.text,
      mtime: file.mtime,
      error: null,
      saveError: null,
      stale: false,
      loading: false
    })
  } catch (error) {
    if (!buffers.has(path)) return
    const current = buffers.get(path)
    /* Тот же случай, что и в ветке успеха, и сторожит она то же самое —
       перечитывание. Отказ чтения не повод выбросить набранное руками, поэтому
       текст остаётся, а ошибка просто прикладывается к нему. force приходит от
       reloadTab: там человек сам попросил забыть свои правки. */
    if (!force && current.text !== current.original) {
      buffers.set(path, { ...current, error, loading: false })
      return
    }
    buffers.set(path, {
      text: '',
      original: '',
      mtime: 0,
      error,
      saveError: null,
      stale: false,
      loading: false
    })
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
  /* Пока первое чтение не вернулось, править нечего: буфер пуст не потому,
     что файл пуст. Правка в него стала бы правкой поверх невидимого. */
  if (!buffer || buffer.error || buffer.loading) return
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
  /* loading: писать содержимое, которого никто не читал, нечем и незачем. */
  if (!buffer || buffer.error || buffer.loading || !isDirty(path)) return chain
  const text = buffer.text
  chain = chain
    .then(async () => {
      /* Метку берём в момент выполнения, а не постановки в очередь: первая из
         двух записей подряд уже сдвинула её на диске, и метка, захваченная при
         постановке, дала бы `stale` на собственное сохранение. Текст, наоборот,
         захвачен при постановке — сохраняем то, что человек попросил. */
      const before = buffers.get(path)
      if (!before || before.error || before.loading) return
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
    /* Отвергнутое обещание, оставленное в цепочке, отравило бы все следующие
       записи: их `.then` молча пропустил бы тело, и Cmd+S перестал бы что-либо
       делать до перезапуска. Отказ самой записи разобран выше, сюда попадает
       только неожиданное — но и оно не имеет права остановить очередь. */
    .catch((err) => {
      console.error('[tabs] запись не удалась:', err)
    })
  return chain
}

/* Файл уехал под грязной вкладкой — это заметил проход по фокусу окна.
   Решение за человеком, поэтому вкладка получает полоску с кнопками.

   Отказ чтения при этом снимается, а не сохраняется: сюда приходят и те
   буферы, у которых он был, — файл удалили и тут же вернули, права починили.
   Файл на месте, набранный текст на месте, и оставлять поле запертым значило
   бы заморозить его навсегда: из `error` больше ничего не выводит. */
export function markStale(path) {
  const buffer = buffers.get(path)
  if (!buffer || buffer.loading) return
  buffers.set(path, { ...buffer, error: null, stale: true })
}

/* Файл исчез под чистой вкладкой. Терять нечего, и перечитывать нечего, но и
   показывать содержимое того, чего нет, часами нельзя. Отказ чтения тут не
   выдумка, а правда: следующее чтение вернуло бы ровно его, — а вместе с ним
   приходят и объяснение под полоской, и замок на вкладке, и запертое поле.
   Вернувшийся файл поднимет вкладку обратно — этим занят проход по фокусу. */
export function markGone(path) {
  const buffer = buffers.get(path)
  if (!buffer || buffer.loading || buffer.error) return
  buffers.set(path, { ...buffer, error: { kind: 'notFound' }, stale: false })
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

export function saveTabs(paths) {
  return Promise.all(paths.filter(isDirty).map(saveTab))
}

/* Отказ от несохранённого в перечисленных вкладках — ответ «Не сохранять».
   Список обязателен: спросили про одну вкладку — трогаем только её. */
export function discardTabs(paths) {
  for (const path of paths) {
    const buffer = buffers.get(path)
    if (buffer) buffers.set(path, { ...buffer, original: buffer.text })
  }
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
