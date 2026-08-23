import { describe, expect, it } from 'vitest'
import { loadStores } from '../support/stores.js'

describe('the update machine, mirrored', () => {
  it('reads the state whole, in Rust’s own shape', async () => {
    const { ipc, stores } = await loadStores()
    const state = { kind: 'available', version: '0.2.0', notes: 'Fixes.', date: '2026-08-20' }
    ipc.on('updates_state', state)

    await stores.updates.initUpdates()

    // Whole and unread: a field this front end has not heard of must not
    // silently read as one it has, and the rule that draws it is the one place
    // the tag is interpreted.
    expect(stores.updates.updatesState.state).toEqual(state)
  })

  it('reads an answer in a shape it does not know as nobody to ask', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('updates_state', { version: '0.2.0' })

    await stores.updates.initUpdates()

    expect(stores.updates.updatesState.state).toBe(null)
  })

  it('lets a state that arrived by event stand against the first read', async () => {
    // The subscription is made before the read, so the two can land in either
    // order — and an event that arrived first is the newer of the two. Without
    // the guard the read's answer would put the older picture back.
    const { ipc, emit, stores } = await loadStores()
    ipc.on('updates_state', async () => {
      await emit('updates:state', { kind: 'ready', version: '0.2.0' })
      return { kind: 'downloading', received: 40, total: 48 }
    })

    await stores.updates.initUpdates()

    expect(stores.updates.updatesState.state).toEqual({ kind: 'ready', version: '0.2.0' })
  })

  it('adopts what the check answers, since that is the state the machine is in', async () => {
    // `updates_check` never fails: a check that cannot start is answered with
    // the state that stopped it, and drawing that at once means no wait for an
    // event which is not coming.
    const { ipc, stores } = await loadStores()
    ipc.on('updates_state', { kind: 'idle' })
    ipc.on('updates_check', { kind: 'checking' })

    await stores.updates.initUpdates()
    await stores.updates.checkForUpdate()

    expect(stores.updates.updatesState.state).toEqual({ kind: 'checking' })
    expect(ipc.calls('updates_check')).toHaveLength(1)
  })

  it('hands a refused install back whole, for the window to put into words', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('updates_state', { kind: 'ready', version: '0.2.0' })
    ipc.fail('updates_install', { kind: 'run_live', detail: { projects: 'smetana' } })

    await stores.updates.initUpdates()

    await expect(stores.updates.installUpdate()).rejects.toMatchObject({
      kind: 'run_live',
      detail: { projects: 'smetana' }
    })
    // The refusal is not a new state: what was downloaded is still downloaded
    // and the press is still there to make again.
    expect(stores.updates.updatesState.state).toEqual({ kind: 'ready', version: '0.2.0' })
    expect(stores.notifications.notificationsState.items).toHaveLength(1)
  })
})
