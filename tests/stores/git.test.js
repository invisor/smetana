import { describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

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
