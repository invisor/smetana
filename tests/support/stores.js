import { vi } from 'vitest'
import { installIpc } from './ipc.js'

/* A fresh store graph per test.

   Stores are module singletons, and they hold more state than the reactive
   objects they export: timer, chain, watching, closing in settings.js, chain
   and ask in tabs.js, moving in projects.js. Vitest gives a fresh module
   registry per file but not per test, so the graph is rebuilt here.

   Every store comes from one graph deliberately: projects.js imports the
   others, and a store from another instance would look at a different
   settings.settings.

   nextTick is handed out from here rather than imported statically by the test:
   resetModules recreates vue too, and another instance's nextTick drives
   another scheduler — the test would wait for a tick that never comes in the
   fresh graph. */
export async function loadStores() {
  vi.resetModules()
  const ipc = installIpc()

  const [vue, event, files, settings, tabs, tracker, projects, terminals, git, runs, attachments] =
    await Promise.all([
      import('vue'),
      import('@tauri-apps/api/event'),
      import('../../src/stores/files.js'),
      import('../../src/stores/settings.js'),
      import('../../src/stores/tabs.js'),
      import('../../src/stores/tracker.js'),
      import('../../src/stores/projects.js'),
      import('../../src/stores/terminals.js'),
      import('../../src/stores/git.js'),
      import('../../src/stores/runs.js'),
      import('../../src/stores/attachments.js')
    ])

  return {
    ipc,
    emit: event.emit,
    nextTick: vue.nextTick,
    stores: { files, settings, tabs, tracker, projects, terminals, git, runs, attachments }
  }
}
