/* Настройки приложения во фронте. Компоненты видят обычный реактивный объект;
   про Tauri, диск и версию схемы знает только этот файл — как tracker.js
   знает про трекер.

   Разница с трекером принципиальная: там истина снаружи, в bd, и хранилище
   догоняет её дельтами. Здесь истина в этом объекте — настройки меняет только
   этот интерфейс, а Rust отвечает за схему и диск. */
import { reactive, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'

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
let timer = null
let watching = false

function scheduleSave() {
  clearTimeout(timer)
  timer = setTimeout(flush, SAVE_DELAY)
}

async function flush() {
  timer = null
  try {
    /* Реактивный прокси не переживает переход через IPC: отправляем простой
       объект. structuredClone здесь нельзя — цель сборки es2021. */
    await invoke('settings_save', { settings: JSON.parse(JSON.stringify(settings)) })
  } catch (err) {
    console.error('[settings] сохранить не удалось:', err)
  }
}

/* Закрытие окна не ждёт дебаунс — последняя правка иначе пропала бы. Дожать
   запись до конца при закрытии нельзя: вебвью умирает раньше, чем ответит
   бэкенд. Это лучшее, что здесь можно сделать, и обещать больше нечего. */
function flushPending() {
  if (timer) {
    clearTimeout(timer)
    flush()
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
    watching = true
  }
  return settings
}
