import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

/* The client build: the shell says `split`, the device's half is a local file and the server's half
   is written by patch. The only thing mocked is the IPC transport. */
let ipc
let emit
let nextTick
let settings

const splitState = { mode: 'split', state: 'connected', epoch: 1 }

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  emit = loaded.emit
  nextTick = loaded.nextTick
  settings = loaded.stores.settings
  ipc.on('connection_state', splitState)
  /* The server's answer carries the device fields at their shipped values, which is what a server
     really holds of them. */
  ipc.on('settings_load', {
    ...settings.defaults(),
    agent: 'codex',
    openProjects: ['/work/a'],
    activeProject: '/work/a',
    git: { autoFetch: true, removeWorktrees: false },
    project: { runSettings: { mode: 'solo' } }
  })
  ipc.on('device_settings_load', {
    appearance: { theme: 'light', density: 'compact', uiFontSize: 15 },
    project: { selectedTask: 'a-1' }
  })
  ipc.on('settings_patch', null)
  ipc.on('device_settings_save', null)
})

const wait = async () => {
  await nextTick()
  await settings.flushPending()
}

describe('loading in the client build', () => {
  it('takes the server half from the server and the device half from the device file', async () => {
    await settings.loadSettings()
    const s = settings.settings
    expect(s.agent).toBe('codex')
    expect(s.git).toEqual({ autoFetch: true, removeWorktrees: false })
    expect(s.openProjects).toEqual(['/work/a'])
    expect(s.project.runSettings).toEqual({ mode: 'solo' })
    expect(s.appearance.theme).toBe('light')
    expect(s.appearance.density).toBe('compact')
    expect(s.project.selectedTask).toBe('a-1')
    expect(ipc.calls('device_settings_load')).toEqual([{ project: '/work/a' }])
  })

  it('does not write the load back as a change', async () => {
    await settings.loadSettings()
    await wait()
    expect(ipc.calls('settings_patch')).toEqual([])
  })
})

describe('saving in the client build', () => {
  it('sends the server only the field that changed, and never the whole file', async () => {
    await settings.loadSettings()
    settings.settings.agentRoles.tasks.agent = 'codex'
    settings.settings.agentRoles.tasks.model = ''
    settings.settings.subscription.pauseAt = 80
    await wait()
    expect(ipc.calls('settings_patch')).toEqual([
      { patch: { agentRoles: { tasks: { agent: 'codex' } }, subscription: { pauseAt: 80 } } }
    ])
    expect(ipc.calls('settings_save')).toEqual([])
    expect(ipc.calls('device_settings_save')).toHaveLength(1)
  })

  it('keeps theme and density on the device: nothing goes to the server', async () => {
    await settings.loadSettings()
    settings.settings.appearance.theme = 'dark'
    settings.settings.appearance.density = 'comfortable'
    settings.settings.layout.leftWidth = 300
    await wait()
    expect(ipc.calls('settings_patch')).toEqual([])
    const saved = ipc.calls('device_settings_save')
    expect(saved).toHaveLength(1)
    expect(saved[0].settings.appearance.theme).toBe('dark')
    expect(saved[0].settings.layout.leftWidth).toBe(300)
  })

  it('sends the open project and the project list when they move, and a run setting under its project', async () => {
    await settings.loadSettings()
    settings.settings.project.runSettings = { mode: 'auto' }
    settings.settings.openProjects.push('/work/b')
    await wait()
    expect(ipc.calls('settings_patch')).toEqual([
      { patch: { openProjects: ['/work/a', '/work/b'], projects: { '/work/a': { runSettings: { mode: 'auto' } } } } }
    ])
  })

  it('does not send a change twice', async () => {
    await settings.loadSettings()
    settings.settings.agentPrompt = 'be brief'
    await wait()
    settings.settings.layout.rightWidth = 400
    await wait()
    expect(ipc.calls('settings_patch')).toHaveLength(1)
  })

  it('takes the server state back when it refuses the change, and does not replay it', async () => {
    await settings.loadSettings()
    ipc.fail('settings_patch', 'The connection to the server was lost. Nothing was changed.')
    const errors = vi.spyOn(console, 'error').mockImplementation(() => {})
    settings.settings.agentPrompt = 'be brief'
    await wait()
    await vi.waitFor(() => expect(settings.settings.agentPrompt).toBe(''))
    expect(ipc.calls('settings_patch')).toHaveLength(1)
    errors.mockRestore()
    ipc.on('settings_patch', null)
    await wait()
    expect(ipc.calls('settings_patch'), 'the refused change is not sent again').toHaveLength(1)
  })
})

describe('following the other clients', () => {
  it('shows a change another client made, with no restart', async () => {
    await settings.loadSettings()
    await emit('settings:changed', {
      agent: 'claude',
      agentRoles: { tasks: { agent: 'codex', model: '' }, code: { agent: '', model: '' }, runLead: { agent: '', model: '' }, reviewBranch: { agent: '', model: '' } },
      openProjects: ['/work/a'],
      lastProject: '/work/a',
      projects: { '/work/a': { runSettings: { mode: 'solo' }, agents: null } }
    })
    expect(settings.settings.agent).toBe('claude')
    expect(settings.settings.agentRoles.tasks.agent).toBe('codex')
    expect(settings.sharedSettings().agentRoles.tasks.agent).toBe('codex')
    await wait()
    expect(ipc.calls('settings_patch'), 'what arrived is not sent back').toEqual([])
  })

  it('keeps the other client\'s theme out of this window and this window\'s edit alongside theirs', async () => {
    await settings.loadSettings()
    settings.settings.agentPrompt = 'mine'
    await emit('settings:changed', { agent: 'claude', appearance: { theme: 'dark' } })
    expect(settings.settings.agent).toBe('claude')
    expect(settings.settings.agentPrompt).toBe('mine')
    expect(settings.settings.appearance.theme).toBe('light')
    await wait()
    expect(ipc.calls('settings_patch')).toEqual([{ patch: { agentPrompt: 'mine' } }])
  })
})

describe('the release build and a browser', () => {
  it('write one file whole, as before', async () => {
    ipc.on('connection_state', { mode: 'in-process', state: 'connected', epoch: 0 })
    await settings.loadSettings()
    settings.settings.agentPrompt = 'x'
    await wait()
    expect(ipc.calls('settings_save')).toHaveLength(1)
    expect(ipc.calls('settings_patch')).toEqual([])
    expect(ipc.calls('device_settings_save')).toEqual([])
  })
})
