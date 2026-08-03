import { beforeEach, describe, expect, it, vi } from 'vitest'
import { isReactive } from 'vue'
import { loadStores } from '../support/stores.js'
import { buffer } from '../support/fixtures.js'

let ipc
let emit
let nextTick
let settings
let tabs

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  emit = loaded.emit
  nextTick = loaded.nextTick
  settings = loaded.stores.settings
  tabs = loaded.stores.tabs
})

describe('loading', () => {
  it('an empty answer leaves the defaults', async () => {
    ipc.on('settings_load', {})

    await settings.loadSettings()

    expect(settings.settings.appearance).toEqual({ theme: 'dark', density: 'comfortable' })
    expect(settings.settings.openProjects).toEqual([])
    expect(settings.settings.project.activeTab).toBe('kanban')
  })

  it('stored values cover the defaults field by field, not section by section', async () => {
    ipc.on('settings_load', { appearance: { theme: 'light' } })

    await settings.loadSettings()

    expect(settings.settings.appearance.theme).toBe('light')
    expect(settings.settings.appearance.density).toBe('comfortable')
  })

  it('a read refusal leaves the defaults and does not break startup', async () => {
    ipc.fail('settings_load', new Error('the file does not read'))

    await expect(settings.loadSettings()).resolves.toBeTruthy()
    expect(settings.settings.appearance.theme).toBe('dark')
  })
})

describe('a project\'s layout', () => {
  it('a project absent from the map starts clean rather than wearing somebody else\'s fields', async () => {
    ipc.on('settings_load', {
      project: { sideTab: 'agents', openTabs: ['a.txt'], expanded: ['src'] }
    })
    await settings.loadSettings()
    expect(settings.settings.project.openTabs).toEqual(['a.txt'])

    ipc.on('settings_load', { project: { sideTab: 'files' } })
    await settings.loadProjectLayout('/new')

    expect(settings.settings.project.sideTab).toBe('files')
    expect(settings.settings.project.openTabs).toEqual([])
    expect(settings.settings.project.expanded).toEqual([])
  })

  it('with no project it sets the defaults and does not go to the disk', async () => {
    ipc.on('settings_load', { project: { sideTab: 'agents' } })
    await settings.loadSettings()
    const before = ipc.calls('settings_load').length

    await settings.loadProjectLayout(null)

    expect(settings.settings.project.sideTab).toBe('files')
    expect(ipc.calls('settings_load')).toHaveLength(before)
  })

  it('a section is merged in place: the reference to the object stays the same', async () => {
    ipc.on('settings_load', {})
    await settings.loadSettings()
    const held = settings.settings.project

    ipc.on('settings_load', { project: { sideTab: 'agents' } })
    await settings.loadProjectLayout('/new')

    expect(settings.settings.project).toBe(held)
    expect(held.sideTab).toBe('agents')
  })
})

describe('writes', () => {
  beforeEach(async () => {
    ipc.on('settings_load', {})
    ipc.on('settings_save', null)
    await settings.loadSettings()
  })

  it('a stream of edits collapses into one write', async () => {
    /* Three edits in one synchronous block are collapsed by Vue's scheduler —
       the watcher fires once per tick regardless of SAVE_DELAY (the test stays
       green even with SAVE_DELAY = 0). To catch the debounce itself, two edits
       are spread across different ticks: time is advanced by less than
       SAVE_DELAY between them, so the first timer has to survive and the second
       edit has to land in the same, not-yet-sent write. */
    vi.useFakeTimers()

    settings.settings.layout.leftCollapsed = true
    await nextTick()
    vi.advanceTimersByTime(200)
    expect(ipc.calls('settings_save')).toHaveLength(0)

    settings.settings.layout.rightCollapsed = true
    await nextTick()
    vi.advanceTimersByTime(200)
    /* The first timer (400 ms from the first edit) would have fired here had
       the second edit not moved it: 200 + 200 = 400 ms from the start. */
    expect(ipc.calls('settings_save')).toHaveLength(0)

    vi.advanceTimersByTime(200)
    vi.useRealTimers()
    await Promise.resolve()

    expect(ipc.calls('settings_save')).toHaveLength(1)
    expect(ipc.calls('settings_save')[0].settings.layout.leftCollapsed).toBe(true)
    expect(ipc.calls('settings_save')[0].settings.layout.rightCollapsed).toBe(true)
  })

  it('flushPending sees a timer set in the same synchronous block', async () => {
    settings.settings.layout.leftCollapsed = true
    await settings.flushPending()

    expect(ipc.calls('settings_save')).toHaveLength(1)
  })

  it('two flushPending calls in one tick give one write with the last value', async () => {
    settings.settings.layout.leftCollapsed = true
    const first = settings.flushPending()
    settings.settings.layout.leftCollapsed = false
    const second = settings.flushPending()
    await Promise.all([first, second])

    /* The watcher is deferred to a microtask and fires once per tick, so only
       one timer is set. The first flush clears it and sends a snapshot taken
       after both edits; there is nothing for the second to call — it returns the
       same chain. */
    expect(ipc.calls('settings_save')).toHaveLength(1)
    expect(ipc.calls('settings_save')[0].settings.layout.leftCollapsed).toBe(false)
  })

  it('two writes never overlap', async () => {
    const order = []
    ipc.on('settings_save', async (args) => {
      const mark = `${args.settings.layout.leftCollapsed}/${args.settings.layout.rightCollapsed}`
      order.push(`start:${mark}`)
      await new Promise((resolve) => setTimeout(resolve, 20))
      order.push(`end:${mark}`)
      return null
    })

    settings.settings.layout.leftCollapsed = true
    const first = settings.flushPending()
    /* The tick yields to the first write: its flush is already in the chain,
       and only now does the second edit become a separate write rather than
       part of the same one. */
    await nextTick()
    settings.settings.layout.rightCollapsed = true
    const second = settings.flushPending()
    await Promise.all([first, second])

    /* Rust writes through a temp file and a rename: two overlapping writes
       would race for order, and the second could land on disk before the
       first. */
    expect(order).toEqual([
      'start:true/false',
      'end:true/false',
      'start:true/true',
      'end:true/true'
    ])
  })

  it('a plain object goes to the disk, not a reactive proxy', async () => {
    settings.settings.layout.leftCollapsed = true
    await settings.flushPending()

    const sent = ipc.calls('settings_save')[0].settings
    /* A proxy is structurally equal to its JSON clone — toEqual(JSON.parse(JSON.
       stringify(sent))) is always true, reactive or not, and so guards nothing.
       isReactive works across vue instances: the reactivity flag is read by a
       proxy getter on a string key ('__v_isReactive') rather than on a Symbol,
       which would be tied to a particular module. */
    expect(isReactive(sent)).toBe(false)
  })

  it('a write refusal does not break whoever is awaiting it', async () => {
    ipc.fail('settings_save', new Error('the disk is full'))

    settings.settings.layout.leftCollapsed = true
    await expect(settings.flushPending()).resolves.toBeUndefined()
  })
})

describe('closing the window', () => {
  beforeEach(async () => {
    ipc.on('settings_load', {})
    ipc.on('settings_save', null)
    await settings.loadSettings()
  })

  it('flushes the write and only then destroys the window', async () => {
    ipc.on('plugin:window|destroy', null)
    settings.settings.layout.leftCollapsed = true
    await nextTick()

    await emit('tauri://close-requested', {})
    await vi.waitFor(() => expect(ipc.commands()).toContain('plugin:window|destroy'))

    const commands = ipc.commands()
    /* Without this, indexOf on a settings_save that never happened would give
       -1, and -1 < N would be true in any order — the assert below would pass
       for nothing. */
    expect(commands).toContain('settings_save')
    expect(commands.indexOf('settings_save')).toBeLessThan(
      commands.indexOf('plugin:window|destroy')
    )
  })

  it('"the person changed their mind" does not close the window', async () => {
    ipc.on('plugin:window|destroy', null)
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'an edit' }))
    tabs.onUnsaved(() => false)

    await emit('tauri://close-requested', {})
    await new Promise((resolve) => setTimeout(resolve, 30))

    expect(ipc.commands()).not.toContain('plugin:window|destroy')
  })

  it('the window closes even when the write does not answer', async () => {
    /* What is promised is that the window closes. What is not promised is that
       the edit reaches the disk. */
    ipc.on('plugin:window|destroy', null)
    ipc.on('settings_save', () => new Promise(() => {}))
    settings.settings.layout.leftCollapsed = true
    await nextTick()

    await emit('tauri://close-requested', {})
    await vi.waitFor(
      () => expect(ipc.commands()).toContain('plugin:window|destroy'),
      { timeout: 4000 }
    )
  })

  it('a failed destroy clears closing: the next request reaches destroy again', async () => {
    ipc.fail('plugin:window|destroy', new Error('the window is busy'))

    await emit('tauri://close-requested', {})
    await vi.waitFor(() => expect(ipc.calls('plugin:window|destroy')).toHaveLength(1))

    /* Without clearing closing, a repeat request would be silently swallowed by
       the re-entrancy guard, and the window would stay unclosable forever. */
    await emit('tauri://close-requested', {})
    await vi.waitFor(() => expect(ipc.calls('plugin:window|destroy')).toHaveLength(2))
  })

  it('the unsaved-work question is asked before the settings write, not inside its ceiling', async () => {
    ipc.on('plugin:window|destroy', null)
    const asked = []
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'an edit' }))
    tabs.onUnsaved(() => {
      asked.push(ipc.commands().includes('settings_save'))
      return true
    })
    settings.settings.layout.leftCollapsed = true
    await nextTick()

    await emit('tauri://close-requested', {})
    await vi.waitFor(() => expect(ipc.commands()).toContain('plugin:window|destroy'))

    expect(asked).toEqual([false])
  })
})
