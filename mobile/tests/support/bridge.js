import { vi } from 'vitest'
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks'

/* The phone's end of the transport as a test sees it: the Rust core is replaced
   by a recorder behind the official `mockIPC`, which is the only thing mocked.
   Everything above it — the swapped imports, the stores, the bridge modules —
   is the real code. */
export function installCore(answers = {}) {
  const calls = []
  mockWindows('main')
  mockIPC(
    (cmd, args) => {
      calls.push({ cmd, args })
      if (cmd === 'bridge_invoke') {
        const handler = answers[args.cmd]
        if (!handler) throw `forbidden`
        return typeof handler === 'function' ? handler(args.args) : handler
      }
      if (cmd in answers) {
        const handler = answers[cmd]
        return typeof handler === 'function' ? handler(args) : handler
      }
      return null
    },
    { shouldMockEvents: true }
  )
  return {
    calls,
    bridged: (cmd) => calls.filter((c) => c.cmd === 'bridge_invoke' && c.args.cmd === cmd).map((c) => c.args.args),
    core: (cmd) => calls.filter((c) => c.cmd === cmd).map((c) => c.args)
  }
}

/* A fresh module graph, with the real `emit`/`listen` of the same graph, for
   the reason tests/support/stores.js in the desktop tests gives: resetModules
   recreates `vue` and the Tauri event module too. */
export async function loadShell() {
  vi.resetModules()
  const [event, vue, tracker, status] = await Promise.all([
    import('@tauri-apps/api/event'),
    import('vue'),
    import('@src/stores/tracker.js'),
    import('../../src/bridge/status.js')
  ])
  return { emit: event.emit, nextTick: vue.nextTick, tracker, status }
}

export const snapshot = (generation = 1, ids = ['smetana-1']) => ({
  generation,
  columns: [
    { name: 'open', category: 'active' },
    { name: 'closed', category: 'done' }
  ],
  issues: ids.map((id) => ({
    id,
    title: `Task ${id}`,
    status: 'open',
    updated_at: '2026-10-15T10:00:00Z',
    labels: [],
    dependencies: []
  }))
})
