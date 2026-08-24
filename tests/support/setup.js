import { afterEach, vi } from 'vitest'
import { clearMocks } from '@tauri-apps/api/mocks'
import { settleStores } from './stores.js'

/* Tauri's internals live on window and only fail to leak between files because
   they are torn down. We restore real timers: otherwise a test that forgot to
   do so would break a neighbouring file.

   The three steps are in this order and each one needs the ones before it.
   Real timers first, because settleStores waits on a timer of its own and a
   test that left fake ones installed would have it never fire. settleStores
   second, because what it cancels is a real-clock timer holding a write, and
   the write it lets out has to go through the mock this test installed — after
   clearMocks there would be nothing there, and after the next installIpc it
   would be recorded against a test that never made it. clearMocks last, which
   is where it always was. settleStores itself carries the whole account of what
   that pending write is and how it used to arrive in somebody else's test.

   This is a setup file, so it registers its afterEach before any imported
   helper does, and vitest runs afterEach hooks in the order they were
   registered: this one runs first, which is what makes "before clearMocks" also
   mean "before anything a test file's own helpers do". */
afterEach(async () => {
  vi.useRealTimers()
  await settleStores()
  clearMocks()
})
