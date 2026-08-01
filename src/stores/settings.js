/* Настройки приложения во фронте. Компоненты видят обычный реактивный объект;
   про Tauri, диск и версию схемы знает только этот файл — как tracker.js
   знает про трекер.

   Разница с трекером принципиальная: там истина снаружи, в bd, и хранилище
   догоняет её дельтами. Здесь истина в этом объекте — настройки меняет только
   этот интерфейс, а Rust отвечает за схему и диск. */
import { reactive, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'

/* Умолчания повторяют умолчания в Rust. Если бэкенда нет (браузер) или чтение
   упало, приложение всё равно обязано открыться в известном виде. */
const defaults = () => ({
  appearance: { theme: 'dark', density: 'comfortable' },
  layout: { leftCollapsed: false, rightCollapsed: false },
  openProjects: [],
  activeProject: null,
  project: {
    sideTab: 'files',
    activeTab: 'kanban',
    selectedTask: null,
    selectedPath: null,
    expanded: [],
    usedAt: null
  }
})

/* Экспортируется ради браузерной заглушки: она отвечает теми же умолчаниями,
   и второй их копии в проекте быть не должно. */
export { defaults }

export const settings = reactive(defaults())

/* Запись стоит похода на диск, а панель за одно перетаскивание меняется
   десятки раз. Копим и пишем один раз, когда поток утих. */
const SAVE_DELAY = 400
/* Сколько ждём запись при закрытии. Больше — и зависший IPC превратится в
   окно, которое не закрывается; это хуже потерянной последней правки. */
const CLOSE_FLUSH_LIMIT = 2000
let timer = null
let watching = false
let closing = false

/* Записи выстроены в цепочку: одновременно в полёте всегда не больше одной.
   Rust пишет через временный файл и переименование, и две записи внахлёст
   спорили бы за порядок — вторая могла бы лечь на диск раньше первой. */
let chain = Promise.resolve()

function scheduleSave() {
  clearTimeout(timer)
  timer = setTimeout(flush, SAVE_DELAY)
}

function flush() {
  timer = null
  /* Реактивный прокси не переживает переход через IPC: отправляем простой
     объект. structuredClone здесь нельзя — цель сборки es2021. Снимок берём
     сейчас, а не в момент отправки: в очереди состояние успеет уехать. */
  const snapshot = JSON.parse(JSON.stringify(settings))
  chain = chain.then(() =>
    invoke('settings_save', { settings: snapshot }).catch((err) => {
      console.error('[settings] сохранить не удалось:', err)
    })
  )
  return chain
}

/* Отправляет отложенное немедленно и отдаёт обещание, по которому видно,
   когда диск догнал состояние. */
export function flushPending() {
  if (timer) {
    clearTimeout(timer)
    return flush()
  }
  return chain
}

/* Закрытие окна — крестиком, Cmd+W, системным меню «закрыть окно» — идёт
   через этот обработчик. Выход через Cmd+Q — другое дело: на macOS так можно
   завершить процесс, вовсе не доведя per-window close request до вебвью, и
   здесь это никем не проверено. За этот случай отвечает только дебаунс
   SAVE_DELAY (400 мс) — если правка внутри него, а Cmd+Q обработчик не
   увидел, она может не успеть на диск. Для того закрытия, что до нас
   доходит, просим Tauri его придержать: дожимаем запись и закрываем окно
   сами.

   Что здесь обещано: окно закроется. Повторный запрос игнорируем, ожидание
   записи ограничено CLOSE_FLUSH_LIMIT, destroy зовём и после отказа записи.
   Чего не обещано: что правка успеет лечь на диск — если бэкенд молчит две
   секунды, окно всё равно закроется, и правка будет потеряна. */
async function closeAfterFlush() {
  if (closing) return
  closing = true
  try {
    await Promise.race([
      flushPending(),
      new Promise((resolve) => setTimeout(resolve, CLOSE_FLUSH_LIMIT))
    ])
  } catch (err) {
    console.error('[settings] запись при закрытии не удалась:', err)
  }
  try {
    await getCurrentWindow().destroy()
  } catch (err) {
    /* Отказ здесь не должен запереть окно навсегда: сбрасываем флаг, чтобы
       следующий запрос на закрытие снова попал в этот обработчик, а не был
       молча проглочен re-entrancy guard'ом. Запись к этому моменту уже
       завершена (успешно или нет), так что повторный заход не повторит flush. */
    closing = false
    console.error('[settings] закрыть окно не удалось:', err)
  }
}

/* В браузере окна Tauri нет: getCurrentWindow падает синхронно, раньше, чем
   дело доходит до подписки. Это нормальный режим, а не поломка — перехватывать
   в браузере нечего, окна для этого попросту нет, а то немногое, что там
   можно сохранить, уже покрыто запасным beforeunload; поэтому здесь debug, а
   не error — красная строка на каждой загрузке страницы прятала бы настоящие
   ошибки. Подписка (второй catch) — другое дело: это уже настоящий Tauri, и
   если onCloseRequested отклонит (например, ACL), это сигнал, а не норма, —
   там уровень остаётся прежним. */
function watchClose() {
  try {
    getCurrentWindow()
      .onCloseRequested((event) => {
        event.preventDefault()
        closeAfterFlush()
      })
      .catch((err) => console.warn('[settings] закрытие окна не перехвачено:', err))
  } catch (err) {
    console.debug('[settings] закрытие окна не перехвачено (браузер, окна нет):', err)
  }
}

/* `project` — «прочитай состояние вот этого проекта»: так стор проектов
   забирает раскладку при переключении, не перезапуская приложение. */
export async function loadSettings(project = null) {
  try {
    const stored = await invoke('settings_load', { project })
    settings.appearance = { ...settings.appearance, ...stored.appearance }
    settings.layout = { ...settings.layout, ...stored.layout }
    settings.project = { ...defaults().project, ...stored.project }
    settings.openProjects = stored.openProjects ?? []
    settings.activeProject = stored.activeProject ?? null
  } catch (err) {
    console.error('[settings] прочитать не удалось, берём умолчания:', err)
  }

  /* Слежение включаем только после загрузки: иначе раскладка прочитанных
     значений сама вернулась бы на диск как «изменение». */
  if (!watching) {
    watch(settings, scheduleSave, { deep: true })
    window.addEventListener('beforeunload', flushPending)
    watchClose()
    watching = true
  }
  return settings
}
