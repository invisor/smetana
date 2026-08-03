import { mockIPC, mockWindows } from '@tauri-apps/api/mocks'

/* A router on top of the official mockIPC. It exists for readability: without
   it every test would write its own switch on the command name.

   mockWindows is mandatory and belongs here rather than in the tests. Without
   it getCurrentWindow() throws, and settings.js reads that throw as "we are in
   a browser, there is no window" and silently never subscribes to the close —
   the close tests would be checking emptiness. */
export function installIpc() {
  const handlers = new Map()
  const recorded = []

  const ipc = {
    on(cmd, reply) {
      handlers.set(cmd, typeof reply === 'function' ? reply : () => reply)
      return ipc
    },
    fail(cmd, error) {
      handlers.set(cmd, () => {
        throw error
      })
      return ipc
    },
    calls(cmd) {
      return recorded.filter((call) => call.cmd === cmd).map((call) => call.args)
    },
    /* recorded is written before the handler is called, so commands()/calls()
       do not tell "the command ran" from "the command threw and the caller
       caught the refusal". A test of command ordering built on this list alone
       does not check that a command succeeded — see fixes 2 and 3
       (settings_save may not have happened at all, and indexOf would still give
       a comparable number: -1). */
    commands() {
      return recorded.map((call) => call.cmd)
    }
  }

  mockWindows('main')
  mockIPC(
    (cmd, args) => {
      recorded.push({ cmd, args })
      const handler = handlers.get(cmd)
      /* A command the test did not think about has to fail with the command's
         name. A silent undefined would break the test three lines later, and
         the store would look like the culprit. */
      if (!handler) throw new Error(`[test] command ${cmd} is not registered: add ipc.on('${cmd}', …)`)
      return handler(args)
    },
    { shouldMockEvents: true }
  )

  return ipc
}
