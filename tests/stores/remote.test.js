import { describe, expect, it } from 'vitest'
import { loadStores } from '../support/stores.js'

const record = {
  key: 'ab12',
  name: 'Pixel 8',
  platform: 'android',
  paired_at: 1_760_000_000,
  last_contact: null
}

describe('the phone companion store', () => {
  it('reads the device list in camelCase and keeps the push credentials to itself', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('remote_devices', [record])

    const [device] = await stores.remote.listDevices()

    expect(device).toEqual({
      key: 'ab12',
      name: 'Pixel 8',
      platform: 'android',
      pairedAt: 1_760_000_000,
      lastContact: null
    })
    expect(Object.keys(device).sort()).toEqual(['key', 'lastContact', 'name', 'pairedAt', 'platform'])
  })

  it('drops a record with no key rather than drawing a row with no name', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('remote_devices', [record, { name: 'No key' }, null, 'junk'])

    expect(await stores.remote.listDevices()).toHaveLength(1)
  })

  it('reads anything that is not a list as an empty one', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('remote_devices', { oops: true })

    expect(await stores.remote.listDevices()).toEqual([])
  })

  it('begins a pairing and renames the expiry', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('remote_pair_begin', { ticket: 'smetana1:abc', expires_at: 1_760_000_120 })

    expect(await stores.remote.beginPairing('https://relay.example.org')).toEqual({
      ticket: 'smetana1:abc',
      expiresAt: 1_760_000_120
    })
  })

  /* The relay on screen is the relay used: an edit followed at once by the press
     must carry the new address, not whatever `settings.json` held a debounce ago. */
  it('passes the relay in the field with the press, straight after an edit', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('remote_pair_begin', { ticket: 'smetana1:abc', expires_at: 1_760_000_120 })

    stores.settings.applyPatch({ remoteRelayUrl: 'https://new.example.org' })
    await stores.remote.beginPairing(stores.settings.settings.remote.relayUrl)

    expect(ipc.calls('remote_pair_begin')).toEqual([{ relayUrl: 'https://new.example.org' }])
    expect(ipc.calls('settings_save')).toEqual([])
  })

  it('sends the answer as a strict boolean', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('remote_pair_decide', null)

    await stores.remote.decidePairing(true)
    await stores.remote.decidePairing('yes')

    expect(ipc.calls('remote_pair_decide').map((call) => call.accept)).toEqual([true, false])
  })

  it('revokes by key', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('remote_device_revoke', null)

    await stores.remote.revokeDevice('ab12')

    expect(ipc.calls('remote_device_revoke')).toEqual([{ key: 'ab12' }])
  })

  it('throws the back end’s refusal as a readable error', async () => {
    const { ipc, stores } = await loadStores()
    ipc.fail('remote_pair_begin', 'could not start the endpoint')

    await expect(stores.remote.beginPairing()).rejects.toThrow('could not start the endpoint')
  })

  it('hears a pairing request, and a change to the list', async () => {
    const { emit, stores, nextTick } = await loadStores()
    const requests = []
    const lists = []
    let ended = 0
    await stores.remote.watchPairEnded(() => ended++)
    await stores.remote.watchPairRequests((request) => requests.push(request))
    await stores.remote.watchDevices((list) => lists.push(list))

    await emit('remote:pair-request', { device_name: 'Pixel 8', platform: 'android' })
    await emit('remote:devices', [record])
    await emit('remote:pair-ended', null)
    await nextTick()
    expect(ended).toBe(1)

    expect(requests).toEqual([{ deviceName: 'Pixel 8', platform: 'android' }])
    expect(lists).toHaveLength(1)
    expect(lists[0][0].name).toBe('Pixel 8')
  })
})
