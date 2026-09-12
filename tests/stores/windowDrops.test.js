import { describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

/* The shared subscription behind `watchSessionDrops` in `terminals.js` and
   `watchDrops` in `attachments.js`, and the conversation panel's own call to
   it (`smetana-h8vq`). This file pins the contract directly, once, rather than
   through any one of its callers — the same shape `terminals.test.js` used to
   pin alone before the lifecycle moved out here; that file's own "drops on
   the window" tests still pass unchanged, since `watchSessionDrops` is now a
   plain re-export of `watchWindowDrops`.

   `onDragDropEvent` is four `listen` calls deep, each one a round trip through
   the mocked transport, so the subscription is not in place on the next
   microtask. */
describe('the shared window-drop subscription', () => {
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0))

  const watch = async (stores) => {
    const seen = []
    const stop = stores.windowDrops.watchWindowDrops({
      over: (at) => seen.push({ type: 'over', ...at }),
      leave: () => seen.push({ type: 'leave' }),
      drop: (at) => seen.push({ type: 'drop', ...at })
    })
    await settle()
    return { seen, stop }
  }

  const ready = async () => {
    const loaded = await loadStores()
    loaded.ipc.on('drag_drop_space', 'physical')
    return loaded
  }

  it('hands over the paths and the point they were let go at', async () => {
    const { stores, emit } = await ready()
    const { seen } = await watch(stores)

    await emit('tauri://drag-drop', { paths: ['/tmp/a.png'], position: { x: 40, y: 60 } })
    await settle()

    expect(seen).toEqual([{ type: 'drop', x: 40, y: 60, paths: ['/tmp/a.png'] }])
  })

  /* `document.elementFromPoint` reads CSS pixels, so the conversion happens
     here — the one place that knows which units arrived. Where the event is
     really in device pixels, an unconverted point lands at twice the distance
     from the corner on a retina screen, which on a three-column window is a
     different panel altogether. */
  it('turns a physical point into CSS pixels', async () => {
    const loaded = await ready()
    loaded.ipc.on('drag_drop_space', 'physical')
    vi.stubGlobal('devicePixelRatio', 2)
    const { seen } = await watch(loaded.stores)

    await loaded.emit('tauri://drag-drop', { paths: ['/tmp/a.png'], position: { x: 40, y: 60 } })
    await settle()
    vi.unstubAllGlobals()

    expect(seen).toEqual([{ type: 'drop', x: 20, y: 30, paths: ['/tmp/a.png'] }])
  })

  /* And where it is not. Tauri types the position physical everywhere, but wry
     passes the toolkit's own numbers through unscaled, and AppKit's points and
     GTK's widget coordinates are already the webview's CSS pixels. Halving one
     of those was the whole of smetana-uoux. */
  it('leaves a logical point where it arrived, whatever the device pixel ratio', async () => {
    const loaded = await ready()
    loaded.ipc.on('drag_drop_space', 'logical')
    vi.stubGlobal('devicePixelRatio', 2)
    const { seen } = await watch(loaded.stores)

    await loaded.emit('tauri://drag-drop', { paths: ['/tmp/a.png'], position: { x: 40, y: 60 } })
    await settle()
    vi.unstubAllGlobals()

    expect(seen).toEqual([{ type: 'drop', x: 40, y: 60, paths: ['/tmp/a.png'] }])
  })

  /* A word from a back end this front end does not know, and a back end that
     is not there at all, are the same case: the point is read the way every
     platform used to read it rather than not read at all. */
  it('an answer nobody knows is read as physical', async () => {
    const loaded = await ready()
    loaded.ipc.fail('drag_drop_space', new Error('no such command'))
    vi.stubGlobal('devicePixelRatio', 2)
    const { seen } = await watch(loaded.stores)

    await loaded.emit('tauri://drag-drop', { paths: ['/tmp/a.png'], position: { x: 40, y: 60 } })
    await settle()
    vi.unstubAllGlobals()

    expect(seen).toEqual([{ type: 'drop', x: 20, y: 30, paths: ['/tmp/a.png'] }])
  })

  /* Entering carries the paths and the events in the middle of the drag do
     not, which is the whole reason `over` is handed them at all: a caller
     saying how many files are coming has nowhere else to read the number. */
  it('carries the paths on entering and null while moving', async () => {
    const { stores, emit } = await ready()
    const { seen } = await watch(stores)

    await emit('tauri://drag-enter', { paths: ['/tmp/a.png', '/tmp/b.png'], position: { x: 1, y: 2 } })
    await emit('tauri://drag-over', { position: { x: 3, y: 4 } })
    await settle()

    expect(seen).toEqual([
      { type: 'over', x: 1, y: 2, paths: ['/tmp/a.png', '/tmp/b.png'] },
      { type: 'over', x: 3, y: 4, paths: null }
    ])
  })

  it('the drag leaving the window ends the response', async () => {
    const { stores, emit } = await ready()
    const { seen } = await watch(stores)

    await emit('tauri://drag-leave', {})
    await settle()

    expect(seen).toEqual([{ type: 'leave' }])
  })

  it('asks which units a drop arrives in only once for every watcher on the window', async () => {
    const { stores, ipc, emit } = await ready()
    await watch(stores)
    await watch(stores)

    await emit('tauri://drag-drop', { paths: ['/tmp/a.png'], position: { x: 1, y: 2 } })
    await settle()

    expect(ipc.calls('drag_drop_space')).toHaveLength(1)
  })

  /* `getCurrentWebview` reads `window.__TAURI_INTERNALS__.metadata`, which a
     real browser never has — the same throw `settings.js` reads off
     `getCurrentWindow` as "we are in a browser". Removing it here is the
     lightest stand-in for that environment this suite has, since nothing
     installs a browser-shaped `window` otherwise. */
  it('in a browser, nothing is asked and nothing is subscribed', async () => {
    const { stores } = await ready()
    vi.spyOn(console, 'debug').mockImplementation(() => {})
    const original = globalThis.__TAURI_INTERNALS__
    delete globalThis.__TAURI_INTERNALS__

    const stop = stores.windowDrops.watchWindowDrops({ drop: () => {} })

    expect(typeof stop).toBe('function')
    expect(() => stop()).not.toThrow()
    expect(console.debug).toHaveBeenCalledWith(expect.stringContaining('[windowDrops]'))

    globalThis.__TAURI_INTERNALS__ = original
  })

  it('after unsubscribing a drop reaches nothing', async () => {
    const { stores, emit } = await ready()
    const { seen, stop } = await watch(stores)

    stop()
    await emit('tauri://drag-drop', { paths: ['/tmp/a.png'], position: { x: 1, y: 2 } })
    await settle()

    expect(seen).toEqual([])
  })

  /* Unmounting before the subscription has finished being set up: the pane
     goes as soon as the person switches tab, and the promise behind
     `onDragDropEvent` may still be in flight — without the flag the listener
     would be installed after its owner was gone and would never come off. */
  it('unsubscribing before the subscription lands still leaves nothing listening', async () => {
    const { stores, emit } = await ready()
    const seen = []
    const stop = stores.windowDrops.watchWindowDrops({ drop: (at) => seen.push(at) })
    stop()
    await settle()

    await emit('tauri://drag-drop', { paths: ['/tmp/a.png'], position: { x: 1, y: 2 } })
    await settle()

    expect(seen).toEqual([])
  })
})
