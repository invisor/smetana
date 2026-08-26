import { vi } from 'vitest'
import { installIpc } from './ipc.js'

/* Every graph built for the test that is running now, emptied by settleStores
   at the end of it. A test may build more than one: the outer beforeEach makes
   one and the test body asks for another. */
const built = []

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

  const [
    vue,
    event,
    files,
    settings,
    tabs,
    tracker,
    projects,
    terminals,
    git,
    runs,
    attachments,
    notifications,
    updates,
    vcs,
    compare
  ] =
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
      import('../../src/stores/attachments.js'),
      import('../../src/stores/notifications.js'),
      import('../../src/stores/updates.js'),
      import('../../src/stores/vcs.js'),
      import('../../src/stores/compare.js')
    ])

  built.push({ ipc, settings })

  return {
    ipc,
    emit: event.emit,
    listen: event.listen,
    nextTick: vue.nextTick,
    stores: {
      files,
      settings,
      tabs,
      tracker,
      projects,
      terminals,
      git,
      runs,
      attachments,
      notifications,
      updates,
      vcs,
      compare
    }
  }
}

/* Cancels the debounced write every graph of the finished test may still owe,
   and is called from the afterEach in setup.js — before clearMocks, so a write
   that does go out lands on that test's own mock and not on the next test's.

   What is being cancelled, and why it reaches the next test at all.
   `vi.resetModules()` empties the module registry; it does not stop a timer
   that is already running. A test that changes a setting after loadSettings has
   installed the watcher leaves settings.js holding a `setTimeout(flush, 400)`
   on the real clock — `loadProjectLayout` in "a project absent from the map…"
   is one such, and its layout is still the untouched default. The graph is
   thrown away a moment later; the timer is not. Its closure holds that graph's
   own `settings`, but `invoke` reads `window.__TAURI_INTERNALS__` at call time,
   and that is the one thing every graph shares — whichever mock installIpc put
   there last. On an idle machine the 400 ms expire inside the test that started
   them and the write is recorded where nobody minds. Under load the timer
   arrives several tests late, and its `settings_save` is recorded as if the
   test now running had made it: that is the `false/false` write that appeared
   in "two writes never overlap", and the write already standing at the first
   assertion of "a stream of edits collapses into one write".

   flushPending is the only way in from outside to clear that timer, so the
   write is made rather than dropped — it costs one call on a recorder nothing
   reads again. The handler is registered on every graph before any of them is
   flushed because the write goes wherever the transport points, which is the
   last graph's mock: a command no test registered would otherwise print an
   error about a write no test asked for. And the wait is capped, because
   `chain` may still hold a write a test left deliberately unanswered ("the
   window closes even when the write does not answer") — the cap costs nothing,
   since flushPending clears the timer after awaiting nextTick, which is
   microtasks only, and the cap is a timer. */
export async function settleStores() {
  const graphs = built.splice(0)
  for (const graph of graphs) graph.ipc.on('settings_save', null)
  for (const graph of graphs) {
    await Promise.race([
      graph.settings.flushPending(),
      new Promise((resolve) => setTimeout(resolve, 0))
    ])
  }
}
