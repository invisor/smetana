import { describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

/* The handshake that keeps a `*:show` from being lost, from the front end's
   side of it.

   Three windows — settings, compare and image — are focused rather than
   reloaded when they are already open, so what they are to show next reaches
   them as an event. A window exists from the moment it is built, long before
   its webview has subscribed, and Tauri buffers nothing: an event sent into
   that gap used to be gone for good, which is how the image window came to name
   one picture in its frame and show another.

   What is checked here is the half that lives in `src/stores/`: that opening a
   window no longer emits the event itself — the sender is Rust, which is the
   only side that knows whether the window was built or found — that a window
   announces itself with the one command, and that each watcher reads the
   payload Rust writes. The holding itself is Rust's and is tested in
   `src-tauri/src/window.rs`; the order a window announces in lives in a `.vue`
   file and is reachable by no test in this repository. */

/* A graph with the three window commands answered, so nothing in these tests
   reaches the router's refusal for a command nobody registered. */
async function loadWindows() {
  const { stores, ipc, emit, listen } = await loadStores()
  ipc.on('settings_window_open', null)
  ipc.on('compare_window_open', null)
  ipc.on('image_window_open', null)
  ipc.on('window_show_ready', null)
  return { app: stores.app, compare: stores.compare, ipc, emit, listen }
}

/* `pickFiles`, the conversation composer's own picker — any file, no
   filters, and a bare path handed straight back rather than copied anywhere.
   The shape of these tests follows `pickImages` in `tests/stores/
   attachments.test.js`, the picker this one is deliberately not built on top
   of (see `stores/app.js`'s own header on the function). */
describe("the composer's paperclip button", () => {
  it('a cancelled dialog attaches nothing', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('plugin:dialog|open', null)

    await expect(stores.app.pickFiles()).resolves.toEqual([])
  })

  it('several files chosen at once come back as they were given', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('plugin:dialog|open', ['/a/one.txt', '/a/two.png'])

    await expect(stores.app.pickFiles()).resolves.toEqual(['/a/one.txt', '/a/two.png'])
  })

  it('a single path answered as a bare string becomes a list of one', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('plugin:dialog|open', '/a/one.txt')

    await expect(stores.app.pickFiles()).resolves.toEqual(['/a/one.txt'])
  })

  it('offers every file, with no extension filter', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('plugin:dialog|open', null)

    await stores.app.pickFiles()

    const options = ipc.calls('plugin:dialog|open')[0].options
    expect(options.multiple).toBe(true)
    expect(options.filters).toBeUndefined()
  })

  it('the second open starts in the folder of the first file chosen', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('plugin:dialog|open', ['/a/one.txt'])
    await stores.app.pickFiles()

    ipc.on('plugin:dialog|open', null)
    await stores.app.pickFiles()

    const options = ipc.calls('plugin:dialog|open').map((call) => call.options)
    expect(options[1].defaultPath).toBe('/a')
  })
})

describe('a window saying it is ready to be re-aimed', () => {
  it('asks the desktop for whatever it missed while it was loading', async () => {
    const { app, ipc } = await loadWindows()

    await app.announceWindowReady()

    expect(ipc.commands()).toContain('window_show_ready')
  })

  /* Which window is speaking is the webview's own label, read on the far side,
     so there is nothing here for a caller to get wrong or for the two sides to
     disagree about. */
  it('names no window, because the label is the webview it is called from', async () => {
    const { app, ipc } = await loadWindows()

    await app.announceWindowReady()

    expect(ipc.calls('window_show_ready')).toEqual([{}])
  })

  /* A browser is the ordinary way to reach a refusal here, and this window is
     already drawing whatever its URL named. A thrown error would take the rest
     of a window's `onMounted` with it — the settings read, the focus listener —
     over a message that is only ever a correction. */
  it('does not throw at a window when there is nobody to tell', async () => {
    const { app, ipc } = await loadWindows()
    ipc.fail('window_show_ready', new Error('mockBackend: no window to announce to'))

    await expect(app.announceWindowReady()).resolves.toBeUndefined()
  })
})

describe('opening one of the three re-aimable windows', () => {
  /* The event is Rust's to send, and this is the whole of the change: emitted
     from here it reached a window built by this very press only if that window
     had already subscribed, which one a moment old has not. */
  it('leaves the picture to Rust rather than announcing it here', async () => {
    const { app, ipc, listen } = await loadWindows()
    const heard = vi.fn()
    await listen(app.IMAGE_SHOW, heard)

    await app.openImageWindow('/store/a.png', 'a.png')

    expect(ipc.calls('image_window_open')).toEqual([{ path: '/store/a.png', name: 'a.png' }])
    expect(heard).not.toHaveBeenCalled()
  })

  it('leaves the section to Rust rather than announcing it here', async () => {
    const { app, ipc, listen } = await loadWindows()
    const heard = vi.fn()
    await listen(app.SETTINGS_SHOW, heard)

    await app.openSettingsWindow('storage')

    expect(ipc.calls('settings_window_open')).toEqual([{ tab: 'storage' }])
    expect(heard).not.toHaveBeenCalled()
  })

  it('leaves the pair to Rust rather than announcing it here', async () => {
    const { compare, ipc, listen } = await loadWindows()
    const heard = vi.fn()
    await listen(compare.COMPARE_SHOW, heard)

    await compare.openCompareWindow('/tmp/r', 'feature')

    expect(ipc.calls('compare_window_open')).toEqual([{ repo: '/tmp/r', branch: 'feature' }])
    expect(heard).not.toHaveBeenCalled()
  })

  /* A window that did not open is the larger failure and is the one thing said
     loudly; nothing else about the press is left to do. */
  it('says so and gives up when the window itself did not open', async () => {
    const { app, ipc } = await loadWindows()
    ipc.fail('image_window_open', new Error('mockBackend: no window to make'))
    const said = vi.spyOn(console, 'error').mockImplementation(() => {})

    await expect(app.openImageWindow('/store/a.png', 'a.png')).resolves.toBeUndefined()

    expect(said).toHaveBeenCalled()
    said.mockRestore()
  })
})

describe('what a re-aimed window reads off the event', () => {
  /* The field names are the other half of a pair with `image_show` in
     `src-tauri/src/window.rs`, and nothing mechanical holds the two sides
     together: renamed on either, the window opens on its empty state for every
     picture, for ever. */
  it('reads the picture out of the words Rust writes', async () => {
    const { app, emit } = await loadWindows()
    const shown = vi.fn()
    await app.watchImageShow(shown)

    await emit(app.IMAGE_SHOW, { path: '/store/b.png', name: 'b.png' })

    expect(shown).toHaveBeenCalledWith('/store/b.png', 'b.png')
  })

  it('reads the section out of the words Rust writes', async () => {
    const { app, emit } = await loadWindows()
    const asked = vi.fn()
    await app.watchSettingsSection(asked)

    await emit(app.SETTINGS_SHOW, { tab: 'storage' })

    expect(asked).toHaveBeenCalledWith('storage')
  })

  it('marks a focus-only reopening as a Codex model refresh', async () => {
    const { stores, emit } = await loadStores()
    const asked = vi.fn()
    await stores.app.watchSettingsSection(asked)
    await emit(stores.app.SETTINGS_SHOW, { tab: null, refreshCodexModels: true })
    expect(asked).toHaveBeenCalledWith(null, true)
  })

  it('reads the pair out of the words Rust writes', async () => {
    const { compare, emit } = await loadWindows()
    const aimed = vi.fn()
    await compare.watchCompareTarget(aimed)

    await emit(compare.COMPARE_SHOW, { repo: '/tmp/r', branch: 'feature' })

    expect(aimed).toHaveBeenCalledWith('/tmp/r', 'feature')
  })

  /* An event is not a response to anything, so a malformed one costs nothing:
     the window is told about no picture and draws the empty state it already
     has for one. */
  it('treats an event with nothing in it as no picture at all', async () => {
    const { app, emit } = await loadWindows()
    const shown = vi.fn()
    await app.watchImageShow(shown)

    await emit(app.IMAGE_SHOW, null)

    expect(shown).toHaveBeenCalledWith(null, '')
  })

  /* Several arrive when a window was re-aimed after it had loaded — the
     ordinary case, where nothing is held at all — and the window is left on the
     last of them. */
  it('leaves a window on the last picture it was asked for', async () => {
    const { app, emit } = await loadWindows()
    const shown = vi.fn()
    await app.watchImageShow(shown)

    await emit(app.IMAGE_SHOW, { path: '/store/a.png', name: 'a.png' })
    await emit(app.IMAGE_SHOW, { path: '/store/b.png', name: 'b.png' })

    expect(shown).toHaveBeenLastCalledWith('/store/b.png', 'b.png')
  })
})

describe('the subscription probe', () => {
  /* `project` rides beside `agent` on every call, new with smetana-9x2y: it is
     what lets `runs/commands.rs`'s own project-aware fallback answer about the
     same project a run's own gate would, whichever of the two callers ever
     leaves `agent` unnamed. */
  it('sends the agent and the project together', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('agent_usage', { state: 'read', sessionPct: 10, sessionReset: '', weekPct: null, weekReset: null })

    await stores.app.readAgentUsage('codex', '/work/holiday-curb')

    expect(ipc.calls('agent_usage')).toEqual([{ agent: 'codex', project: '/work/holiday-curb' }])
  })

  it('is a caller with no opinion by default', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('agent_usage', { state: 'read', sessionPct: 10, sessionReset: '', weekPct: null, weekReset: null })

    await stores.app.readAgentUsage()

    expect(ipc.calls('agent_usage')).toEqual([{ agent: null, project: null }])
  })
})
