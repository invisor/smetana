import { describe, expect, it } from 'vitest'
import { loadStores } from './stores.js'
import { issue, snapshot } from './fixtures.js'

describe('оснастка', () => {
  it('happy-dom даёт window.crypto.getRandomValues, без которого mockIPC не встаёт', () => {
    expect(typeof window.crypto.getRandomValues).toBe('function')
  })

  it('каждый вызов loadStores даёт свежее состояние стора', async () => {
    const first = await loadStores()
    first.stores.files.filesState.root = '/один'

    const second = await loadStores()
    expect(second.stores.files.filesState.root).toBe(null)
  })

  it('незаведённая команда падает с её именем', async () => {
    const { stores } = await loadStores()
    stores.files.setRoot('/проект')

    await expect(stores.files.readFile('a.txt')).rejects.toMatchObject({
      kind: 'io',
      message: expect.stringContaining('files_read')
    })
  })

  it('ipc.calls отдаёт аргументы состоявшихся вызовов', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('files_read', () => ({ path: 'a.txt', text: 'привет', mtime: 7 }))
    stores.files.setRoot('/проект')

    await stores.files.readFile('a.txt')

    expect(ipc.calls('files_read')).toEqual([{ root: '/проект', path: 'a.txt' }])
  })

  it('дельта доезжает настоящим emit через plugin:event', async () => {
    const { ipc, emit, stores } = await loadStores()
    ipc.on('tracker_health', { state: 'ok' })
    ipc.on('tracker_snapshot', snapshot({ generation: 5, issues: [issue()] }))

    await stores.tracker.initTracker()
    expect(stores.tracker.trackerState.generation).toBe(5)

    await emit('tracker:delta', {
      generation: 6,
      upserted: [issue({ id: 'bd-2', title: 'вторая' })],
      removed: []
    })

    expect(stores.tracker.trackerState.issues.get('bd-2').title).toBe('вторая')
    expect(stores.tracker.trackerState.generation).toBe(6)
  })
})
