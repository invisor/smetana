import { mockIPC, mockWindows } from '@tauri-apps/api/mocks'

/* Маршрутизатор поверх официального mockIPC. Существует ради читаемости: без
   него каждый тест писал бы свой switch по имени команды.

   mockWindows обязателен и стоит здесь, а не в тестах. Без него
   getCurrentWindow() бросает, а settings.js трактует этот бросок как «мы в
   браузере, окна нет» и молча не подписывается на закрытие — тесты закрытия
   проверяли бы пустоту. */
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
    commands() {
      return recorded.map((call) => call.cmd)
    }
  }

  mockWindows('main')
  mockIPC(
    (cmd, args) => {
      recorded.push({ cmd, args })
      const handler = handlers.get(cmd)
      /* Команда, о которой тест не думал, обязана падать с именем команды.
         Молчаливый undefined развалил бы тест тремя строками ниже, и виноватым
         выглядел бы стор. */
      if (!handler) throw new Error(`[тест] команда ${cmd} не заведена: добавьте ipc.on('${cmd}', …)`)
      return handler(args)
    },
    { shouldMockEvents: true }
  )

  return ipc
}
