import { describe, expect, it } from 'vitest'
import { loadStores } from './stores.js'
import { issue, snapshot } from './fixtures.js'

describe('the harness', () => {
  it('happy-dom provides window.crypto.getRandomValues, without which mockIPC does not come up', () => {
    expect(typeof window.crypto.getRandomValues).toBe('function')
  })

  it('every loadStores call gives fresh store state', async () => {
    const first = await loadStores()
    first.stores.files.filesState.root = '/one'

    const second = await loadStores()
    expect(second.stores.files.filesState.root).toBe(null)
  })

  it('an unregistered command fails with its name', async () => {
    const { stores } = await loadStores()
    stores.files.setRoot('/project')

    await expect(stores.files.readFile('a.txt')).rejects.toMatchObject({
      kind: 'io',
      message: expect.stringContaining('files_read')
    })
  })

  it('ipc.calls returns the arguments of the calls that happened', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('files_read', () => ({ path: 'a.txt', text: 'hello', mtime: 7 }))
    stores.files.setRoot('/project')

    await stores.files.readFile('a.txt')

    expect(ipc.calls('files_read')).toEqual([{ root: '/project', path: 'a.txt' }])
  })

  it('a delta arrives through a real emit over plugin:event', async () => {
    const { ipc, emit, stores } = await loadStores()
    ipc.on('tracker_health', { state: 'ok' })
    ipc.on('tracker_snapshot', snapshot({ generation: 5, issues: [issue()] }))

    await stores.tracker.initTracker()
    expect(stores.tracker.trackerState.generation).toBe(5)

    await emit('tracker:delta', {
      generation: 6,
      upserted: [issue({ id: 'bd-2', title: 'the second one' })],
      removed: []
    })

    expect(stores.tracker.trackerState.issues.get('bd-2').title).toBe('the second one')
    expect(stores.tracker.trackerState.generation).toBe(6)
  })
})
