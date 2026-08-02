import { afterEach, vi } from 'vitest'
import { clearMocks } from '@tauri-apps/api/mocks'

/* Внутренности Tauri живут в window и между файлами не текут только потому,
   что их снимают. Таймеры возвращаем настоящими: тест, забывший это сделать,
   иначе развалил бы соседний файл. */
afterEach(() => {
  clearMocks()
  vi.useRealTimers()
})
