import { describe, expect, it } from 'vitest'
import { loadStores } from '../support/stores.js'

const OK = { state: 'ok', config: { project: { repos: ['.'] } } }

describe('the active project\'s run configuration', () => {
  it('a configured project needs no setup', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)

    await stores.runs.loadConfig('/p')

    expect(stores.runs.runsState.config.state).toBe('ok')
    expect(stores.runs.needsSetup.value).toBe(false)
    expect(stores.runs.configError.value).toBe(null)
    expect(ipc.calls('project_config')).toEqual([{ project: '/p' }])
  })

  it('a project with no file needs setup, and that is not an error', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', { state: 'missing' })

    await stores.runs.loadConfig('/p')

    expect(stores.runs.needsSetup.value).toBe(true)
    // Missing is the ordinary case: every project starts here, and nothing
    // about it belongs in a toast.
    expect(stores.runs.configError.value).toBe(null)
  })

  it('a damaged file is an error, and not an invitation to overwrite it', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', { state: 'broken', message: 'unknown field `gate`' })

    await stores.runs.loadConfig('/p')

    expect(stores.runs.configError.value).toContain('gate')
    // The setup dialog must not be offered for a file that exists: the agent
    // would write over something the person cannot currently read.
    expect(stores.runs.needsSetup.value).toBe(false)
  })

  it('with no project there is nothing to ask about', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    await stores.runs.loadConfig('/p')

    await stores.runs.loadConfig(null)

    expect(stores.runs.needsSetup.value).toBe(false)
    expect(stores.runs.runsState.config.state).toBe('missing')
    expect(ipc.calls('project_config')).toEqual([{ project: '/p' }])
  })

  it('a response for the project we already left is dropped', async () => {
    // The same guard git.js and terminals.js carry: two calls in flight have no
    // ordering guarantee, and without this the last response would win rather
    // than the last call — one project's configuration under another's name.
    // Resolved by hand, the way git.test.js does it, so the /slow call's
    // answer genuinely arrives after /fast's rather than merely being
    // *invoked* first: a mock that just resolves in call order would let this
    // pass even with the guard deleted.
    const { stores } = await loadStores()
    const pending = new Map()
    const { mockIPC } = await import('@tauri-apps/api/mocks')
    mockIPC((cmd, args) => new Promise((resolve) => pending.set(args.project, resolve)))

    const slow = stores.runs.loadConfig('/slow')
    const fast = stores.runs.loadConfig('/fast')

    pending.get('/fast')({ state: 'missing' })
    await fast
    pending.get('/slow')(OK)
    await slow

    expect(stores.runs.runsState.project).toBe('/fast')
    expect(stores.runs.runsState.config.state).toBe('missing')
  })

  it('a failed command leaves no stale configuration behind', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    await stores.runs.loadConfig('/p')

    ipc.fail('project_config', new Error('nope'))
    await stores.runs.loadConfig('/other')

    expect(stores.runs.runsState.config.state).toBe('missing')
  })
})

const RUN = {
  project: '/p',
  settings: {
    scope: { kind: 'queue' },
    mode: 'auto',
    target_branch: 'main',
    min_priority: 2,
    live_check: true,
    file_findings: true
  },
  state: { kind: 'working', iteration: 0 },
  session: 4,
  batches: 1,
  stopping: false
}

describe('the run in the active project', () => {
  it('starting one puts it in the store and hands it back', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_start', RUN)
    await stores.runs.loadConfig('/p')

    const started = await stores.runs.startRun('/p', RUN.settings)

    expect(started).toEqual(RUN)
    expect(stores.runs.runsState.run).toEqual(RUN)
    expect(stores.runs.running.value).toBe(true)
    // Passed through untouched: this is the shape Rust deserializes, and
    // translating the field names here would put them in two places.
    expect(ipc.calls('run_start')).toEqual([{ project: '/p', settings: RUN.settings }])
  })

  it('a refusal reaches the caller instead of being swallowed', async () => {
    // The dialog is the only thing that can say which of its own fields is
    // wrong, so a run that would not start has to throw rather than resolve.
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    await stores.runs.loadConfig('/p')
    ipc.fail('run_start', { kind: 'broken_config', detail: 'unknown field `gate`' })

    await expect(stores.runs.startRun('/p', RUN.settings)).rejects.toBeTruthy()
    expect(stores.runs.runsState.run).toBe(null)
  })

  it('a stopped run stays on screen, because its reason is what there is to read', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_start', RUN)
    await stores.runs.loadConfig('/p')
    await stores.runs.startRun('/p', RUN.settings)

    ipc.on('run_stop', { ...RUN, state: { kind: 'stopped', reason: { kind: 'cancelled' } }, session: null })
    await stores.runs.stopRun('/p')

    expect(stores.runs.runsState.run).not.toBe(null)
    expect(stores.runs.running.value).toBe(false)
  })

  it('a state event for another project is dropped', async () => {
    // An event is not a response to anything, so nothing orders it against a
    // project switch. Without the guard a batch ending just as somebody moves
    // away would show its run under the new project's name.
    const { emit, ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    await stores.runs.initRuns()
    await stores.runs.loadConfig('/p')

    await emit('run:state', { ...RUN, project: '/elsewhere' })

    expect(stores.runs.runsState.run).toBe(null)

    await emit('run:state', RUN)
    expect(stores.runs.runsState.run).toEqual(RUN)
  })

  it('switching projects takes the run with it', async () => {
    // Cleared where the project moves, not left for loadRun to overwrite: for
    // as long as that call takes, the old run would be on screen under the new
    // project's name.
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_start', RUN)
    await stores.runs.loadConfig('/p')
    await stores.runs.startRun('/p', RUN.settings)

    await stores.runs.loadConfig('/other')

    expect(stores.runs.runsState.run).toBe(null)
  })

  it('nothing running is a null the panel can draw, not a failure', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_state', null)
    await stores.runs.loadConfig('/p')

    await stores.runs.loadRun('/p')

    expect(stores.runs.runsState.run).toBe(null)
    expect(stores.runs.running.value).toBe(false)
  })
})
