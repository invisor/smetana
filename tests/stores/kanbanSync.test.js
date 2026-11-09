import { describe, expect, it } from 'vitest'
import { loadStores } from '../support/stores.js'

describe('project-addressed Kanban sync state', () => {
  it('keeps two projects separate and clears only the successful operation', async () => {
    const { ipc, emit } = await loadStores()
    const sync = await import('../../src/stores/kanbanSync.js')
    ipc.on('kanban_sync_state', ({ project }) => ({ project: `/canonical${project}`, pullError: null, pushError: null }))
    await sync.initKanbanSync()
    await sync.readKanbanSync('/a')
    await sync.readKanbanSync('/b')
    const pullError = { operation: 'pull', code: 7, timedOut: false, stderr: 'offline' }
    const pushError = { operation: 'push', code: 8, timedOut: false, stderr: 'rejected' }
    await emit('kanban:sync', { project: '/canonical/a', pullError, pushError: null })
    await emit('kanban:sync', { project: '/canonical/b', pullError: null, pushError })
    expect(sync.syncProblem('/a')).toEqual(pullError)
    expect(sync.syncProblem('/b')).toEqual(pushError)
    await emit('kanban:sync', { project: '/canonical/a', pullError: null, pushError: null })
    expect(sync.syncProblem('/a')).toBeNull()
    expect(sync.syncProblem('/b')).toEqual(pushError)
  })

  it('keeps a newer event when an older state read finishes later', async () => {
    const { ipc, emit } = await loadStores()
    const sync = await import('../../src/stores/kanbanSync.js')
    await sync.initKanbanSync()
    const error = { operation: 'pull', code: 7, timedOut: false, stderr: 'offline' }
    ipc.on('kanban_sync_state', async () => {
      await emit('kanban:sync', { project: '/canonical/a', pullError: error, pushError: null })
      return { project: '/canonical/a', pullError: null, pushError: null }
    })
    await sync.readKanbanSync('/a')
    expect(sync.syncProblem('/a')).toEqual(error)
  })
})
