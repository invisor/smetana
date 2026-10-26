import { describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

const split = (over = {}) => ({
  mode: 'client',
  state: 'connected',
  epoch: 1,
  attempt: 0,
  reason: null,
  local: true,
  server: { version: '0.1.0', protocolMajor: 1, protocolMinor: 9 },
  outdated: null,
  ...over
})

describe('the connection state', () => {
  it('reads as connected and not stale in the build with no server of its own', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('connection_state', { mode: 'browser', state: 'connected', epoch: 0 })
    await stores.connection.initConnection()
    expect(stores.connection.connection.mode).toBe('browser')
    expect(stores.connection.stale.value).toBe(false)
  })

  it('stays harmless when there is nobody to ask, as in a browser', async () => {
    const { ipc, stores } = await loadStores()
    ipc.fail('connection_state', new Error('no such command'))
    await stores.connection.initConnection()
    expect(stores.connection.connection.state).toBe('connected')
    expect(stores.connection.stale.value).toBe(false)
  })

  it('follows the shell through a loss and back, marking the data stale while it is down', async () => {
    const { ipc, emit, stores } = await loadStores()
    ipc.on('connection_state', split())
    await stores.connection.initConnection()
    expect(stores.connection.stale.value).toBe(false)

    await emit('connection:state', split({ state: 'reconnecting', reason: 'the server stopped' }))
    expect(stores.connection.connection.state).toBe('reconnecting')
    expect(stores.connection.connection.reason).toBe('the server stopped')
    expect(stores.connection.stale.value).toBe(true)

    await emit('connection:state', split({ epoch: 2 }))
    expect(stores.connection.stale.value).toBe(false)
  })

  it('knows every state the shell names as stale except connected', async () => {
    const { ipc, emit, stores } = await loadStores()
    ipc.on('connection_state', split())
    await stores.connection.initConnection()
    for (const state of ['connecting', 'reconnecting', 'incompatible', 'unpaired']) {
      await emit('connection:state', split({ state }))
      expect(stores.connection.stale.value, state).toBe(true)
    }
  })

  it('lets an event that arrived before the first read stand against it', async () => {
    const { ipc, emit, stores } = await loadStores()
    ipc.on('connection_state', async () => {
      await emit('connection:state', split({ epoch: 2 }))
      return split({ state: 'connecting', epoch: 0 })
    })
    await stores.connection.initConnection()
    expect(stores.connection.connection.epoch).toBe(2)
    expect(stores.connection.connection.state).toBe('connected')
  })

  it('ignores a view older than the one it holds', async () => {
    const { ipc, emit, stores } = await loadStores()
    ipc.on('connection_state', split({ epoch: 3 }))
    await stores.connection.initConnection()
    await emit('connection:state', split({ epoch: 2, state: 'reconnecting' }))
    expect(stores.connection.connection.state).toBe('connected')
  })
})

describe('resynchronising after a connection', () => {
  it('runs every registered read after a new connection and not for the same one twice', async () => {
    const { ipc, emit, stores } = await loadStores()
    const hook = vi.fn(async () => {})
    stores.connection.onReconnect(hook)
    ipc.on('connection_state', split())
    await stores.connection.initConnection()
    expect(hook, 'the app mounted onto a connection that was already up').not.toHaveBeenCalled()

    await emit('connection:state', split({ state: 'reconnecting' }))
    expect(hook).not.toHaveBeenCalled()
    await emit('connection:state', split({ epoch: 2 }))
    await vi.waitFor(() => expect(hook).toHaveBeenCalledTimes(1))
    await emit('connection:state', split({ epoch: 2, outdated: null }))
    expect(hook).toHaveBeenCalledTimes(1)
  })

  it('resyncs the first connection when the app mounted before it was made', async () => {
    const { ipc, emit, stores } = await loadStores()
    const hook = vi.fn(async () => {})
    stores.connection.onReconnect(hook)
    ipc.on('connection_state', split({ state: 'connecting', epoch: 0 }))
    await stores.connection.initConnection()
    await emit('connection:state', split({ epoch: 1 }))
    await vi.waitFor(() => expect(hook).toHaveBeenCalledTimes(1))
  })

  it('never resyncs in the build with no server of its own', async () => {
    const { ipc, emit, stores } = await loadStores()
    const hook = vi.fn(async () => {})
    stores.connection.onReconnect(hook)
    ipc.on('connection_state', { mode: 'browser', state: 'connected', epoch: 0 })
    await stores.connection.initConnection()
    await emit('connection:state', { mode: 'browser', state: 'connected', epoch: 1 })
    expect(hook).not.toHaveBeenCalled()
  })

  it('lets one failing read leave the others to run', async () => {
    const { ipc, emit, stores } = await loadStores()
    const error = vi.spyOn(console, 'error').mockImplementation(() => {})
    const good = vi.fn(async () => {})
    stores.connection.onReconnect(async () => {
      throw new Error('boom')
    })
    stores.connection.onReconnect(good)
    ipc.on('connection_state', split({ state: 'connecting', epoch: 0 }))
    await stores.connection.initConnection()
    await emit('connection:state', split())
    await vi.waitFor(() => expect(good).toHaveBeenCalled())
    error.mockRestore()
  })

  it('reads the board again through the tracker store after a new connection', async () => {
    const { ipc, emit, stores } = await loadStores()
    ipc.on('connection_state', split())
    ipc.on('tracker_resync', { issues: [], generation: 4, health: null, project: null })
    await stores.connection.initConnection()
    await emit('connection:state', split({ epoch: 2 }))
    await vi.waitFor(() => expect(ipc.calls('tracker_resync')).toHaveLength(1))
  })
})

describe('the commands the strip presses', () => {
  it('asks to connect now', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('connection_retry', null)
    await stores.connection.retryConnection()
    expect(ipc.calls('connection_retry')).toHaveLength(1)
  })

  it('confirms a restart with the list the person was shown, and only that', async () => {
    const { ipc, emit, stores } = await loadStores()
    const work = [{ kind: 'run', id: 'r1', project: '/p', label: 'a run', owner: null }]
    ipc.on('connection_state', split({ outdated: { running: '0.0.9', bundled: '0.1.0', work, canRestart: true } }))
    ipc.on('server_restart', { outcome: 'restarting', scheduled: false, work })
    await stores.connection.initConnection()
    await stores.connection.restartServer('now')
    expect(ipc.calls('server_restart')).toEqual([{ mode: 'now', confirmed: work }])
    await emit('connection:state', split())
  })

  it('remembers that a restart is waiting for the work to finish, and that it was withdrawn', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('connection_state', split({ outdated: { running: '0.0.9', bundled: '0.1.0', work: [], canRestart: true } }))
    ipc.on('server_restart', ({ mode }) => ({ outcome: mode === 'cancel' ? 'cancelled' : 'scheduled', scheduled: true, work: [] }))
    await stores.connection.initConnection()
    await stores.connection.restartServer('when-idle')
    expect(stores.connection.connection.restartScheduled).toBe(true)
    expect(ipc.calls('server_restart')[0].confirmed).toBe(null)
    await stores.connection.restartServer('cancel')
    expect(stores.connection.connection.restartScheduled).toBe(false)
  })
})

describe('which server this device uses', () => {
  it('reads a server on another machine as remote and everything else as local', async () => {
    const { ipc, emit, stores } = await loadStores()
    ipc.on('connection_state', split({ local: true }))
    await stores.connection.initConnection()
    expect(stores.connection.isRemote.value).toBe(false)

    await emit('connection:state', split({ local: false, epoch: 2 }))
    expect(stores.connection.isRemote.value).toBe(true)

    await emit('connection:state', split({ local: true, epoch: 3 }))
    expect(stores.connection.isRemote.value).toBe(false)
  })

  it('never reads a browser as remote', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('connection_state', { mode: 'browser', state: 'connected', epoch: 0, local: false })
    await stores.connection.initConnection()
    expect(stores.connection.isRemote.value).toBe(false)
  })

  it('answers the shell\'s target, and a browser where there is nobody to ask', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('server_target', { target: { kind: 'remote', node: 'abc', relay: 'https://r.example' } })
    expect(await stores.connection.readServerTarget()).toEqual({ kind: 'remote', node: 'abc', relay: 'https://r.example' })
    ipc.fail('server_target', new Error('no such command'))
    expect(await stores.connection.readServerTarget()).toEqual({ kind: 'browser' })
  })

  it('lists the local server\'s work, and an empty list when it cannot be asked', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('server_local_work', [{ kind: 'run', id: 1, label: 'run', project: '/p', owner: null }])
    expect(await stores.connection.localServerWork()).toHaveLength(1)
    ipc.fail('server_local_work', 'nope')
    expect(await stores.connection.localServerWork()).toEqual([])
  })

  it('switches to a remote server with the ticket and the stop choice, and says what the shell answered', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('server_use_remote', { target: { kind: 'remote', node: 'abc', relay: 'r' }, localStop: 'scheduled' })
    const answer = await stores.connection.useRemoteServer('smetana2:abc', true)
    expect(ipc.calls('server_use_remote')).toEqual([{ ticket: 'smetana2:abc', stopLocal: true }])
    expect(answer.localStop).toBe('scheduled')
  })

  it('rejects with the shell\'s words when pairing is refused, and the same for going back', async () => {
    const { ipc, stores } = await loadStores()
    ipc.fail('server_use_remote', 'the server refused the pairing: This code has expired.')
    await expect(stores.connection.useRemoteServer('smetana2:abc', false)).rejects.toThrow(/expired/)
    ipc.fail('server_use_local', new Error('The connection has not been set up.'))
    await expect(stores.connection.useLocalServer()).rejects.toThrow(/not been set up/)
  })
})

describe('a request to choose another server goes through the app window', () => {
  it('carries the request to the app window, which answers with the shell\'s reply', async () => {
    const { emit: _unused, stores } = await loadStores()
    const seen = []
    await stores.connection.watchServerSwitch(async (request) => {
      seen.push(request)
      return { target: { kind: 'remote' }, localStop: 'scheduled' }
    })
    const answer = await stores.connection.requestServerSwitch({ kind: 'remote', ticket: 'smetana2:abc', stopLocal: true })
    expect(seen).toEqual([{ kind: 'remote', ticket: 'smetana2:abc', stopLocal: true }])
    expect(answer.localStop).toBe('scheduled')
  })

  it('answers a cancel as a cancel and a refusal as a refusal', async () => {
    const { stores } = await loadStores()
    let next = { cancelled: true }
    await stores.connection.watchServerSwitch(async () => {
      if (next instanceof Error) throw next
      return next
    })
    expect(await stores.connection.requestServerSwitch({ kind: 'local' })).toEqual({ cancelled: true })
    next = new Error('the server refused the pairing: spent')
    await expect(stores.connection.requestServerSwitch({ kind: 'local' })).rejects.toThrow(/spent/)
  })

  it('says at once when there is no app window to answer', async () => {
    vi.useFakeTimers()
    try {
      const { stores } = await loadStores()
      const pending = stores.connection.requestServerSwitch({ kind: 'local' })
      const caught = pending.catch((err) => err)
      await vi.advanceTimersByTimeAsync(3500)
      expect((await caught).message).toMatch(/did not answer/)
    } finally {
      vi.useRealTimers()
    }
  })
})
