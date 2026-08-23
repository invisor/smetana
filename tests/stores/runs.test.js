import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

/* The DOM half of the sound, stood in for. Mocked here rather than at
   `HTMLMediaElement.prototype.play`, because the seam this store is answerable
   for is which sound it asks for — whether an element then plays is
   `chime.js`'s business and a browser's. The options object is forwarded too:
   whether the main window has focus is `chime.js`'s question, and what this
   store is answerable for is handing the current setting over. `vi.mock` is
   hoisted above the imports and survives the `vi.resetModules()` `loadStores`
   does. */
const chime = vi.fn()
vi.mock('../../src/chime.js', () => ({ chime: (id, options) => chime(id, options) }))

beforeEach(() => {
  chime.mockClear()
})

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
  token: 1,
  project: '/p',
  settings: {
    scope: { kind: 'queue' },
    mode: 'auto',
    target_branch: 'main',
    min_priority: 2,
    max_parallel_tasks: 3,
    live_check: true,
    file_findings: true
  },
  state: { kind: 'working', iteration: 0 },
  session: 4,
  batches: 1,
  stopping: false
}

/* A second run beside the first: same project, another scope, its own token —
   the shape the worker now allows and the store has to hold. */
const TASK_RUN = {
  ...RUN,
  token: 2,
  session: 9,
  settings: { ...RUN.settings, scope: { kind: 'task', id: 'smetana-5' }, min_priority: null }
}

describe('the runs in the active project', () => {
  it('starting one puts it in the store and hands it back', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_start', RUN)
    await stores.runs.loadConfig('/p')

    const started = await stores.runs.startRun('/p', RUN.settings)

    expect(started).toEqual(RUN)
    expect(stores.runs.runsState.runs).toEqual([RUN])
    // Passed through untouched: this is the shape Rust deserializes, and
    // translating the field names here would put them in two places.
    expect(ipc.calls('run_start')).toEqual([{ project: '/p', settings: RUN.settings }])
  })

  it('a second run of another scope goes beside the first, not over it', async () => {
    // The acceptance criterion on the store's side: two runs in one project at
    // once, each whole, each under its own token.
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_start', RUN)
    await stores.runs.loadConfig('/p')
    await stores.runs.startRun('/p', RUN.settings)

    ipc.on('run_start', TASK_RUN)
    await stores.runs.startRun('/p', TASK_RUN.settings)

    expect(stores.runs.runsState.runs).toEqual([RUN, TASK_RUN])
  })

  it('a refusal reaches the caller instead of being swallowed', async () => {
    // The dialog is the only thing that can say which of its own fields is
    // wrong, so a run that would not start has to throw rather than resolve.
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    await stores.runs.loadConfig('/p')
    ipc.fail('run_start', { kind: 'broken_config', detail: 'unknown field `gate`' })

    await expect(stores.runs.startRun('/p', RUN.settings)).rejects.toBeTruthy()
    expect(stores.runs.runsState.runs).toEqual([])
  })

  it('a stopped run stays on screen, because its reason is what there is to read', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_start', RUN)
    await stores.runs.loadConfig('/p')
    await stores.runs.startRun('/p', RUN.settings)

    ipc.on('run_stop', { ...RUN, state: { kind: 'stopped', reason: { kind: 'cancelled' } }, session: null })
    await stores.runs.stopRun(RUN.token)

    expect(stores.runs.runsState.runs).toHaveLength(1)
    expect(stores.runs.runsState.runs[0].state.kind).toBe('stopped')
    // And the stop went out under the run's own name, which is its token.
    expect(ipc.calls('run_stop')).toEqual([{ token: RUN.token }])
  })

  it('stopping one run leaves the other on screen and going', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_start', RUN)
    await stores.runs.loadConfig('/p')
    await stores.runs.startRun('/p', RUN.settings)
    ipc.on('run_start', TASK_RUN)
    await stores.runs.startRun('/p', TASK_RUN.settings)

    ipc.on('run_stop', { ...RUN, state: { kind: 'stopped', reason: { kind: 'cancelled' } }, session: null })
    await stores.runs.stopRun(RUN.token)

    const byToken = Object.fromEntries(stores.runs.runsState.runs.map((r) => [r.token, r.state.kind]))
    expect(byToken).toEqual({ 1: 'stopped', 2: 'working' })
  })

  it("a new run takes over its own scope's stopped slot and nobody else's", async () => {
    // The single-run store overwrote its one slot on every start; per scope is
    // that same behaviour now that there are several slots. A stopped run of
    // another scope keeps its place — its reason has not been read against
    // this start.
    const { emit, ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_start', RUN)
    await stores.runs.initRuns()
    await stores.runs.loadConfig('/p')
    await stores.runs.startRun('/p', RUN.settings)
    await emit('run:state', { ...TASK_RUN, state: { kind: 'stopped', reason: { kind: 'queue_empty' } }, session: null })
    await emit('run:state', { ...RUN, state: { kind: 'stopped', reason: { kind: 'crashed', attempts: 5 } }, session: null })

    const NEXT_QUEUE_RUN = { ...RUN, token: 3, batches: 0, state: { kind: 'preflight' } }
    ipc.on('run_start', NEXT_QUEUE_RUN)
    await stores.runs.startRun('/p', NEXT_QUEUE_RUN.settings)

    expect(stores.runs.runsState.runs.map((r) => r.token)).toEqual([TASK_RUN.token, 3])
  })

  it('an event lands on the run it names and only that one', async () => {
    // The stale-response guard per run: the token is what tells one run's
    // event from another's, so a batch ending in one run cannot rewrite its
    // neighbour.
    const { emit, ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_start', RUN)
    await stores.runs.initRuns()
    await stores.runs.loadConfig('/p')
    await stores.runs.startRun('/p', RUN.settings)
    ipc.on('run_start', TASK_RUN)
    await stores.runs.startRun('/p', TASK_RUN.settings)

    await emit('run:state', { ...TASK_RUN, state: { kind: 'deciding' }, session: null })

    expect(stores.runs.runsState.runs.find((r) => r.token === RUN.token).state.kind).toBe('working')
    expect(stores.runs.runsState.runs.find((r) => r.token === TASK_RUN.token).state.kind).toBe('deciding')
  })

  it('a run the worker ended is over whatever it ended for', async () => {
    // The ending arrives whole and stays whole: a reason this front end has
    // never heard of is still `stopped`, and it is the state's kind — never
    // the reason — that anything downstream (runScopes.js) reads as "over".
    const { emit, ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_start', RUN)
    await stores.runs.initRuns()
    await stores.runs.loadConfig('/p')
    await stores.runs.startRun('/p', RUN.settings)

    await emit('run:state', { ...RUN, state: { kind: 'stopped', reason: { kind: 'session_removed' } }, session: null })

    expect(stores.runs.runsState.runs[0].state.kind).toBe('stopped')
    expect(stores.runs.runsState.runs[0].state.reason.kind).toBe('session_removed')
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

    expect(stores.runs.runsState.runs).toEqual([])

    await emit('run:state', RUN)
    expect(stores.runs.runsState.runs).toEqual([RUN])
  })

  it('switching projects takes the runs with it', async () => {
    // Cleared where the project moves, not left for loadRun to overwrite: for
    // as long as that call takes, the old runs would be on screen under the
    // new project's name.
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_start', RUN)
    await stores.runs.loadConfig('/p')
    await stores.runs.startRun('/p', RUN.settings)

    await stores.runs.loadConfig('/other')

    expect(stores.runs.runsState.runs).toEqual([])
  })

  it('nothing running is an empty list the panel can draw, not a failure', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_state', [])
    await stores.runs.loadConfig('/p')

    await stores.runs.loadRun('/p')

    expect(stores.runs.runsState.runs).toEqual([])
  })

  it('the worker\'s answer replaces the list wholesale', async () => {
    // `run_state` is the worker's whole truth for the project; what it does
    // not name has ended and left the map.
    const { emit, ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    await stores.runs.initRuns()
    await stores.runs.loadConfig('/p')
    await emit('run:state', RUN)

    ipc.on('run_state', [TASK_RUN])
    await stores.runs.loadRun('/p')

    expect(stores.runs.runsState.runs).toEqual([TASK_RUN])
  })
})

const NO_TOOLS = {
  playwright_mcp: false,
  playwright_browsers: false,
  extension: false,
  busy_project: null
}

describe('what this machine can drive a browser with', () => {
  it('nobody has asked until somebody does, which is not the same as finding nothing', async () => {
    // Null is a third state beside the four facts, and the run dialog spends it:
    // it opens before this answer lands, and blocking its live-check toggle on
    // a fact nobody has established would switch it off on every open.
    const { stores } = await loadStores()
    expect(stores.runs.runsState.browserTools).toBe(null)
  })

  it('the answer lands in the store whole', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('browser_tools', NO_TOOLS)
    await stores.runs.loadConfig('/p')

    await stores.runs.loadBrowserTools('/p')

    expect(stores.runs.runsState.browserTools).toEqual(NO_TOOLS)
    expect(ipc.calls('browser_tools')).toEqual([{ project: '/p' }])
  })

  it('a response for the project we already left is dropped', async () => {
    // The same guard loadConfig carries, and it matters more here: this answer
    // decides whether a control is disabled, so a slow read for a project
    // somebody has left would block the toggle in the one they moved to, over a
    // machine state that was never about it. Resolved by hand for the reason
    // spelled out on the loadConfig version of this test.
    const { stores } = await loadStores()
    const pending = new Map()
    const { mockIPC } = await import('@tauri-apps/api/mocks')
    mockIPC((cmd, args) => new Promise((resolve) => pending.set(args.project, resolve)))

    const slow = stores.runs.loadBrowserTools('/slow')
    stores.runs.runsState.project = '/fast'
    const fast = stores.runs.loadBrowserTools('/fast')

    pending.get('/fast')({ ...NO_TOOLS, extension: true })
    await fast
    pending.get('/slow')(NO_TOOLS)
    await slow

    expect(stores.runs.runsState.browserTools.extension).toBe(true)
  })

  it('switching projects forgets it rather than carrying a guess across', async () => {
    // Per project the same way the run is: `.mcp.json` is the project's own
    // file, and busy-ness is about the runs beside this one.
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('browser_tools', NO_TOOLS)
    await stores.runs.loadConfig('/p')
    await stores.runs.loadBrowserTools('/p')

    await stores.runs.loadConfig('/other')

    expect(stores.runs.runsState.browserTools).toBe(null)
  })

  it('a failed command leaves nothing established, which leaves the toggle live', async () => {
    // The cheaper of the two mistakes: a run that fails inside its check is
    // where things were before this existed, against taking a working live
    // check away over a broken IPC call.
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('browser_tools', NO_TOOLS)
    await stores.runs.loadConfig('/p')
    await stores.runs.loadBrowserTools('/p')

    ipc.fail('browser_tools', new Error('nope'))
    await stores.runs.loadBrowserTools('/p')

    expect(stores.runs.runsState.browserTools).toBe(null)
  })
})

describe('the sound a finished run makes', () => {
  const stopped = (over = {}) => ({
    ...RUN,
    state: { kind: 'stopped', reason: { kind: 'queue_empty' } },
    session: null,
    ...over
  })

  it('a run reaching its ending plays the chosen sound exactly once', async () => {
    const { emit, ipc, stores, nextTick } = await loadStores()
    ipc.on('project_config', OK)
    await stores.runs.initRuns()
    await stores.runs.loadConfig('/p')
    stores.settings.settings.notifications.runFinished = 'sound-3'

    await emit('run:state', RUN)
    await nextTick()
    expect(chime).not.toHaveBeenCalled()

    await emit('run:state', stopped())
    await nextTick()
    expect(chime).toHaveBeenCalledWith('sound-3', { unlessFocused: true })

    // The summary lands seconds after the ending and is another event about the
    // same stopped run. One run, one sound.
    await emit('run:state', stopped({ summary: { tasks: null } }))
    await nextTick()
    expect(chime).toHaveBeenCalledTimes(1)
  })

  it('another run gets its own sound', async () => {
    const { emit, ipc, stores, nextTick } = await loadStores()
    ipc.on('project_config', OK)
    await stores.runs.initRuns()
    await stores.runs.loadConfig('/p')

    await emit('run:state', stopped({ token: 11 }))
    await emit('run:state', stopped({ token: 12 }))
    await nextTick()

    expect(chime).toHaveBeenCalledTimes(2)
  })

  it('a run this window never saw stop is not announced when the list is reread', async () => {
    // `loadRun` replaces the list on every window focus and every project
    // switch: a sound from there would announce this morning's run this
    // afternoon.
    const { ipc, stores } = await loadStores()
    ipc.on('project_config', OK)
    ipc.on('run_state', [stopped()])
    await stores.runs.initRuns()
    await stores.runs.loadConfig('/p')

    await stores.runs.loadRun('/p')

    expect(chime).not.toHaveBeenCalled()
  })

  it('off is silence, not a default sound', async () => {
    const { emit, ipc, stores, nextTick } = await loadStores()
    ipc.on('project_config', OK)
    await stores.runs.initRuns()
    await stores.runs.loadConfig('/p')
    stores.settings.settings.notifications.runFinished = 'off'

    await emit('run:state', stopped())
    await nextTick()

    // `chime` itself refuses `off`; the store still hands it over rather than
    // deciding here, so there is one place that knows what silence is.
    expect(chime).toHaveBeenCalledWith('off', { unlessFocused: true })
  })

  it('hands the focus switch over as it stands, and never decides it here', async () => {
    /* The same division as `off` above: this store reads the setting and passes
       it, and whether the main window has focus is asked in `chime.js`, which
       is the one place with a document to ask. */
    const { emit, ipc, stores, nextTick } = await loadStores()
    ipc.on('project_config', OK)
    await stores.runs.initRuns()
    await stores.runs.loadConfig('/p')
    stores.settings.settings.notifications.onlyWhenUnfocused = false

    await emit('run:state', stopped())
    await nextTick()

    expect(chime).toHaveBeenCalledWith('sound-1', { unlessFocused: false })
  })
})
