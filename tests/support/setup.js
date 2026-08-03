import { afterEach, vi } from 'vitest'
import { clearMocks } from '@tauri-apps/api/mocks'

/* Tauri's internals live on window and only fail to leak between files because
   they are torn down. We restore real timers: otherwise a test that forgot to
   do so would break a neighbouring file. */
afterEach(() => {
  clearMocks()
  vi.useRealTimers()
})
