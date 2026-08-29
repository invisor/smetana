import { beforeEach, describe, expect, it, vi } from 'vitest'
import { isReactive } from 'vue'
import { loadStores } from '../support/stores.js'
import { buffer } from '../support/fixtures.js'

let ipc
let emit
let listen
let nextTick
let settings
let tabs

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  emit = loaded.emit
  listen = loaded.listen
  nextTick = loaded.nextTick
  settings = loaded.stores.settings
  tabs = loaded.stores.tabs
})

describe('loading', () => {
  it('an empty answer leaves the defaults', async () => {
    ipc.on('settings_load', {})

    await settings.loadSettings()

    expect(settings.settings.appearance).toEqual({
      theme: 'dark',
      density: 'comfortable',
      uiFontSize: 13
    })
    /* Word wrap off is today's behaviour to the letter — a long line scrolls
       sideways — which is why the shipped value is the one it is. */
    expect(settings.settings.editor).toEqual({ fontSize: 12, wordWrap: false })
    /* Both on. The fetch, because the marks the Git panel draws are worthless
       when nothing goes and asks, and the switch is for the machines where
       background network is not free. The removal, because that is today's
       behaviour exactly — the running-tasks skill has always swept a merged
       task's worktree up. `settings/model.rs` carries the same pair. */
    expect(settings.settings.git).toEqual({ autoFetch: true, removeWorktrees: true })
    /* On, because it is what the app has always done: the window has opened
       where it was left since before there was a switch over it, and
       `settings/model.rs` carries this same default. */
    expect(settings.settings.window).toEqual({ restoreGeometry: true })
    /* On, because an app that never asks is an app whose update system does not
       exist for anybody who does not go looking. `settings/model.rs` carries
       this same default, and so do the settings window and the component. */
    expect(settings.settings.updates).toEqual({ autoCheck: true })
    /* Today's board exactly: every column, every task. Nothing on anybody's
       screen moves until they go and choose. */
    expect(settings.settings.kanban).toEqual({
      columns: 'all',
      alwaysShow: [],
      interval: 'all',
      unlimited: []
    })
    /* Shipped on rather than off, and as two different sounds — a file written
       before this section existed is every file on a person's disk right now,
       and `settings/model.rs` carries the same pair. The report switch beside
       them is on for the same reason one step over: that is what the app has
       always done, and an update must not quietly stop a person's reports
       arriving. */
    expect(settings.settings.notifications).toEqual({
      runFinished: 'sound-1',
      needsAttention: 'sound-2',
      onlyWhenUnfocused: true,
      showReport: true
    })
    expect(settings.settings.openProjects).toEqual([])
    expect(settings.settings.project.activeTab).toBe('kanban')
  })

  it('reads a stored pair of sounds off the file', async () => {
    ipc.on('settings_load', { notifications: { runFinished: 'off', needsAttention: 'sound-4' } })

    await settings.loadSettings()

    expect(settings.settings.notifications.runFinished).toBe('off')
    expect(settings.settings.notifications.needsAttention).toBe('sound-4')
  })

  it('reads the stored focus switch off the file', async () => {
    /* The one default here that is a change rather than a preservation, so the
       stored `false` is somebody asking for what the app used to do. */
    ipc.on('settings_load', { notifications: { onlyWhenUnfocused: false } })

    await settings.loadSettings()

    expect(settings.settings.notifications.onlyWhenUnfocused).toBe(false)
    expect(
      settings.settings.notifications.showReport,
      'a section naming only this switch leaves the one beside it alone'
    ).toBe(true)
  })

  it('reads a stored report switch off the file', async () => {
    ipc.on('settings_load', { notifications: { showReport: false } })

    await settings.loadSettings()

    expect(settings.settings.notifications.showReport).toBe(false)
    expect(settings.settings.notifications.runFinished).toBe(
      'sound-1',
      'a section naming only the switch leaves the sounds at their defaults'
    )
  })

  it('reads a stored false for the window geometry off the file', async () => {
    ipc.on('settings_load', { window: { restoreGeometry: false } })

    await settings.loadSettings()

    expect(settings.settings.window.restoreGeometry).toBe(false)
  })

  /* A file written by a build before this section existed is every file on a
     person's disk right now, and it has to open with the check on rather than
     with the `undefined` a missing section would otherwise leave. */
  it('reads the update switch off the file, and takes the default without one', async () => {
    ipc.on('settings_load', { updates: { autoCheck: false } })

    await settings.loadSettings()

    expect(settings.settings.updates.autoCheck).toBe(false)

    ipc.on('settings_load', { appearance: { theme: 'light' } })

    await settings.loadSettings()

    expect(settings.settings.updates.autoCheck).toBe(true)
  })

  it('reads the board settings off the file', async () => {
    ipc.on('settings_load', {
      kanban: { columns: 'non-empty', alwaysShow: ['ready'], interval: 'day' }
    })

    await settings.loadSettings()

    expect(settings.settings.kanban.columns).toBe('non-empty')
    expect(settings.settings.kanban.alwaysShow).toEqual(['ready'])
    expect(settings.settings.kanban.interval).toBe('day')
    expect(settings.settings.kanban.unlimited).toEqual([], 'the field the file left out takes its default')
  })

  it('stored values cover the defaults field by field, not section by section', async () => {
    ipc.on('settings_load', { appearance: { theme: 'light' } })

    await settings.loadSettings()

    expect(settings.settings.appearance.theme).toBe('light')
    expect(settings.settings.appearance.density).toBe('comfortable')
  })

  it('the rail is open until somebody hides it, and a stored flag survives a load', async () => {
    ipc.on('settings_load', { layout: { leftWidth: 300 } })
    await settings.loadSettings()
    expect(settings.settings.layout.railOpen).toBe(true)
    expect(settings.settings.layout.leftWidth).toBe(300)

    const hidden = await loadStores()
    hidden.ipc.on('settings_load', { layout: { railOpen: false } })
    await hidden.stores.settings.loadSettings()
    expect(hidden.stores.settings.settings.layout.railOpen).toBe(false)
  })

  it('a read refusal leaves the defaults and does not break startup', async () => {
    ipc.fail('settings_load', new Error('the file does not read'))

    await expect(settings.loadSettings()).resolves.toBeTruthy()
    expect(settings.settings.appearance.theme).toBe('dark')
  })

  it('opens on Claude Code when the file names no agent', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('settings_load', {})
    ipc.on('settings_save', null)
    await stores.settings.loadSettings()
    expect(stores.settings.settings.agent).toBe('claude')
  })

  it('takes the agent from the file and sends it back on a save', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('settings_load', { agent: 'codex' })
    ipc.on('settings_save', null)
    await stores.settings.loadSettings()
    expect(stores.settings.settings.agent).toBe('codex')

    stores.settings.settings.appearance.theme = 'light'
    await stores.settings.flushPending()
    expect(ipc.calls('settings_save').at(-1).settings.agent).toBe('codex')
  })

  it('opens on English when the file names no language, and takes what it does name', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('settings_load', {})
    ipc.on('settings_save', null)
    await stores.settings.loadSettings()
    expect(stores.settings.settings.agentLanguage).toBe('en')
    expect(stores.settings.settings.taskLanguage).toBe('en')
    expect(stores.settings.settings.commitLanguage).toBe('en')
    expect(stores.settings.settings.reportLanguage).toBe('en')

    const second = await loadStores()
    second.ipc.on('settings_load', {
      agentLanguage: 'ru',
      taskLanguage: 'ja',
      commitLanguage: 'de',
      reportLanguage: 'it'
    })
    second.ipc.on('settings_save', null)
    await second.stores.settings.loadSettings()
    expect(second.stores.settings.settings.agentLanguage).toBe('ru')
    expect(second.stores.settings.settings.taskLanguage).toBe('ja')
    expect(second.stores.settings.settings.commitLanguage).toBe('de')
    expect(second.stores.settings.settings.reportLanguage).toBe('it')

    /* And back out on the next write, so a restart brings the choice back. */
    second.stores.settings.settings.appearance.theme = 'light'
    await second.stores.settings.flushPending()
    const sent = second.ipc.calls('settings_save').at(-1).settings
    expect(sent.agentLanguage).toBe('ru')
    expect(sent.taskLanguage).toBe('ja')
    expect(sent.commitLanguage).toBe('de')
    expect(sent.reportLanguage).toBe('it')
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

  it("clears one project's recent tasks when another project is opened", async () => {
    /* The defaults layer can only clear a key that is in the defaults object,
       and this one has to be cleared: the palette draws these under `Recent`,
       and a previous project's three tasks standing under the next project's
       board would be three rows nobody there has ever opened. */
    ipc.on('settings_load', { project: { recentTasks: ['a', 'b', 'c'] } })
    await settings.loadSettings()
    expect(settings.settings.project.recentTasks).toEqual(['a', 'b', 'c'])

    ipc.on('settings_load', { project: {} })
    await settings.loadProjectLayout('/new')

    expect(settings.settings.project.recentTasks).toEqual([])
  })

  it("remembers the right column's tab, and clears it for a project that has none", async () => {
    /* The tab the right column was left on, stored beside `sideTab` and read
       the same way — the whole point of the field is that a restart brings it
       back. The second half is what listing it in the defaults buys: a project
       with no `rightTab` of its own has to open on Task, and a key missing from
       the defaults object is a key the defaults layer cannot clear, so the
       previous project's Sessions would stand under the next project's board.

       Written out of the store as well as into it, since a value that only ever
       arrives is a value no restart can preserve. */
    ipc.on('settings_load', { project: { rightTab: 'sessions' } })
    ipc.on('settings_save', null)
    await settings.loadSettings()
    expect(settings.settings.project.rightTab).toBe('sessions')

    settings.settings.project.selectedTask = 'smetana-l56'
    await settings.flushPending()
    expect(ipc.calls('settings_save').at(-1).settings.project.rightTab).toBe('sessions')

    ipc.on('settings_load', { project: {} })
    await settings.loadProjectLayout('/new')

    expect(settings.settings.project.rightTab).toBe('task')
  })

  it("keeps a project's pinned branches, and clears them for a project with none", async () => {
    /* Which branches are pinned above the Git panel's tree. Listed in the
       defaults for `rightTab`'s reason — a key missing from that object is a
       key the defaults layer cannot clear — and the consequence here is louder
       than a tab: one project's `main` standing in another project's panel is a
       row somebody never marked.

       Written out of the store as well as into it, since a value that only ever
       arrives is a value no restart can preserve. */
    ipc.on('settings_load', { project: { favoriteBranches: ['main', 'fix/legacy'] } })
    ipc.on('settings_save', null)
    await settings.loadSettings()
    expect(settings.settings.project.favoriteBranches).toEqual(['main', 'fix/legacy'])

    settings.settings.project.selectedTask = 'smetana-j9y'
    await settings.flushPending()
    expect(ipc.calls('settings_save').at(-1).settings.project.favoriteBranches).toEqual([
      'main',
      'fix/legacy'
    ])

    ipc.on('settings_load', { project: {} })
    await settings.loadProjectLayout('/new')

    expect(settings.settings.project.favoriteBranches).toEqual([])
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

  /* The choice has to survive a restart, and the whole of that is the store
     writing the section back out: the save sends the settings object as it
     stands, and Rust reads `notifications.showReport` off it by name. */
  it('carries the report switch to the disk', async () => {
    settings.settings.notifications.showReport = false
    await settings.flushPending()

    expect(ipc.calls('settings_save').at(-1).settings.notifications.showReport).toBe(false)
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

describe('the settings window', () => {
  beforeEach(async () => {
    ipc.on('settings_load', {})
    ipc.on('settings_save', null)
    await settings.loadSettings()
    await settings.initSettingsBridge()
  })

  it('an edit made over there lands here and goes to disk from here', async () => {
    /* The whole point of the one-writer rule: the settings window never calls
       settings_save, and what reaches the file is this window's own debounced
       write, carrying everything else it knows about. */
    await emit(settings.SETTINGS_APPLY, { theme: 'light', uiFontSize: 16, editorFontSize: 18 })
    await nextTick()

    expect(settings.settings.appearance.theme).toBe('light')
    expect(settings.settings.appearance.uiFontSize).toBe(16)
    expect(settings.settings.editor.fontSize).toBe(18)

    await settings.flushPending()
    const sent = ipc.calls('settings_save').at(-1).settings
    expect(sent.appearance.theme).toBe('light')
    expect(sent.appearance.uiFontSize).toBe(16)
    expect(sent.editor.fontSize).toBe(18)
  })

  it('system is a theme it accepts', async () => {
    await emit(settings.SETTINGS_APPLY, { theme: 'system' })
    await nextTick()
    expect(settings.settings.appearance.theme).toBe('system')
  })

  it('a value it cannot honour is skipped, and its neighbours still land', async () => {
    /* An event is not a response to anything: a malformed one has to cost
       nothing. Skipped rather than reset to the shipped default, too — the
       person did not ask for that either. */
    settings.settings.appearance.uiFontSize = 16
    await emit(settings.SETTINGS_APPLY, { theme: 'chartreuse', uiFontSize: 900, agent: 'codex' })
    await nextTick()

    expect(settings.settings.appearance.theme).toBe('dark')
    expect(settings.settings.appearance.uiFontSize).toBe(16)
    expect(settings.settings.agent).toBe('codex', 'the field beside them still arrived')
  })

  it('takes a language it is sent and keeps the one it holds when the value is not a language id', async () => {
    /* Rust owns the list of language ids (`agents::LANGUAGES`), so what is
       guarded here is the shape and nothing else: a string travels, and an id
       nobody ships is dropped on the way to the file. */
    await emit(settings.SETTINGS_APPLY, {
      agentLanguage: 'ru',
      taskLanguage: 'zh-Hans',
      commitLanguage: 'tr',
      reportLanguage: 'ko'
    })
    await nextTick()
    expect(settings.settings.agentLanguage).toBe('ru')
    expect(settings.settings.taskLanguage).toBe('zh-Hans')
    expect(settings.settings.commitLanguage).toBe('tr')
    expect(settings.settings.reportLanguage).toBe('ko')

    /* Skipped rather than reset to the shipped default, the same as every other
       field here: an event is not a response to anything, so a malformed one
       must cost nothing — and reverting to English would be a change nobody
       asked for. */
    await emit(settings.SETTINGS_APPLY, {
      agentLanguage: 7,
      taskLanguage: '',
      commitLanguage: null,
      reportLanguage: {}
    })
    await nextTick()
    expect(settings.settings.agentLanguage).toBe('ru')
    expect(settings.settings.taskLanguage).toBe('zh-Hans')
    expect(settings.settings.commitLanguage).toBe('tr')
    expect(settings.settings.reportLanguage).toBe('ko')
  })

  it('takes the board settings and cleans the two column lists on the way in', async () => {
    await emit(settings.SETTINGS_APPLY, {
      kanbanColumns: 'non-empty',
      kanbanInterval: 'week',
      kanbanAlwaysShow: ['ready', 'ready', '', 7, 'done'],
      kanbanUnlimited: ['blocked']
    })
    await nextTick()

    expect(settings.settings.kanban.columns).toBe('non-empty')
    expect(settings.settings.kanban.interval).toBe('week')
    expect(settings.settings.kanban.alwaysShow).toEqual(['ready', 'done'])
    expect(settings.settings.kanban.unlimited).toEqual(['blocked'])
  })

  it('keeps the board setting it holds when the value is off its closed list', async () => {
    /* The two scalars are checked against `boardView.js`'s lists, and Rust
       validates the file against its own copy — so what this guards is that a
       value neither of them would accept never becomes the board a person
       stares at. Skipped rather than reset, like every other field here. */
    await emit(settings.SETTINGS_APPLY, { kanbanColumns: 'non-empty', kanbanInterval: 'week' })
    await nextTick()

    await emit(settings.SETTINGS_APPLY, {
      kanbanColumns: 'some',
      kanbanInterval: 'fortnight',
      kanbanUnlimited: 'ready'
    })
    await nextTick()

    expect(settings.settings.kanban.columns).toBe('non-empty')
    expect(settings.settings.kanban.interval).toBe('week')
    expect(settings.settings.kanban.unlimited).toEqual([], 'a list that is not one is skipped')
  })

  /* The Editor tab's switch. A malformed event has to leave the previous value
     standing rather than fall back to the shipped default: an event is not a
     response to a request, so a broken one must cost nothing — and coercion
     would turn it into a deliberate-looking answer either way. */
  it('takes the word-wrap switch, and a bad value leaves the previous one', async () => {
    await emit(settings.SETTINGS_APPLY, { editorWordWrap: true })
    await nextTick()
    expect(settings.settings.editor.wordWrap).toBe(true)

    await emit(settings.SETTINGS_APPLY, { editorWordWrap: 'yes' })
    await nextTick()
    expect(settings.settings.editor.wordWrap).toBe(
      true,
      'a value that is not a boolean leaves the previous one, not the shipped default'
    )

    await emit(settings.SETTINGS_APPLY, { editorWordWrap: null })
    await nextTick()
    expect(settings.settings.editor.wordWrap).toBe(true)

    /* The size beside it rides the same message as a separate flat field and is
       untouched by any of this. */
    expect(settings.settings.editor.fontSize).toBe(12)

    await emit(settings.SETTINGS_APPLY, { editorWordWrap: false })
    await nextTick()
    expect(settings.settings.editor.wordWrap).toBe(false)
  })

  /* The one setting whose value the app acts on by opening a socket, so `false`
     has to reach the store from the settings window intact — and anything that
     is not a boolean has to be skipped rather than coerced, since coercion
     would turn a malformed event into a deliberate-looking "off". */
  it('takes the background fetch switch, and only a boolean one', async () => {
    await emit(settings.SETTINGS_APPLY, { gitAutoFetch: false })
    await nextTick()
    expect(settings.settings.git.autoFetch).toBe(false)

    await emit(settings.SETTINGS_APPLY, { gitAutoFetch: 'yes' })
    await nextTick()
    expect(settings.settings.git.autoFetch).toBe(false, 'a value that is not a boolean is skipped')

    await emit(settings.SETTINGS_APPLY, { gitAutoFetch: true })
    await nextTick()
    expect(settings.settings.git.autoFetch).toBe(true)
  })

  /* The Git tab's second switch, checked exactly as the first is. `false` is
     the whole point of this one — it is the position that changes what a run
     does — so it has to reach the store intact, and anything that is not a
     boolean has to leave the previous choice standing. */
  it('takes the worktree switch, and only a boolean one', async () => {
    await emit(settings.SETTINGS_APPLY, { gitRemoveWorktrees: false })
    await nextTick()
    expect(settings.settings.git.removeWorktrees).toBe(false)

    await emit(settings.SETTINGS_APPLY, { gitRemoveWorktrees: 'no' })
    await nextTick()
    expect(settings.settings.git.removeWorktrees).toBe(
      false,
      'a value that is not a boolean is skipped'
    )

    /* Its neighbour is untouched by any of it: the two ride one message as
       separate flat fields, and an edit to one must not carry the other. */
    expect(settings.settings.git.autoFetch).toBe(true)

    await emit(settings.SETTINGS_APPLY, { gitRemoveWorktrees: true })
    await nextTick()
    expect(settings.settings.git.removeWorktrees).toBe(true)
  })

  /* The switch that is not on that tab at all, and the one whose `false` the
     app acts on by leaving a window where the configuration puts it. Skipped
     rather than coerced for `gitAutoFetch`'s reason: coercion would turn a
     malformed event into a deliberate-looking "off", and off here is a visible
     change to where the app opens. */
  it('takes the window-geometry switch, and only a boolean one', async () => {
    await emit(settings.SETTINGS_APPLY, { restoreGeometry: false })
    await nextTick()
    expect(settings.settings.window.restoreGeometry).toBe(false)

    await emit(settings.SETTINGS_APPLY, { restoreGeometry: 'yes' })
    await nextTick()
    expect(settings.settings.window.restoreGeometry).toBe(
      false,
      'a value that is not a boolean is skipped'
    )

    await emit(settings.SETTINGS_APPLY, { restoreGeometry: null })
    await nextTick()
    expect(settings.settings.window.restoreGeometry).toBe(false)

    await emit(settings.SETTINGS_APPLY, { restoreGeometry: true })
    await nextTick()
    expect(settings.settings.window.restoreGeometry).toBe(true)
  })

  /* The switch over the update timer, checked the same way and for the same
     reason: coercing a malformed event into a deliberate-looking "off" would
     quietly stop somebody being told a release exists. */
  it('takes the update switch, and only a boolean one', async () => {
    await emit(settings.SETTINGS_APPLY, { updatesAutoCheck: false })
    await nextTick()
    expect(settings.settings.updates.autoCheck).toBe(false)

    await emit(settings.SETTINGS_APPLY, { updatesAutoCheck: 'yes' })
    await nextTick()
    expect(settings.settings.updates.autoCheck).toBe(
      false,
      'a value that is not a boolean is skipped'
    )

    await emit(settings.SETTINGS_APPLY, { updatesAutoCheck: null })
    await nextTick()
    expect(settings.settings.updates.autoCheck).toBe(false)

    await emit(settings.SETTINGS_APPLY, { updatesAutoCheck: true })
    await nextTick()
    expect(settings.settings.updates.autoCheck).toBe(true)
  })

  it('the update switch reaches the settings window as a flat field', async () => {
    settings.settings.updates.autoCheck = false

    expect(settings.sharedSettings().updatesAutoCheck).toBe(false)
  })

  /* The two sounds ride the same message as flat fields, named for their
     events. A value outside the closed list is skipped rather than reset: the
     person did not ask for the shipped default either. */
  it('takes a sound this build ships, and leaves the previous one standing otherwise', async () => {
    await emit(settings.SETTINGS_APPLY, { notificationRunFinished: 'sound-4' })
    await nextTick()
    expect(settings.settings.notifications.runFinished).toBe('sound-4')

    await emit(settings.SETTINGS_APPLY, { notificationRunFinished: 'sound-9' })
    await nextTick()
    expect(settings.settings.notifications.runFinished).toBe(
      'sound-4',
      'a sound nobody ships is skipped, not replaced by the default'
    )

    await emit(settings.SETTINGS_APPLY, { notificationRunFinished: null })
    await nextTick()
    expect(settings.settings.notifications.runFinished).toBe('sound-4')

    await emit(settings.SETTINGS_APPLY, { notificationNeedsAttention: 'off' })
    await nextTick()
    expect(settings.settings.notifications.needsAttention).toBe('off', 'off is a value it accepts')
  })

  it('both sounds reach the settings window as flat fields', async () => {
    settings.settings.notifications.runFinished = 'off'
    settings.settings.notifications.needsAttention = 'sound-3'

    const shared = settings.sharedSettings()

    expect(shared.notificationRunFinished).toBe('off')
    expect(shared.notificationNeedsAttention).toBe('sound-3')
  })

  /* The one condition on whether a finished run shows its report, checked the
     way the geometry switch above is: `false` is the whole point of it, so a
     malformed event is skipped rather than coerced into a deliberate-looking
     "off". */
  it('takes the report switch, and only a boolean one', async () => {
    await emit(settings.SETTINGS_APPLY, { notificationShowReport: false })
    await nextTick()
    expect(settings.settings.notifications.showReport).toBe(false)

    await emit(settings.SETTINGS_APPLY, { notificationShowReport: 'yes' })
    await nextTick()
    expect(settings.settings.notifications.showReport).toBe(
      false,
      'a value that is not a boolean is skipped'
    )

    await emit(settings.SETTINGS_APPLY, { notificationShowReport: null })
    await nextTick()
    expect(settings.settings.notifications.showReport).toBe(false)

    await emit(settings.SETTINGS_APPLY, { notificationShowReport: true })
    await nextTick()
    expect(settings.settings.notifications.showReport).toBe(true)
  })

  /* The switch over the two sounds, checked the same way and for the same
     reason: `false` is the whole point of it, so a malformed event is skipped
     rather than coerced into a deliberate-looking "off". */
  it('takes the focus switch, and only a boolean one', async () => {
    await emit(settings.SETTINGS_APPLY, { notificationOnlyWhenUnfocused: false })
    await nextTick()
    expect(settings.settings.notifications.onlyWhenUnfocused).toBe(false)

    await emit(settings.SETTINGS_APPLY, { notificationOnlyWhenUnfocused: 'yes' })
    await nextTick()
    expect(settings.settings.notifications.onlyWhenUnfocused).toBe(
      false,
      'a value that is not a boolean is skipped'
    )

    await emit(settings.SETTINGS_APPLY, { notificationOnlyWhenUnfocused: null })
    await nextTick()
    expect(settings.settings.notifications.onlyWhenUnfocused).toBe(false)

    await emit(settings.SETTINGS_APPLY, { notificationOnlyWhenUnfocused: true })
    await nextTick()
    expect(settings.settings.notifications.onlyWhenUnfocused).toBe(true)
  })

  it('the focus switch reaches the settings window as a flat field', async () => {
    settings.settings.notifications.onlyWhenUnfocused = false

    expect(settings.sharedSettings().notificationOnlyWhenUnfocused).toBe(false)
  })

  it('the report switch reaches the settings window as a flat field', async () => {
    settings.settings.notifications.showReport = false

    expect(settings.sharedSettings().notificationShowReport).toBe(false)
  })

  it('answers a hello with what this window holds, not with what is on disk', async () => {
    settings.settings.appearance.uiFontSize = 20
    const heard = []
    await listen(settings.SETTINGS_STATE, (event) => heard.push(event.payload))

    await emit(settings.SETTINGS_HELLO, null)
    await vi.waitFor(() => expect(heard).toHaveLength(1))

    expect(heard[0]).toEqual({
      theme: 'dark',
      density: 'comfortable',
      uiFontSize: 20,
      editorFontSize: 12,
      editorWordWrap: false,
      kanbanColumns: 'all',
      kanbanAlwaysShow: [],
      kanbanInterval: 'all',
      kanbanUnlimited: [],
      gitAutoFetch: true,
      gitRemoveWorktrees: true,
      restoreGeometry: true,
      updatesAutoCheck: true,
      notificationRunFinished: 'sound-1',
      notificationNeedsAttention: 'sound-2',
      notificationOnlyWhenUnfocused: true,
      notificationShowReport: true,
      agent: 'claude',
      agentLanguage: 'en',
      taskLanguage: 'en',
      commitLanguage: 'en',
      reportLanguage: 'en'
    })
  })

  it('announces the new truth after every edit, so a refused value is corrected', async () => {
    const heard = []
    await listen(settings.SETTINGS_STATE, (event) => heard.push(event.payload))

    await emit(settings.SETTINGS_APPLY, { uiFontSize: 900 })
    await vi.waitFor(() => expect(heard).toHaveLength(1))

    expect(heard[0].uiFontSize).toBe(13, 'what it actually holds, not what was asked for')
  })

  it('reads the file directly for the moment before this window has answered', async () => {
    ipc.on('settings_load', { appearance: { theme: 'light', uiFontSize: 15 }, agent: 'codex' })

    await expect(settings.readSharedSettings()).resolves.toEqual({
      theme: 'light',
      density: 'comfortable',
      uiFontSize: 15,
      editorFontSize: 12,
      editorWordWrap: false,
      kanbanColumns: 'all',
      kanbanAlwaysShow: [],
      kanbanInterval: 'all',
      kanbanUnlimited: [],
      gitAutoFetch: true,
      gitRemoveWorktrees: true,
      restoreGeometry: true,
      updatesAutoCheck: true,
      notificationRunFinished: 'sound-1',
      notificationNeedsAttention: 'sound-2',
      notificationOnlyWhenUnfocused: true,
      notificationShowReport: true,
      agent: 'codex',
      agentLanguage: 'en',
      taskLanguage: 'en',
      commitLanguage: 'en',
      reportLanguage: 'en'
    })
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
