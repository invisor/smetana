import { beforeEach, describe, expect, it, vi } from 'vitest'
import { effectScope } from 'vue'
import { useCopyFeedback } from '../../../src/components/core/copyFeedback.js'
import { COPIED_MS } from '../../../src/components/kanban/copyId.js'

/* The copy-confirmation policy, which until now was in no runner's reach at
   all: it was written out four times over inside two `.vue` files, and a `.vue`
   file is the one thing no test in this repository can see. The pair that drew
   a task's id is in this project's hazards list as having already cost once —
   a stranded reset timer sat in both copies, a copy that had worked showed no
   confirmation, and the fix had to be made twice by hand. Every `it` below is
   one of the steps that were duplicated. */

/* A scope, because the composable clears its pending timer on
   `onScopeDispose`, and outside one that call has nothing to register on. It is
   also what the last test here disposes on purpose. */
const mount = (write) => {
  const scope = effectScope()
  return { scope, feedback: scope.run(() => useCopyFeedback(write)) }
}

/* A write whose answer is handed over by the test rather than by a promise
   already resolved: which press owns the confirmation is decided in the window
   between the press and the clipboard answering, so that window has to be
   somewhere a test can stand. */
const deferred = () => {
  let settle
  const promise = new Promise((resolve) => {
    settle = resolve
  })
  return { promise, settle }
}

const ok = () => Promise.resolve(true)

beforeEach(() => {
  vi.useFakeTimers()
})

describe('what a copy says, and on which control', () => {
  it('says nothing at all before anything has been asked', () => {
    const { feedback } = mount(ok)
    expect(feedback.stateFor('bd-a1b2')).toBe('')
    expect(feedback.nounFor('bd-a1b2')).toBe('')
  })

  it('confirms a copy that worked, on that row and on no other', async () => {
    const write = vi.fn(ok)
    const { feedback } = mount(write)
    await feedback.copy('bd-a1b2', 'bd-a1b2')
    expect(write).toHaveBeenCalledWith('bd-a1b2')
    expect(feedback.stateFor('bd-a1b2')).toBe('copied')
    expect(feedback.stateFor('bd-3c9d')).toBe('')
  })

  /* A refusal lands on the same control as a confirmation, which is the whole
     of why this feature has one channel and not a toast beside it. */
  it('says so on the same control when the clipboard refused', async () => {
    const { feedback } = mount(() => Promise.resolve(false))
    await feedback.copy('bd-a1b2', 'bd-a1b2')
    expect(feedback.stateFor('bd-a1b2')).toBe('failed')
  })

  /* Nothing to copy is a refusal rather than a copy of the empty string: a
     clipboard emptied by a press is worse than one left alone. Three of the
     session menu's verbs can produce nothing at all, which is where this
     branch is reached. */
  it('refuses rather than emptying the clipboard when there is nothing to copy', async () => {
    const write = vi.fn(ok)
    const { feedback } = mount(write)
    await feedback.copy('9f1c', '')
    expect(write).not.toHaveBeenCalled()
    expect(feedback.stateFor('9f1c')).toBe('failed')
  })

  /* The noun is the difference between the two callers: a task's id is a
     control of its own and has nothing to name, while three different verbs
     land on one session's menu button and "Copied" alone would leave a person
     unsure which of them they pressed. */
  it('carries the noun of what was copied, and only for that row', async () => {
    const { feedback } = mount(ok)
    await feedback.copy('9f1c', 'claude --resume 9f1c', 'resume command')
    expect(feedback.nounFor('9f1c')).toBe('resume command')
    expect(feedback.nounFor('0a2e')).toBe('')
  })

  /* The `id != null` guard, and it is not decoration: rows are asked one by
     one while the state is back at null between copies, and without it every
     one of them would answer with the last outcome. */
  it('answers nothing for a row with no id, including while nothing is claimed', async () => {
    const { feedback } = mount(ok)
    expect(feedback.stateFor(null)).toBe('')
    expect(feedback.stateFor(undefined)).toBe('')
    await feedback.copy(null, 'a path with no session behind it')
    expect(feedback.stateFor(null)).toBe('')
    expect(feedback.nounFor(null)).toBe('')
  })

  /* One target at a time: two controls both reading `Copied` would be a claim
     about a clipboard that holds one thing. */
  it('takes the confirmation off the previous row as soon as the next is pressed', async () => {
    const { feedback } = mount(ok)
    await feedback.copy('bd-a1b2', 'bd-a1b2')
    await feedback.copy('bd-3c9d', 'bd-3c9d')
    expect(feedback.stateFor('bd-a1b2')).toBe('')
    expect(feedback.stateFor('bd-3c9d')).toBe('copied')
  })

  /* And it comes off before the second write answers, not after: in the app
     that write takes a moment, and a first row still saying `Copied` over a
     clipboard that now holds something else is the one claim this must never
     make. */
  it('blanks the outcome while the next write is still in flight', async () => {
    const slow = deferred()
    const answers = [Promise.resolve(true), slow.promise]
    const { feedback } = mount(() => answers.shift())

    await feedback.copy('bd-a1b2', 'bd-a1b2')
    expect(feedback.stateFor('bd-a1b2')).toBe('copied')

    const pending = feedback.copy('bd-3c9d', 'bd-3c9d')
    expect(feedback.stateFor('bd-a1b2')).toBe('')
    expect(feedback.stateFor('bd-3c9d')).toBe('')

    slow.settle(true)
    await pending
    expect(feedback.stateFor('bd-3c9d')).toBe('copied')
  })
})

describe('how long it stands, and which press owns it', () => {
  it('holds for the one duration the whole app confirms a copy for', async () => {
    const { feedback } = mount(ok)
    await feedback.copy('bd-a1b2', 'bd-a1b2')
    vi.advanceTimersByTime(COPIED_MS - 1)
    expect(feedback.stateFor('bd-a1b2')).toBe('copied')
    vi.advanceTimersByTime(1)
    expect(feedback.stateFor('bd-a1b2')).toBe('')
    expect(feedback.nounFor('bd-a1b2')).toBe('')
  })

  /* The guard after the await. Two rows pressed in turn, the first clipboard
     write answering last: the answer that arrives out of order must not paint
     itself over the row somebody is actually looking at. */
  it('lets a later press take the state over from an earlier one', async () => {
    const first = deferred()
    const second = deferred()
    const answers = [first.promise, second.promise]
    const { feedback } = mount(() => answers.shift())

    const a = feedback.copy('bd-a1b2', 'bd-a1b2')
    const b = feedback.copy('bd-3c9d', 'bd-3c9d')

    first.settle(true)
    await a
    expect(feedback.stateFor('bd-a1b2')).toBe('')

    second.settle(true)
    await b
    expect(feedback.stateFor('bd-3c9d')).toBe('copied')
    expect(feedback.stateFor('bd-a1b2')).toBe('')
  })

  /* The stranded timer, which is the defect this project has already paid for
     once and in two files. Two presses on the *same* row both get past the
     guard above, so without the second `clearTimeout` the first press's reset
     goes on running with nothing pointing at it and puts out the second press's
     confirmation early — and, since it also blanks the claimed row, leaves a
     copy that worked saying nothing at all. A double-click is the most ordinary
     way there is to point at a word somebody wants. */
  it('leaves no stranded reset behind a second press on the same row', async () => {
    const first = deferred()
    const second = deferred()
    const answers = [first.promise, second.promise]
    const { feedback } = mount(() => answers.shift())

    const a = feedback.copy('bd-a1b2', 'bd-a1b2')
    const b = feedback.copy('bd-a1b2', 'bd-a1b2')

    first.settle(true)
    await a
    // A moment passes, and then the second write answers too.
    vi.advanceTimersByTime(100)
    second.settle(true)
    await b

    // The first press's reset would have fired here. The second's must not.
    vi.advanceTimersByTime(COPIED_MS - 100)
    expect(feedback.stateFor('bd-a1b2')).toBe('copied')

    // It goes out a full duration after the press that actually won.
    vi.advanceTimersByTime(100)
    expect(feedback.stateFor('bd-a1b2')).toBe('')
  })

  /* What `onScopeDispose` is for: a window closed inside the 1.2 s would
     otherwise leave a timer running against refs nothing draws. */
  it('drops the pending reset when the scope around it goes away', async () => {
    const { scope, feedback } = mount(ok)
    await feedback.copy('bd-a1b2', 'bd-a1b2')
    expect(vi.getTimerCount()).toBe(1)
    scope.stop()
    expect(vi.getTimerCount()).toBe(0)
  })
})
