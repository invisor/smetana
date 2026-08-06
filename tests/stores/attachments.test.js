import { describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

/* What `attachment_import` and `attachment_write` answer with. The base64 is
   two bytes, and only its round trip into the thumbnail's URL is under test —
   what makes a PNG a PNG is Rust's business and is tested there. */
const stored = (name) => ({
  path: `/data/attachments/${name}`,
  name,
  bytes: 2,
  mime: 'image/png',
  data: 'AQI='
})

/* A stand-in for the File a paste produces. Only two things about it are read:
   the name, which a pasted screenshot does not have, and the bytes. */
const file = (name, bytes) => ({
  name,
  arrayBuffer: async () => new Uint8Array(bytes).buffer
})

describe('images attached to a task that has not been filed', () => {
  it('a file on disk is copied and comes back as a thumbnail', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('attachment_import', ({ path }) => stored(path.split('/').pop()))

    await stores.attachments.importPaths(['/Users/you/Downloads/mock.png'])

    expect(ipc.calls('attachment_import')).toEqual([{ path: '/Users/you/Downloads/mock.png' }])
    expect(stores.attachments.attachmentsState.items).toEqual([
      {
        path: '/data/attachments/mock.png',
        name: 'mock.png',
        bytes: 2,
        url: 'data:image/png;base64,AQI='
      }
    ])
  })

  it('a paste travels as base64 and keeps whatever name it had', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('attachment_write', () => stored('20260806-121314-image.png'))

    await stores.attachments.attachFiles([file('', [1, 2])])

    // No name: a screenshot on the clipboard has none, and Rust invents one.
    expect(ipc.calls('attachment_write')).toEqual([{ name: null, data: 'AQI=' }])
    expect(stores.attachments.attachmentsState.items).toHaveLength(1)
  })

  it('a refusal attaches nothing and keeps a line a person can act on', async () => {
    const { ipc, stores } = await loadStores()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('attachment_import', {
      kind: 'tooLarge',
      message: 'huge.png is 9000000 bytes; the ceiling is 2097152 bytes'
    })

    await stores.attachments.importPaths(['/Users/you/Downloads/huge.png'])

    expect(stores.attachments.attachmentsState.items).toEqual([])
    expect(stores.attachments.attachmentsState.lastError).toBe(
      'huge.png is 9000000 bytes; the ceiling is 2097152 bytes'
    )
  })

  it('one refusal in the middle does not stop the rest of a selection', async () => {
    const { ipc, stores } = await loadStores()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.on('attachment_import', ({ path }) => {
      if (path.endsWith('huge.png')) throw { kind: 'tooLarge', message: 'too big' }
      return stored(path.split('/').pop())
    })

    await stores.attachments.importPaths(['/a/one.png', '/a/huge.png', '/a/two.png'])

    expect(stores.attachments.attachmentsState.items.map((i) => i.name)).toEqual([
      'one.png',
      'two.png'
    ])
  })

  it('a cancelled picker attaches nothing and refuses nothing', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('plugin:dialog|open', null)

    await stores.attachments.pickImages()

    expect(ipc.calls('attachment_import')).toEqual([])
    expect(stores.attachments.attachmentsState.items).toEqual([])
    expect(stores.attachments.attachmentsState.lastError).toBe(null)
  })

  it('the picker takes several at once', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('plugin:dialog|open', ['/a/one.png', '/a/two.png'])
    ipc.on('attachment_import', ({ path }) => stored(path.split('/').pop()))

    await stores.attachments.pickImages()

    expect(stores.attachments.attachmentsState.items).toHaveLength(2)
  })

  /* Removing forgets the path and leaves the file: the store's own note says
     why, and this pins the half that is visible — the other thumbnails stay. */
  it('removing takes out the one that was asked for and nothing else', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('attachment_import', ({ path }) => stored(path.split('/').pop()))
    await stores.attachments.importPaths(['/a/one.png', '/a/two.png'])

    stores.attachments.removeAttachment('/data/attachments/one.png')

    expect(stores.attachments.attachmentsState.items.map((i) => i.name)).toEqual(['two.png'])

    stores.attachments.clearAttachments()
    expect(stores.attachments.attachmentsState.items).toEqual([])
  })
})
