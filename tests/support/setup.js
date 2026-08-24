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

   Which is all the ordering settleStores needs, and it is worth being exact
   about how little that is. It has to come before clearMocks, which is the line
   below it in this same hook, and before the next test's installIpc, which is
   the next beforeEach. Nothing else.

   In particular, this hook runs **last**, not first. A setup file registers its
   afterEach before a test file's, and vitest's default `sequence.hooks` is
   `stack` (`vitest/dist/chunks/coverage.*.js`), under which the runner reverses
   the after hooks and takes the innermost suite first
   (`@vitest/runner/dist/chunk-artifact.js`). Run in this tree, a describe-level
   afterEach went first, a test file's top-level afterEach second, and this one
   third. So the guarantee the other way round is **not** available: a test file
   cannot arrange its own afterEach to run after settleStores, and one that
   reads `ipc.calls('settings_save')` will see the write settleStores makes.
   Wanting that would mean moving this hook, not reasoning about it. */
afterEach(async () => {
  vi.useRealTimers()
  await settleStores()
  clearMocks()
})
