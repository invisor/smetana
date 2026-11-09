import { describe, expect, it } from 'vitest'
import { loadStores } from '../support/stores.js'

describe('project-addressed Kanban sync state', () => {
  it('publishes this desktop’s open projects without changing shared settings', async () => {
    const { ipc } = await loadStores()
    const sync = await import('../../src/stores/kanbanSync.js')
    ipc.on('kanban_sync_presence', null)
    await sync.publishOpenProjects(['/a', '/b'])
    expect(ipc.calls('kanban_sync_presence')).toEqual([{ projects: ['/a', '/b'] }])
    await sync.publishOpenProjects(['/b'])
    expect(ipc.calls('kanban_sync_presence').at(-1)).toEqual({ projects: ['/b'] })
  })

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
    expect(sync.syncProblems('/a')).toEqual([pullError])
    expect(sync.syncProblems('/b')).toEqual([pushError])
    await emit('kanban:sync', { project: '/canonical/a', pullError, pushError })
    expect(sync.syncProblems('/a')).toEqual([pullError, pushError])
    await emit('kanban:sync', { project: '/canonical/a', pullError: null, pushError: null })
    expect(sync.syncProblems('/a')).toEqual([])
    expect(sync.syncProblems('/b')).toEqual([pushError])
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
    expect(sync.syncProblems('/a')).toEqual([error])
  })

  it('lets an event for B arrive while a fresh read for A is pending', async () => {
    const { ipc, emit } = await loadStores()
    const sync = await import('../../src/stores/kanbanSync.js')
    await sync.initKanbanSync()
    const error = { operation: 'pull', code: 7, timedOut: false, stderr: 'offline' }
    ipc.on('kanban_sync_state', { project: '/canonical/a', pullError: error, pushError: null })
    await sync.readKanbanSync('/a')
    expect(sync.syncProblems('/a')).toEqual([error])
    ipc.on('kanban_sync_state', async () => {
      await emit('kanban:sync', { project: '/canonical/b', pullError: error, pushError: null })
      return { project: '/canonical/a', pullError: null, pushError: null }
    })
    await sync.readKanbanSync('/a')
    expect(sync.syncProblems('/a')).toEqual([])
    expect(sync.syncProblems('/canonical/b')).toEqual([error])
  })
})
