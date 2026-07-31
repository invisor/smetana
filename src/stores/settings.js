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
function flushPending() {
  if (timer) {
    clearTimeout(timer)
    return flush()
  }
  return chain
}

/* Cmd+Q и крестик рвут нативный вебвью, и цикл unload при этом чаще всего не
   проходит вовсе — на один beforeunload полагаться нельзя, последняя правка
   внутри дебаунса просто пропала бы. Поэтому просим Tauri придержать
   закрытие: дожимаем запись и закрываем окно сами.

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
    console.error('[settings] закрыть окно не удалось:', err)
  }
}

/* В браузере окна Tauri нет: и getCurrentWindow, и сама подписка (она идёт
   через IPC) там падают. Это нормальный режим, а не поломка, — глушим на
   месте, чтобы не оставлять необработанный отказ. */
function watchClose() {
  try {
    getCurrentWindow()
      .onCloseRequested((event) => {
        event.preventDefault()
        closeAfterFlush()
      })
      .catch((err) => console.warn('[settings] закрытие окна не перехвачено:', err))
  } catch (err) {
    console.warn('[settings] закрытие окна не перехвачено:', err)
  }
}

export async function loadSettings() {
  try {
    const stored = await invoke('settings_load')
    settings.appearance = { ...settings.appearance, ...stored.appearance }
    settings.layout = { ...settings.layout, ...stored.layout }
    settings.project = { ...settings.project, ...stored.project }
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
