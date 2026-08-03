import { vi } from 'vitest'
import { installIpc } from './ipc.js'

/* Свежий граф сторов на каждый тест.

   Сторы — модульные синглтоны, и состояния в них больше, чем экспортированные
   реактивные объекты: timer, chain, watching, closing в settings.js, chain и
   ask в tabs.js, moving в projects.js. Vitest даёт свежий реестр модулей на
   файл, но не на тест, поэтому граф пересобирается здесь.

   Все пять сторов берутся из одного графа намеренно: projects.js импортирует
   остальные четыре, и стор из другого экземпляра смотрел бы на другой
   settings.settings.

   nextTick отдаётся отсюда, а не импортируется тестом статически: resetModules
   пересоздаёт и vue, а nextTick чужого экземпляра дёргает чужой планировщик —
   тест ждал бы тик, который в свежем графе не наступит. */
export async function loadStores() {
  vi.resetModules()
  const ipc = installIpc()

  const [vue, event, files, settings, tabs, tracker, projects] = await Promise.all([
    import('vue'),
    import('@tauri-apps/api/event'),
    import('../../src/stores/files.js'),
    import('../../src/stores/settings.js'),
    import('../../src/stores/tabs.js'),
    import('../../src/stores/tracker.js'),
    import('../../src/stores/projects.js')
  ])

  return {
    ipc,
    emit: event.emit,
    nextTick: vue.nextTick,
    stores: { files, settings, tabs, tracker, projects }
  }
}
