import { describe, expect, it } from 'vitest'
import { loadStores } from '../support/stores.js'

/* One comparison and one file, answered by the transport. The shas are the only
   thing that matters about them: every read afterwards has to go by these and
   never by the branch name. */
const LEFT = '1111111111111111111111111111111111111111'
const RIGHT = '2222222222222222222222222222222222222222'

const COMPARISON = {
  left: LEFT,
  right: RIGHT,
  files: [
    { path: 'src/a.js', origPath: null, kind: 'modified' },
    { path: 'src/added.js', origPath: null, kind: 'added' }
  ]
}

/* A fresh graph with both of this window's commands answered. A file comes back
   as `<rev>:<path>`, so an assertion can say which revision the text on screen
   was read at — the whole subject of this file. `src/added.js` has nothing on
   the left, which is the second of `vcs_file_at_rev`'s two answers and not a
   failure. */
async function loadCompare() {
  const { stores, ipc } = await loadStores()
  ipc.on('vcs_compare', COMPARISON)
  ipc.on('vcs_file_at_rev', (args) =>
    args.path === 'src/added.js' && args.rev === LEFT ? null : `${args.rev}:${args.path}`
  )
  return { compare: stores.compare, ipc }
}

describe('the branch comparison', () => {
  it('reads a file at the two revisions the comparison resolved', async () => {
    const { compare, ipc } = await loadCompare()
    await compare.aim('/tmp/r', 'feature')
    await compare.select('src/a.js')

    const reads = ipc.calls('vcs_file_at_rev')
    expect(reads.map((args) => args.rev)).toEqual([LEFT, RIGHT])
    expect(reads.every((args) => args.repo === '/tmp/r')).toBe(true)
    expect(compare.compareState.head).toBe(`${LEFT}:src/a.js`)
    expect(compare.compareState.work).toBe(`${RIGHT}:src/a.js`)
  })

  /* The whole reason the shas travel back from Rust rather than the names: a
     branch name asked twice can answer from two different commits. */
  it('never asks for a file by branch name', async () => {
    const { compare, ipc } = await loadCompare()
    await compare.aim('/tmp/r', 'feature')
    await compare.select('src/a.js')

    const revs = ipc.calls('vcs_file_at_rev').map((args) => args.rev)
    /* A read that never happened would satisfy the two refusals below without
       saying anything, so the count comes first. */
    expect(revs).toHaveLength(2)
    expect(revs).not.toContain('feature')
    expect(revs).not.toContain('HEAD')
  })

  /* The guard `loadDiff` keeps in `stores/tabs.js`, here for the same reason:
     without it the last response wins rather than the last call, and one file's
     text lands under another file's name. */
  it('does not let a slow read land under the file picked after it', async () => {
    const { compare, ipc } = await loadCompare()
    const gate = []
    ipc.on(
      'vcs_file_at_rev',
      (args) =>
        new Promise((resolve) => {
          gate.push(() => resolve(`${args.rev}:${args.path}`))
        })
    )
    await compare.aim('/tmp/r', 'feature')

    const first = compare.select('src/a.js')
    const second = compare.select('src/added.js')
    /* The first call's two reads answer last. */
    gate.slice(2).forEach((release) => release())
    gate.slice(0, 2).forEach((release) => release())
    await Promise.all([first, second])

    expect(compare.compareState.selected).toBe('src/added.js')
    expect(compare.compareState.work).toBe(`${RIGHT}:src/added.js`)
  })

  it('re-runs the comparison when the mode changes', async () => {
    const { compare, ipc } = await loadCompare()
    await compare.aim('/tmp/r', 'feature')
    await compare.setMode('direct')

    expect(ipc.calls('vcs_compare').at(-1).mode).toBe('direct')
    expect(compare.compareState.mode).toBe('direct')
  })

  it('opens on the diverged mode', async () => {
    const { compare, ipc } = await loadCompare()
    await compare.aim('/tmp/r', 'feature')
    expect(ipc.calls('vcs_compare')[0].mode).toBe('diverged')
  })

  /* `null` is a revision that does not have the file, which is exactly a file
     added on the other side. The empty pane is the truth, and the flag is what
     lets the caption say which of the two empties it is. */
  it('says an added file is missing on the left rather than failing', async () => {
    const { compare } = await loadCompare()
    await compare.aim('/tmp/r', 'feature')
    await compare.select('src/added.js')

    expect(compare.compareState.missingLeft).toBe(true)
    expect(compare.compareState.head).toBe('')
    expect(compare.compareState.fileError).toBe(null)
  })

  it("carries a refusal in Rust's own shape and wraps anything else", async () => {
    const { compare, ipc } = await loadCompare()
    await compare.aim('/tmp/r', 'feature')

    ipc.fail('vcs_file_at_rev', { kind: 'binary', message: 'binary file: src/a.js' })
    await compare.select('src/a.js')
    expect(compare.compareState.fileError).toEqual({
      kind: 'binary',
      message: 'binary file: src/a.js'
    })

    ipc.fail('vcs_file_at_rev', new Error('the channel went away'))
    await compare.select('src/a.js')
    expect(compare.compareState.fileError.kind).toBe('io')
  })

  /* Two branches with nothing between them is an ordinary answer, and the list
     being empty is not the same state as a refusal. */
  it('keeps an empty comparison apart from a failed one', async () => {
    const { compare, ipc } = await loadCompare()
    ipc.on('vcs_compare', { left: LEFT, right: RIGHT, files: [] })
    await compare.aim('/tmp/r', 'feature')

    expect(compare.compareState.files).toEqual([])
    expect(compare.compareState.error).toBe(null)
  })

  it("carries the comparison's own refusal", async () => {
    const { compare, ipc } = await loadCompare()
    ipc.fail('vcs_compare', {
      kind: 'unrelated',
      message: 'These two branches share no history.'
    })
    await compare.aim('/tmp/r', 'feature')

    expect(compare.compareState.error.kind).toBe('unrelated')
    expect(compare.compareState.files).toEqual([])
  })

  /* Right-clicking the branch already on screen is the ordinary way to ask for
     the list again. An open window is focused rather than rebuilt precisely so
     that the file somebody is in the middle of reading survives it, so a
     re-aim that threw the selection away would defeat the reason it is not
     rebuilt. */
  it('keeps the file being read when it is re-aimed at the same pair', async () => {
    const { compare, ipc } = await loadCompare()
    await compare.aim('/tmp/r', 'feature')
    await compare.select('src/a.js')

    await compare.aim('/tmp/r', 'feature')

    expect(compare.compareState.selected).toBe('src/a.js')
    expect(compare.compareState.head).toBe(`${LEFT}:src/a.js`)
    expect(compare.compareState.work).toBe(`${RIGHT}:src/a.js`)
    /* Still a read: the whole of what the gesture asks for is a fresh list. */
    expect(ipc.calls('vcs_compare')).toHaveLength(2)
    /* And the open file is read again at the shas that fresh comparison
       resolved, which is this store's central rule and the half the three
       assertions above cannot see: the fixture answers the same text every
       time and `head`/`work` already held it, so a re-aim that kept the
       selection and never called `select` would satisfy every one of them.
       Two reads per `select`, one per side, and `select` has run twice. */
    expect(ipc.calls('vcs_file_at_rev')).toHaveLength(4)
  })

  /* A different pair is a different question, and the file open on the old one
     has no meaning under it. */
  it('drops the file being read when it is re-aimed at another pair', async () => {
    const { compare, ipc } = await loadCompare()
    await compare.aim('/tmp/r', 'feature')
    await compare.select('src/a.js')

    await compare.aim('/tmp/r', 'other')

    expect(compare.compareState.selected).toBe(null)
    expect(compare.compareState.head).toBe('')
    expect(compare.compareState.work).toBe('')
    expect(ipc.calls('vcs_compare').at(-1).branch).toBe('other')
  })

  /* The window's header is drawn from `branch` and its rows from `files`, and
     `aim` writes the first before git has been asked for the second. The whole
     of the defect is the moment in between, so the comparison is held open
     here rather than answered. */
  it('holds no list from the old pair while the new one is being compared', async () => {
    const { compare, ipc } = await loadCompare()
    await compare.aim('/tmp/r', 'feature')
    await compare.select('src/a.js')

    let release
    ipc.on(
      'vcs_compare',
      () =>
        new Promise((resolve) => {
          release = () => resolve(COMPARISON)
        })
    )
    const aimed = compare.aim('/tmp/r', 'other')

    expect(compare.compareState.branch).toBe('other')
    expect(compare.compareState.files).toEqual([])
    expect(compare.compareState.left).toBe('')
    expect(compare.compareState.right).toBe('')

    /* And the answer still lands, so the assertions above are about the wait
       rather than about a comparison that never happened. */
    release()
    await aimed
    expect(compare.compareState.files).toEqual(COMPARISON.files)
    expect(compare.compareState.left).toBe(LEFT)
  })

  /* The same moment, for the refusal rather than for the rows: a sentence about
     the pair git would not compare, still in the panel while the header already
     names the pair it has not been asked about yet. */
  it('holds no refusal from the old pair while the new one is being compared', async () => {
    const { compare, ipc } = await loadCompare()
    ipc.fail('vcs_compare', {
      kind: 'unrelated',
      message: 'These two branches share no history.'
    })
    await compare.aim('/tmp/r', 'feature')
    expect(compare.compareState.error.kind).toBe('unrelated')

    let release
    ipc.on(
      'vcs_compare',
      () =>
        new Promise((resolve) => {
          release = () => resolve(COMPARISON)
        })
    )
    const aimed = compare.aim('/tmp/r', 'other')

    expect(compare.compareState.branch).toBe('other')
    expect(compare.compareState.error).toBe(null)

    /* And the answer still lands, so the assertion above is about the wait
       rather than about a comparison that never happened. */
    release()
    await aimed
    expect(compare.compareState.error).toBe(null)
    expect(compare.compareState.files).toEqual(COMPARISON.files)
  })
})
