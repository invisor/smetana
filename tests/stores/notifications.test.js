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
