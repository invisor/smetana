import { afterEach, vi } from 'vitest'
import { clearMocks } from '@tauri-apps/api/mocks'

afterEach(() => {
  vi.useRealTimers()
  clearMocks()
})
