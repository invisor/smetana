import { describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

/* What the store holds: a branch and the repositories that do not have it. A
   single-repository project is short of nothing, which is every fixture here. */
const everywhere = (...names) => names.map((name) => ({ name, missing_in: [] }))

describe('the active project\'s branch', () => {
  it('the branch is read and lands in the store', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('git_head', { branch: 'feat/worktree-rename', detached: null })

    await stores.git.loadHead('/p')

    expect(stores.git.gitState.branch).toBe('feat/worktree-rename')
    expect(stores.git.gitState.detached).toBe(null)
    expect(ipc.calls('git_head')).toEqual([{ project: '/p' }])
  })

  it('a detached HEAD does not pass itself off as a branch', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('git_head', { branch: null, detached: '9a1b2c3' })

    await stores.git.loadHead('/p')

    expect(stores.git.gitState.branch).toBe(null)
    expect(stores.git.gitState.detached).toBe('9a1b2c3')
  })

  it('with no project there is no branch and nothing to ask about', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('git_head', { branch: 'main', detached: null })
    await stores.git.loadHead('/p')

    await stores.git.loadHead(null)

    expect(stores.git.gitState.branch).toBe(null)
    // There is no second call: there is nothing to ask a branch of.
    expect(ipc.calls('git_head')).toEqual([{ project: '/p' }])
  })

  it('a failed command clears the branch instead of leaving somebody else\'s', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('git_head', { branch: 'main', detached: null })
    await stores.git.loadHead('/p')

    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('git_head', 'it broke')
    await stores.git.loadHead('/q')

    expect(stores.git.gitState.branch).toBe(null)
  })

  /* The same guard as loadSessions has: the last call wins, not the last
     answer. Otherwise the scope bar would name one project's branch under
     another project's name. */
  it('a stale answer does not overwrite the new project\'s branch', async () => {
    const { stores } = await loadStores()
    const pending = new Map()
    const { mockIPC } = await import('@tauri-apps/api/mocks')
    mockIPC((cmd, args) => new Promise((resolve) => pending.set(args.project, resolve)))

    const slow = stores.git.loadHead('/old')
    const fast = stores.git.loadHead('/new')

    pending.get('/new')({ branch: 'new-branch', detached: null })
    await fast
    pending.get('/old')({ branch: 'old-branch', detached: null })
    await slow

    expect(stores.git.gitState.branch).toBe('new-branch')
  })
})

describe('the branches offered to the run dialog', () => {
  it('the list is read and lands in the store', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('target_branches', everywhere('staging', 'main'))

    await stores.git.loadBranches('/p')

    expect(stores.git.gitState.branches).toEqual(everywhere('staging', 'main'))
  })

  /* Half of smetana-6gs: the dialog opens and asks for the branches in the same
     breath, so emptying this project's list to go and read it again leaves the
     field with nothing to fill from at the moment it fills. */
  it('this project\'s branches stay on screen while they are read again', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('target_branches', everywhere('staging', 'main'))
    await stores.git.loadBranches('/p')

    let answer
    ipc.on('target_branches', () => new Promise((resolve) => (answer = resolve)))
    const again = stores.git.loadBranches('/p')

    expect(stores.git.gitState.branches).toEqual(everywhere('staging', 'main'))
    answer(everywhere('main', 'staging'))
    await again
    expect(stores.git.gitState.branches).toEqual(everywhere('main', 'staging'))
  })

  /* The other side of it, and the reason the clearing was there: a branch of a
     repository somebody has left must never be an option in front of them. */
  it('another project\'s branches go the moment this one is asked about', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('target_branches', everywhere('staging', 'main'))
    await stores.git.loadBranches('/p')

    ipc.on('target_branches', () => new Promise(() => {}))
    stores.git.loadBranches('/q')

    expect(stores.git.gitState.branches).toEqual([])
  })

  it('with no project there is nothing to offer and nothing to ask', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('target_branches', everywhere('main'))
    await stores.git.loadBranches('/p')

    await stores.git.loadBranches(null)

    expect(stores.git.gitState.branches).toEqual([])
    expect(ipc.calls('target_branches')).toEqual([{ project: '/p' }])
  })

  it('a failed listing leaves nothing rather than a list nobody confirmed', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('target_branches', everywhere('main'))
    await stores.git.loadBranches('/p')

    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('target_branches', 'it broke')
    await stores.git.loadBranches('/p')

    expect(stores.git.gitState.branches).toEqual([])
  })

  it('a stale answer does not overwrite the new project\'s branches', async () => {
    const { stores } = await loadStores()
    const pending = new Map()
    const { mockIPC } = await import('@tauri-apps/api/mocks')
    mockIPC((cmd, args) => new Promise((resolve) => pending.set(args.project, resolve)))

    const slow = stores.git.loadBranches('/old')
    const fast = stores.git.loadBranches('/new')

    pending.get('/new')(everywhere('new-main'))
    await fast
    pending.get('/old')(everywhere('old-main'))
    await slow

    expect(stores.git.gitState.branches).toEqual(everywhere('new-main'))
  })

  /* The multi-repository half, and what the field draws the lower group from.
     The store keeps the record whole rather than flattening it to a name, the
     same instinct `runs.js` follows with a `Run`: a fact this front end has not
     learned to draw must not be silently thrown away on the way in. */
  it('keeps which repositories a branch is missing from', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('target_branches', [
      { name: 'develop', missing_in: [] },
      { name: 'release/7', missing_in: ['admin', 'extension'] }
    ])

    await stores.git.loadBranches('/p')

    expect(stores.git.gitState.branches[1].missing_in).toEqual(['admin', 'extension'])
  })
})
