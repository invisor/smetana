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
  size: bytes.length,
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
      message: 'huge.png is 9000000 bytes; the ceiling is 8388608 bytes'
    })

    await stores.attachments.importPaths(['/Users/you/Downloads/huge.png'])

    expect(stores.attachments.attachmentsState.items).toEqual([])
    expect(stores.attachments.attachmentsState.lastError).toBe(
      'huge.png is 9000000 bytes; the ceiling is 8388608 bytes'
    )
  })

  /* The refusal has to survive the successes around it. A batch where the
     oversized file is refused and the small one lands would otherwise show one
     thumbnail, no message and nothing at all to say the other never arrived —
     a write that failed and looked like it worked. */
  it('a refusal in the middle of a batch is still on screen at the end of it', async () => {
    const { ipc, stores } = await loadStores()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.on('attachment_import', ({ path }) => {
      if (path.endsWith('huge.png')) throw { kind: 'tooLarge', message: 'huge.png is too big' }
      return stored(path.split('/').pop())
    })

    await stores.attachments.importPaths(['/a/one.png', '/a/huge.png', '/a/two.png'])

    expect(stores.attachments.attachmentsState.items.map((i) => i.name)).toEqual([
      'one.png',
      'two.png'
    ])
    expect(stores.attachments.attachmentsState.lastError).toBe('huge.png is too big')
  })

  it('the next batch starts without the last one\'s refusal', async () => {
    const { ipc, stores } = await loadStores()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('attachment_import', { kind: 'tooLarge', message: 'too big' })
    await stores.attachments.importPaths(['/a/huge.png'])
    expect(stores.attachments.attachmentsState.lastError).toBe('too big')

    ipc.on('attachment_import', ({ path }) => stored(path.split('/').pop()))
    await stores.attachments.importPaths(['/a/one.png'])

    expect(stores.attachments.attachmentsState.lastError).toBe(null)
  })

  /* The size is judged before the bytes are encoded: encoding first would build
     a base64 string a third larger than the file only to have Rust refuse it. */
  it('an oversized file is refused without ever reaching the back end', async () => {
    const { ipc, stores } = await loadStores()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const huge = { name: 'huge.png', size: 40 * 1024 * 1024, arrayBuffer: async () => new ArrayBuffer(0) }

    await stores.attachments.attachFiles([huge])

    expect(ipc.calls('attachment_write')).toEqual([])
    expect(stores.attachments.attachmentsState.items).toEqual([])
    expect(stores.attachments.attachmentsState.lastError).toContain('huge.png')
    expect(stores.attachments.attachmentsState.lastError).toContain('ceiling')
  })

  it('a cancelled picker attaches nothing and refuses nothing', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('plugin:dialog|open', null)

    await stores.attachments.pickImages()

    expect(ipc.calls('attachment_import')).toEqual([])
    expect(stores.attachments.attachmentsState.items).toEqual([])
    expect(stores.attachments.attachmentsState.lastError).toBe(null)
  })

  /* Starting from a store that has something to lose, because the test above
     starts from an empty one and would pass whatever the code did with the
     message. Opening the picker is not an attempt to attach anything: a person
     who pastes an oversized screenshot, reads why it was refused, opens the
     picker and thinks better of it has done nothing that should take the
     explanation off the screen. */
  it('cancelling the picker leaves an earlier refusal on screen', async () => {
    const { ipc, stores } = await loadStores()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('attachment_import', { kind: 'tooLarge', message: 'huge.png is too big' })
    await stores.attachments.importPaths(['/a/huge.png'])
    expect(stores.attachments.attachmentsState.lastError).toBe('huge.png is too big')

    ipc.on('plugin:dialog|open', null)
    await stores.attachments.pickImages()

    expect(stores.attachments.attachmentsState.lastError).toBe('huge.png is too big')
  })

  /* The other half: a picker that fails has a message of its own, and it must
     win over whatever an earlier attempt left behind rather than queue behind
     it — `fail` keeps the first refusal of a batch, and this is a new one. */
  it('a picker that fails says so, over an older refusal', async () => {
    const { ipc, stores } = await loadStores()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('attachment_import', { kind: 'tooLarge', message: 'huge.png is too big' })
    await stores.attachments.importPaths(['/a/huge.png'])

    ipc.fail('plugin:dialog|open', new Error('the picker did not open'))
    await stores.attachments.pickImages()

    expect(stores.attachments.attachmentsState.lastError).toContain('the picker did not open')
  })

  it('the picker takes several at once', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('plugin:dialog|open', ['/a/one.png', '/a/two.png'])
    ipc.on('attachment_import', ({ path }) => stored(path.split('/').pop()))

    await stores.attachments.pickImages()

    expect(stores.attachments.attachmentsState.items).toHaveLength(2)
  })

  /* Handed no `defaultPath`, macOS opens the panel where it was left last and,
     with no such memory, in Recents — every application's files at once. The
     panel builds a QuickLook preview for every row it draws there, and an
     unsandboxed app is the process those touches are charged to, so a person
     pressing Attach is asked to let a development tool at their photographs. */
  describe('where the picker opens', () => {
    const optionsOf = (ipc) => ipc.calls('plugin:dialog|open').map((call) => call.options)

    it('the first open after the app starts is the download directory', async () => {
      const { ipc, stores } = await loadStores()
      ipc.on('plugin:path|resolve_directory', '/Users/you/Downloads')
      ipc.on('plugin:dialog|open', null)

      await stores.attachments.pickImages()

      expect(optionsOf(ipc)[0].defaultPath).toBe('/Users/you/Downloads')
    })

    /* `defaultPath` overrides the panel's own memory, so handing over the same
       directory every time would send a person who walked elsewhere back to the
       start on every open — worse than the bug. The folder of the last choice
       is what goes over instead, and of several picked at once it is the first:
       they are siblings, and it is the one that was clicked. */
    it('the next open starts where the last choice was made', async () => {
      const { ipc, stores } = await loadStores()
      ipc.on('plugin:path|resolve_directory', '/Users/you/Downloads')
      ipc.on('attachment_import', ({ path }) => stored(path.split('/').pop()))
      ipc.on('plugin:dialog|open', ['/Users/you/shots/one.png', '/Users/you/shots/two.png'])
      await stores.attachments.pickImages()

      ipc.on('plugin:dialog|open', null)
      await stores.attachments.pickImages()

      expect(optionsOf(ipc)[1].defaultPath).toBe('/Users/you/shots')
    })

    it('cancelling chooses nothing and therefore moves nothing', async () => {
      const { ipc, stores } = await loadStores()
      ipc.on('plugin:path|resolve_directory', '/Users/you/Downloads')
      ipc.on('attachment_import', ({ path }) => stored(path.split('/').pop()))
      ipc.on('plugin:dialog|open', ['/Users/you/shots/one.png'])
      await stores.attachments.pickImages()

      ipc.on('plugin:dialog|open', null)
      await stores.attachments.pickImages()
      await stores.attachments.pickImages()

      expect(optionsOf(ipc)[2].defaultPath).toBe('/Users/you/shots')
    })

    /* `plugin:path|resolve_directory` is deliberately left unregistered here,
       and that is the ordinary case rather than an exotic one: there is no path
       plugin behind a browser under `npm run dev` either. A directory that
       cannot be worked out is not a picker that failed. */
    it('a directory that cannot be resolved leaves the option off and opens anyway', async () => {
      const { ipc, stores } = await loadStores()
      ipc.on('plugin:dialog|open', null)

      await stores.attachments.pickImages()

      expect(optionsOf(ipc)).toHaveLength(1)
      expect(optionsOf(ipc)[0].defaultPath).toBeUndefined()
      expect(stores.attachments.attachmentsState.lastError).toBe(null)
    })
  })

  /* The third route. A drop never reaches the webview — Tauri intercepts it and
     reports it against the window — so these arrive as real events through the
     same transport the tracker's deltas do, with no ordering guarantee about
     when the subscription itself finishes being set up. */
  describe('drops on the window', () => {
    const drop = (paths) => ({ paths, position: { x: 10, y: 20 } })
    /* `onDragDropEvent` is four `listen` calls deep, each one a round trip
       through the mocked transport, so the subscription is not in place on the
       next microtask. */
    const settle = () => new Promise((resolve) => setTimeout(resolve, 0))

    it('a drop while the dialog is open attaches every path in it', async () => {
      const { ipc, emit, stores } = await loadStores()
      ipc.on('attachment_import', ({ path }) => stored(path.split('/').pop()))
      stores.attachments.watchDrops(() => true)
      await settle()

      await emit('tauri://drag-drop', drop(['/a/one.png', '/a/two.png']))
      await settle()

      expect(stores.attachments.attachmentsState.items.map((i) => i.name)).toEqual([
        'one.png',
        'two.png'
      ])
    })

    /* The store does not decide whether anything is collecting — the view does,
       and it is asked. Without the gate a drop anywhere in the app would file
       images into a list nobody has open. */
    it('a drop with nothing collecting is ignored', async () => {
      const { ipc, emit, stores } = await loadStores()
      ipc.on('attachment_import', () => stored('one.png'))
      stores.attachments.watchDrops(() => false)
      await settle()

      await emit('tauri://drag-drop', drop(['/a/one.png']))
      await settle()

      expect(ipc.calls('attachment_import')).toEqual([])
      expect(stores.attachments.attachmentsState.dragging).toBe(false)
    })

    it('dragging over the window and away again is only a flag', async () => {
      const { emit, stores } = await loadStores()
      stores.attachments.watchDrops(() => true)
      await settle()

      await emit('tauri://drag-over', { position: { x: 1, y: 2 } })
      expect(stores.attachments.attachmentsState.dragging).toBe(true)

      await emit('tauri://drag-leave', {})
      expect(stores.attachments.attachmentsState.dragging).toBe(false)
    })

    it('after unsubscribing a drop reaches nothing', async () => {
      const { ipc, emit, stores } = await loadStores()
      vi.spyOn(console, 'warn').mockImplementation(() => {})
      ipc.on('attachment_import', () => stored('one.png'))
      const stop = stores.attachments.watchDrops(() => true)
      await settle()

      stop()
      await emit('tauri://drag-drop', drop(['/a/one.png']))
      await settle()

      expect(ipc.calls('attachment_import')).toEqual([])
    })

    /* Unmounting before the subscription has finished being set up. The view
       leaves as soon as the person closes the window, and the promise behind
       `onDragDropEvent` may still be in flight — without the flag the listener
       would be installed after its owner was gone and would never come off. */
    it('unsubscribing before the subscription lands still leaves nothing listening', async () => {
      const { ipc, emit, stores } = await loadStores()
      vi.spyOn(console, 'warn').mockImplementation(() => {})
      ipc.on('attachment_import', () => stored('one.png'))

      const stop = stores.attachments.watchDrops(() => true)
      stop()
      await settle()

      await emit('tauri://drag-drop', drop(['/a/one.png']))
      await settle()

      expect(ipc.calls('attachment_import')).toEqual([])
    })
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
