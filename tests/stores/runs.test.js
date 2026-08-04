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
    const { ipc, stores } = await loadStores()
    const answers = { '/slow': OK, '/fast': { state: 'missing' } }
    ipc.on('project_config', ({ project }) => answers[project])

    const slow = stores.runs.loadConfig('/slow')
    const fast = stores.runs.loadConfig('/fast')
    await Promise.all([slow, fast])

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
