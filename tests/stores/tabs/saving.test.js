import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../../support/stores.js'
import { buffer, fileText } from '../../support/fixtures.js'

let ipc
let files
let tabs
let mtime

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  files = loaded.stores.files
  tabs = loaded.stores.tabs
  files.setRoot('/project')
  mtime = 10
  ipc.on('files_read', (args) => fileText({ path: args.path, mtime }))
  ipc.on('files_write', () => {
    mtime += 1
    return mtime
  })
})

const opened = async (path = 'a.txt') => {
  tabs.openFile(path, { permanent: true })
  await vi.waitFor(() => expect(tabs.buffers.get(path).loading).toBe(false))
}

describe('a successful save', () => {
  it('carries the timestamp received on read and clears the dirtiness', async () => {
    await opened()
    tabs.setText('a.txt', 'edited')

    await tabs.saveTab('a.txt')

    expect(ipc.calls('files_write')).toEqual([
      { root: '/project', path: 'a.txt', text: 'edited', expectedMtime: 10 }
    ])
    expect(tabs.isDirty('a.txt')).toBe(false)
    expect(tabs.buffers.get('a.txt').mtime).toBe(11)
  })

  it('a clean tab does not go to the disk', async () => {
    await opened()

    await tabs.saveTab('a.txt')

    expect(ipc.calls('files_write')).toHaveLength(0)
  })

  it('the second consecutive write takes the mtime the first moved, not the one captured at queue time', async () => {
    await opened()

    tabs.setText('a.txt', 'the first')
    const first = tabs.saveTab('a.txt')
    tabs.setText('a.txt', 'the second')
    const second = tabs.saveTab('a.txt')
    await Promise.all([first, second])

    const writes = ipc.calls('files_write')
    expect(writes).toHaveLength(2)
    expect(writes[0].expectedMtime).toBe(10)
    expect(writes[1].expectedMtime).toBe(11)
    expect(tabs.isDirty('a.txt')).toBe(false)
  })

  it('what was typed in flight stays dirty', async () => {
    await opened()
    tabs.setText('a.txt', 'the first edit')

    const pending = tabs.saveTab('a.txt')
    tabs.setText('a.txt', 'the first edit and more')
    await pending

    expect(tabs.buffers.get('a.txt').original).toBe('the first edit')
    expect(tabs.buffers.get('a.txt').text).toBe('the first edit and more')
    expect(tabs.isDirty('a.txt')).toBe(true)
  })

  it('a successful write clears a previous write refusal', async () => {
    await opened()
    tabs.buffers.set('a.txt', buffer({ text: 'an edit', saveError: { kind: 'denied' } }))

    await tabs.saveTab('a.txt')

    expect(tabs.buffers.get('a.txt').saveError).toBe(null)
  })
})

describe('refusals', () => {
  it('stale raises the strip without losing anything', async () => {
    await opened()
    tabs.setText('a.txt', 'my edit')
    ipc.fail('files_write', { kind: 'stale', message: 'the file moved' })

    await tabs.saveTab('a.txt')

    const current = tabs.buffers.get('a.txt')
    expect(current.stale).toBe(true)
    expect(current.text).toBe('my edit')
    expect(current.error).toBe(null)
    expect(current.saveError).toBe(null)
  })

  it('a write refusal does not lock the field: editing and saving again are allowed', async () => {
    await opened()
    tabs.setText('a.txt', 'my edit')
    ipc.fail('files_write', { kind: 'denied', message: 'no permission' })

    await tabs.saveTab('a.txt')

    const current = tabs.buffers.get('a.txt')
    expect(current.saveError).toEqual({ kind: 'denied', message: 'no permission' })
    expect(current.error).toBe(null)

    tabs.setText('a.txt', 'another edit')
    expect(tabs.buffers.get('a.txt').text).toBe('another edit')
  })

  it('a refusal does not stop the queue: the next write goes through', async () => {
    await opened()
    tabs.setText('a.txt', 'the first')
    ipc.fail('files_write', { kind: 'denied', message: 'no permission' })
    await tabs.saveTab('a.txt')

    ipc.on('files_write', () => 12)
    tabs.setText('a.txt', 'the second')
    await tabs.saveTab('a.txt')

    expect(tabs.buffers.get('a.txt').mtime).toBe(12)
    expect(tabs.isDirty('a.txt')).toBe(false)
  })
})

describe('what is never written', () => {
  it('a buffer whose first read has not returned', async () => {
    tabs.openFile('a.txt', { permanent: true })

    await tabs.saveTab('a.txt')

    expect(ipc.calls('files_write')).toHaveLength(0)
  })

  /* A buffer from openFile is not dirty yet: text === original === '', and
     saveTab returns on !isDirty rather than on buffer.loading — the guard is
     never reached. To catch the guard itself we need a buffer that is loading
     and dirty at the same time. Such a state cannot be produced through the
     public API (setText lets no edits in while loading is set) — it is
     synthetic, and the app never produces it. The test guards the check in
     saveTab itself, not a reachable state. */
  it('a buffer that is loading and dirty at once is not written — this guards the loading check itself', async () => {
    tabs.buffers.set(
      'a.txt',
      buffer({ loading: true, text: 'typed', original: 'original' })
    )

    await tabs.saveTab('a.txt')

    expect(ipc.calls('files_write')).toHaveLength(0)
  })

  it('a buffer with a read refusal', async () => {
    tabs.buffers.set('a.txt', buffer({ text: 'something', original: '', error: { kind: 'binary' } }))

    await tabs.saveTab('a.txt')

    expect(ipc.calls('files_write')).toHaveLength(0)
  })

  it('a tab that is not there', async () => {
    await tabs.saveTab('no-such-file.txt')

    expect(ipc.calls('files_write')).toHaveLength(0)
  })
})

describe('saveTabs', () => {
  it('writes only the dirty ones among those listed', async () => {
    await opened('a.txt')
    tabs.openFile('b.txt', { permanent: true })
    await vi.waitFor(() => expect(tabs.buffers.get('b.txt').loading).toBe(false))
    tabs.setText('b.txt', 'an edit')

    await tabs.saveTabs(['a.txt', 'b.txt'])

    expect(ipc.calls('files_write').map((call) => call.path)).toEqual(['b.txt'])
  })
})
