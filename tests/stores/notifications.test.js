import { describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

const MIB = 1024 * 1024
const PROJECT = '/Users/you/Projects/smetana'

/* One project's share of the store, in Rust's own shape. `kept` and `removable`
   together are the folder the clean-up button reaches, which is the number the
   ladder is weighed against. */
const survey = (bytes, over = {}) => ({
  store: { files: 40, bytes: bytes + 9 * MIB },
  project: PROJECT,
  board: 'ok',
  kept: { files: 2, bytes },
  removable: { files: 0, bytes: 0 },
  ...over
})

/* The store reads the active project out of the settings, so a test has to open
   one the way the app does. */
const openOn = (stores, project = PROJECT) => {
  stores.settings.settings.activeProject = project
  stores.settings.settings.openProjects = [project]
}

describe('the bell and the attachment store', () => {
  it('says nothing about a folder under every threshold', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    ipc.on('attachments_survey', () => survey(4 * MIB))

    await stores.notifications.measureStorage(PROJECT)

    expect(stores.notifications.notificationsState.items).toEqual([])
    expect(stores.settings.settings.project.storageWarnedMib).toBe(null)
  })

  it('announces a crossing once and stays quiet on every later measurement', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    ipc.on('attachments_survey', () => survey(12 * MIB))

    await stores.notifications.measureStorage(PROJECT)
    const [card] = stores.notifications.notificationsState.items
    expect(card.source).toBe('storage')
    expect(card.threshold).toBe(10)
    expect(stores.settings.settings.project.storageWarnedMib).toBe(10)

    // The same folder, weighed again: the card that is already there stands,
    // and no second one joins it.
    await stores.notifications.measureStorage(PROJECT)
    expect(stores.notifications.notificationsState.items).toHaveLength(1)

    // And a restart — a fresh graph reading the same remembered number — says
    // nothing at all.
    const second = await loadStores()
    openOn(second.stores)
    second.stores.settings.settings.project.storageWarnedMib = 10
    second.ipc.on('attachments_survey', () => survey(12 * MIB))
    await second.stores.notifications.measureStorage(PROJECT)
    expect(second.stores.notifications.notificationsState.items).toEqual([])
  })

  it('announces 50 and then 100 once each', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    let bytes = 12 * MIB
    ipc.on('attachments_survey', () => survey(bytes))

    await stores.notifications.measureStorage(PROJECT)
    expect(stores.notifications.notificationsState.items[0].threshold).toBe(10)

    bytes = 60 * MIB
    await stores.notifications.measureStorage(PROJECT)
    expect(stores.notifications.notificationsState.items).toHaveLength(1)
    expect(stores.notifications.notificationsState.items[0].threshold).toBe(50)
    expect(stores.settings.settings.project.storageWarnedMib).toBe(50)

    bytes = 120 * MIB
    await stores.notifications.measureStorage(PROJECT)
    expect(stores.notifications.notificationsState.items[0].threshold).toBe(100)
    expect(stores.settings.settings.project.storageWarnedMib).toBe(100)

    bytes = 400 * MIB
    await stores.notifications.measureStorage(PROJECT)
    expect(stores.notifications.notificationsState.items[0].threshold).toBe(100)
  })

  it('rewrites a standing card from the size just measured', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    let bytes = 12 * MIB
    ipc.on('attachments_survey', () => survey(bytes))

    await stores.notifications.measureStorage(PROJECT)
    expect(stores.notifications.notificationsState.items[0].body).toContain('12.0 MiB')

    // Grown, but not as far as the next step: the same card, still announced at
    // 10, saying what the folder weighs now rather than what it weighed then.
    bytes = 40 * MIB
    await stores.notifications.measureStorage(PROJECT)
    const [card] = stores.notifications.notificationsState.items
    expect(stores.notifications.notificationsState.items).toHaveLength(1)
    expect(card.threshold).toBe(10)
    expect(card.body).toContain('40.0 MiB')
  })

  it('takes the card away when the folder is cleaned, and arms the ladder again', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    let bytes = 12 * MIB
    ipc.on('attachments_survey', () => survey(bytes))

    await stores.notifications.measureStorage(PROJECT)
    expect(stores.notifications.notificationsState.items).toHaveLength(1)

    bytes = 3 * MIB
    await stores.notifications.measureStorage(PROJECT)
    expect(stores.notifications.notificationsState.items).toEqual([])
    expect(stores.settings.settings.project.storageWarnedMib).toBe(null)

    bytes = 12 * MIB
    await stores.notifications.measureStorage(PROJECT)
    expect(stores.notifications.notificationsState.items[0].threshold).toBe(10)
  })

  it('dismissing is the same write as announcing, so nothing comes back', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    ipc.on('attachments_survey', () => survey(60 * MIB))

    await stores.notifications.measureStorage(PROJECT)
    const [card] = stores.notifications.notificationsState.items
    stores.notifications.dismiss(card.id)

    expect(stores.notifications.notificationsState.items).toEqual([])
    expect(stores.settings.settings.project.storageWarnedMib).toBe(50)

    await stores.notifications.measureStorage(PROJECT)
    expect(stores.notifications.notificationsState.items).toEqual([])
  })

  it('changes nothing at all while the board cannot be read', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    let board = 'ok'
    ipc.on('attachments_survey', () => survey(60 * MIB, { board, kept: { files: 0, bytes: 0 } }))

    // With the board unreadable the survey counts nothing by design, so there
    // is no size to announce — and, just as importantly, none to re-arm the
    // ladder from: taking that zero as a measurement would announce the same
    // threshold a second time the moment the board came back.
    board = 'error'
    await stores.notifications.measureStorage(PROJECT)
    expect(stores.notifications.notificationsState.items).toEqual([])
    expect(stores.settings.settings.project.storageWarnedMib).toBe(null)

    stores.settings.settings.project.storageWarnedMib = 50
    await stores.notifications.measureStorage(PROJECT)
    expect(stores.settings.settings.project.storageWarnedMib).toBe(50)
    expect(stores.notifications.notificationsState.items).toEqual([])
  })

  it('is about the active project only, and never writes under another name', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    // The answer names the project it was measured for, and it is not this one:
    // the tracker worker is still pointing at the project just left.
    ipc.on('attachments_survey', () => survey(60 * MIB, { project: '/Users/you/Projects/other' }))

    await stores.notifications.measureStorage(PROJECT)

    expect(stores.notifications.notificationsState.items).toEqual([])
    expect(stores.settings.settings.project.storageWarnedMib).toBe(null)
  })

  it('drops a card belonging to the project just left', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    ipc.on('attachments_survey', () => survey(60 * MIB))
    await stores.notifications.measureStorage(PROJECT)
    expect(stores.notifications.notificationsState.items).toHaveLength(1)

    // Moving on: the neighbouring project's own folder is small, and the card
    // about the previous one must not be left standing under its name.
    const other = '/Users/you/Projects/other'
    openOn(stores, other)
    ipc.on('attachments_survey', () => survey(2 * MIB, { project: other }))
    await stores.notifications.measureStorage(other)

    expect(stores.notifications.notificationsState.items).toEqual([])
  })

  it('says nothing new when the store could not be read at all', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.on('attachments_survey', () => survey(12 * MIB))
    await stores.notifications.measureStorage(PROJECT)

    ipc.fail('attachments_survey', { kind: 'noStore', message: 'no app data directory' })
    await stores.notifications.measureStorage(PROJECT)

    // The card that was true a moment ago is left alone: a failed read is not
    // evidence that the folder shrank.
    expect(stores.notifications.notificationsState.items).toHaveLength(1)
    expect(stores.settings.settings.project.storageWarnedMib).toBe(10)
  })

  it('has nothing to say with no project open', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    ipc.on('attachments_survey', () => survey(12 * MIB))
    await stores.notifications.measureStorage(PROJECT)

    stores.settings.settings.activeProject = null
    await stores.notifications.measureStorage(null)

    expect(stores.notifications.notificationsState.items).toEqual([])
    expect(ipc.calls('attachments_survey')).toHaveLength(1)
  })
})

/* A run the worker has finished with, whole, the way `run_state` answers. */
const finished = (token, over = {}) => ({
  token,
  project: PROJECT,
  settings: { scope: { kind: 'queue' }, target_branch: 'develop' },
  state: { kind: 'stopped', reason: { kind: 'queue_empty' } },
  session: null,
  batches: 1,
  stopping: false,
  summary: {
    seconds: 840,
    tasks: { closed: [{ id: 'a-1', title: 'One' }], parked: [] },
    report: `${PROJECT}/.smetana/reports/2026-08-12-143155.html`
  },
  ...over
})

describe('the bell and a run that is over', () => {
  it('puts a card up for each stopped run, and the badge counts them', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    ipc.on('project_config', { state: 'ok', config: {} })
    ipc.on('run_state', [finished(1), finished(2, { token: 2 })])

    await stores.runs.loadConfig(PROJECT)
    await stores.runs.loadRun(PROJECT)

    const items = stores.notifications.notificationsState.items
    // The badge is this length, so two runs is a bell reading 2.
    expect(items).toHaveLength(2)
    expect(items.map((item) => item.id)).toEqual(['run:1', 'run:2'])
    expect(items[0].body).toContain('1 closed')
    expect(items[0].actionLabel).toBe('Show details')
  })

  it('says nothing about a run that is still going', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    ipc.on('project_config', { state: 'ok', config: {} })
    ipc.on('run_state', [finished(1, { state: { kind: 'working', iteration: 0 } })])

    await stores.runs.loadConfig(PROJECT)
    await stores.runs.loadRun(PROJECT)

    expect(stores.notifications.notificationsState.items).toEqual([])
  })

  it('takes a dismissed card away for good, however often the list is read again', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    ipc.on('project_config', { state: 'ok', config: {} })
    ipc.on('run_state', [finished(1)])

    await stores.runs.loadConfig(PROJECT)
    await stores.runs.loadRun(PROJECT)
    stores.notifications.dismiss('run:1')
    expect(stores.notifications.notificationsState.items).toEqual([])

    // The card is derived from a list that outlives it, so the whole of
    // "dismissed" is the remembered token: without it the next read of the
    // same list would put the card straight back.
    await stores.runs.loadRun(PROJECT)
    stores.notifications.syncRunCards()
    expect(stores.notifications.notificationsState.items).toEqual([])
  })

  it('makes no card for a run whose report was put in front of the person', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    ipc.on('project_config', { state: 'ok', config: {} })
    ipc.on('run_state', [finished(1)])

    await stores.runs.loadConfig(PROJECT)
    await stores.runs.loadRun(PROJECT)
    expect(stores.notifications.notificationsState.items).toHaveLength(1)

    // Delivery is one of the three, never two: a tab already open in front of
    // somebody is the visit the card would have been asking for, and a person
    // who switched reports off has declined that visit in advance.
    stores.notifications.markRunDelivered(1)
    expect(stores.notifications.notificationsState.items).toEqual([])

    // And it stays gone, for the reason a dismissed one does: the list it is
    // derived from is read again on every focus and every project switch.
    await stores.runs.loadRun(PROJECT)
    expect(stores.notifications.notificationsState.items).toEqual([])
  })

  it('takes the card away with the project it was made in', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    ipc.on('project_config', { state: 'ok', config: {} })
    ipc.on('run_state', [finished(1)])

    await stores.runs.loadConfig(PROJECT)
    await stores.runs.loadRun(PROJECT)
    expect(stores.notifications.notificationsState.items).toHaveLength(1)

    // Switching project empties the run list, and a card about work in a folder
    // this window has left must not stand under the new project's name.
    const other = '/Users/you/Projects/other'
    openOn(stores, other)
    await stores.runs.loadConfig(other)

    expect(stores.notifications.notificationsState.items).toEqual([])
  })

  it('leaves the other sources\' cards exactly where they were', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    ipc.on('attachments_survey', () => survey(12 * MIB))
    ipc.on('project_config', { state: 'ok', config: {} })
    ipc.on('run_state', [finished(1)])

    await stores.notifications.measureStorage(PROJECT)
    await stores.runs.loadConfig(PROJECT)
    await stores.runs.loadRun(PROJECT)

    expect(stores.notifications.notificationsState.items).toHaveLength(2)

    // And dismissing one of them is no statement about the other.
    stores.notifications.dismiss('run:1')
    expect(stores.notifications.notificationsState.items.map((item) => item.source)).toEqual([
      'storage'
    ])
  })

  /* The order is a property of the list, not of whichever source last had
     something to say — so it is checked from both directions rather than in the
     one sequence that happens to produce it. Before this, each writer arranged
     its own half: the storage card was prepended when announced and run cards
     were put in front when the list moved, and the panel's order was therefore
     whatever had happened most recently. */
  it('draws its sources in one order however the cards got there', async () => {
    const runsThenStorage = await loadStores()
    openOn(runsThenStorage.stores)
    runsThenStorage.ipc.on('attachments_survey', () => survey(12 * MIB))
    runsThenStorage.ipc.on('project_config', { state: 'ok', config: {} })
    runsThenStorage.ipc.on('run_state', [finished(1)])
    await runsThenStorage.stores.runs.loadConfig(PROJECT)
    await runsThenStorage.stores.runs.loadRun(PROJECT)
    await runsThenStorage.stores.notifications.measureStorage(PROJECT)

    const storageThenRuns = await loadStores()
    openOn(storageThenRuns.stores)
    storageThenRuns.ipc.on('attachments_survey', () => survey(12 * MIB))
    storageThenRuns.ipc.on('project_config', { state: 'ok', config: {} })
    storageThenRuns.ipc.on('run_state', [finished(1)])
    await storageThenRuns.stores.notifications.measureStorage(PROJECT)
    await storageThenRuns.stores.runs.loadConfig(PROJECT)
    await storageThenRuns.stores.runs.loadRun(PROJECT)

    const sources = (graph) =>
      graph.stores.notifications.notificationsState.items.map((item) => item.source)
    expect(sources(runsThenStorage)).toEqual(['run', 'storage'])
    expect(sources(storageThenRuns)).toEqual(sources(runsThenStorage))
  })
})

describe('the bell and an update that is waiting', () => {
  /* The state as `updates_state` answers it and as `updates:state` carries it —
     the same value either way, which is the whole point of the machine handing
     itself over as one tagged thing. */
  const READY = { kind: 'ready', version: '0.2.0' }

  it('shows exactly one card while an update is ready, and none in any other state', async () => {
    const { ipc, emit, stores } = await loadStores()
    ipc.on('updates_state', { kind: 'idle' })

    await stores.updates.initUpdates()
    expect(stores.notifications.notificationsState.items).toEqual([])

    // Checking and downloading are not news: the app fetches quietly and the
    // bell says nothing until there is something to press.
    await emit('updates:state', { kind: 'checking' })
    expect(stores.notifications.notificationsState.items).toEqual([])
    await emit('updates:state', { kind: 'available', version: '0.2.0' })
    expect(stores.notifications.notificationsState.items).toEqual([])
    await emit('updates:state', { kind: 'downloading', received: 4, total: 9 })
    expect(stores.notifications.notificationsState.items).toEqual([])

    await emit('updates:state', READY)
    const items = stores.notifications.notificationsState.items
    // The badge is this length, so one waiting update is a bell reading 1.
    expect(items).toHaveLength(1)
    expect(items[0].source).toBe('update')
    expect(items[0].id).toBe('update:0.2.0')

    // A check that could not reach GitHub is not something to interrupt
    // anybody with; it belongs on About, where a person went looking.
    await emit('updates:state', { kind: 'failed', message: 'the feed timed out' })
    expect(stores.notifications.notificationsState.items).toEqual([])
  })

  it('draws a download already in progress when the window opens mid-flight', async () => {
    // The state is the app's, not the window's: whatever was going on before
    // this store existed is what the first read answers.
    const { ipc, stores } = await loadStores()
    ipc.on('updates_state', { kind: 'downloading', received: 12, total: 48 })

    await stores.updates.initUpdates()

    expect(stores.updates.updatesState.state).toEqual({ kind: 'downloading', received: 12, total: 48 })
    expect(stores.notifications.notificationsState.items).toEqual([])
  })

  it('takes the card away once the update stops being one', async () => {
    const { ipc, emit, stores } = await loadStores()
    ipc.on('updates_state', READY)

    await stores.updates.initUpdates()
    expect(stores.notifications.notificationsState.items).toHaveLength(1)

    // An installed update leaves nothing behind: the app restarts and the next
    // machine starts at idle. Nothing about the card is written anywhere, so a
    // restart is this and nothing more.
    await emit('updates:state', { kind: 'idle' })
    expect(stores.notifications.notificationsState.items).toEqual([])

    const afterRestart = await loadStores()
    afterRestart.ipc.on('updates_state', { kind: 'idle' })
    await afterRestart.stores.updates.initUpdates()
    expect(afterRestart.stores.notifications.notificationsState.items).toEqual([])
  })

  it('keeps a dismissed card away, and says nothing about the next version', async () => {
    const { ipc, emit, stores } = await loadStores()
    ipc.on('updates_state', READY)

    await stores.updates.initUpdates()
    stores.notifications.dismiss('update:0.2.0')
    expect(stores.notifications.notificationsState.items).toEqual([])

    // The same state announced again — Rust re-emitting, or a second read —
    // does not put it back.
    await emit('updates:state', READY)
    expect(stores.notifications.notificationsState.items).toEqual([])

    // A different release is a different statement, and it speaks.
    await emit('updates:state', { kind: 'ready', version: '0.3.0' })
    expect(stores.notifications.notificationsState.items.map((item) => item.id)).toEqual([
      'update:0.3.0'
    ])
  })

  it('says nothing at all where there is nobody to ask', async () => {
    // A browser: `mockBackend.js` answers this read with null rather than
    // refusing it, so nothing reaches the console either.
    const { ipc, stores } = await loadStores()
    ipc.on('updates_state', null)

    await stores.updates.initUpdates()

    expect(stores.updates.updatesState.state).toBe(null)
    expect(stores.notifications.notificationsState.items).toEqual([])
  })

  it('counts beside the other two sources, and sits between them', async () => {
    const { ipc, stores } = await loadStores()
    openOn(stores)
    ipc.on('updates_state', READY)
    ipc.on('attachments_survey', () => survey(12 * MIB))
    ipc.on('project_config', { state: 'ok', config: {} })
    ipc.on('run_state', [finished(1)])

    await stores.notifications.measureStorage(PROJECT)
    await stores.updates.initUpdates()
    await stores.runs.loadConfig(PROJECT)
    await stores.runs.loadRun(PROJECT)

    const items = stores.notifications.notificationsState.items
    // Three cards is a bell reading 3, and the order is the list's own rather
    // than the order the three sources happened to speak in.
    expect(items).toHaveLength(3)
    expect(items.map((item) => item.source)).toEqual(['run', 'update', 'storage'])
  })
})
