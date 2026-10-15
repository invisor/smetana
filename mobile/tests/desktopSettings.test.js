import { describe, expect, it } from 'vitest'
import { applyDesktopSettings, projectRows } from '../src/desktopSettings.js'

const target = () => ({
  openProjects: [],
  activeProject: null,
  kanban: { columns: 'all', alwaysShow: [], interval: 'all', unlimited: [] },
  project: { columnOrder: [], runSettings: null, sideTab: 'files' },
  conversationPanel: true,
  agent: 'claude',
  model: '',
  agentRoles: { tasks: { agent: '', model: '' }, code: { agent: '', model: '' } },
  notifications: { runFinished: 'sound-1', needsAttention: 'sound-2', onlyWhenUnfocused: true },
  appearance: { theme: 'dark' }
})

describe('what the phone takes from the desktop\'s settings', () => {
  it('takes the open projects and the active one', () => {
    const settings = applyDesktopSettings(target(), { openProjects: ['/a', '/b'], activeProject: '/b' })
    expect(settings.openProjects).toEqual(['/a', '/b'])
    expect(settings.activeProject).toBe('/b')
  })

  it('has no active project when the desktop has none', () => {
    const settings = applyDesktopSettings(target(), { openProjects: 'not a list' })
    expect(settings.openProjects).toEqual([])
    expect(settings.activeProject).toBeNull()
  })

  it('takes the board\'s view and the project\'s own state', () => {
    const settings = applyDesktopSettings(target(), {
      kanban: { columns: 'non-empty', interval: 'week' },
      project: { columnOrder: ['done', 'ready'], runSettings: { mode: 'solo' } }
    })
    expect(settings.kanban.columns).toBe('non-empty')
    expect(settings.kanban.interval).toBe('week')
    expect(settings.project.columnOrder).toEqual(['done', 'ready'])
    expect(settings.project.runSettings).toEqual({ mode: 'solo' })
    // A key the answer does not carry keeps what it had.
    expect(settings.project.sideTab).toBe('files')
  })

  it('reads a damaged kanban section as the shipped view', () => {
    const settings = applyDesktopSettings(target(), { kanban: { columns: 'sideways', interval: 7 } })
    expect(settings.kanban).toMatchObject({ columns: 'all', interval: 'all' })
  })

  it('takes what decides whether an agent can be started as a conversation', () => {
    const settings = applyDesktopSettings(target(), {
      conversationPanel: false,
      agent: 'codex',
      model: 'gpt',
      agentRoles: { tasks: { agent: 'claude', model: 'opus' }, nonsense: { agent: 'x' } }
    })
    expect(settings).toMatchObject({ conversationPanel: false, agent: 'codex', model: 'gpt' })
    expect(settings.agentRoles.tasks).toEqual({ agent: 'claude', model: 'opus' })
    expect(settings.agentRoles.nonsense).toBeUndefined()
  })

  it('keeps what it had where the desktop says nothing', () => {
    const settings = applyDesktopSettings(target(), {})
    expect(settings).toMatchObject({ conversationPanel: true, agent: 'claude' })
  })

  it('is silent: the stores must not ring a sound on a phone that a notification already covers', () => {
    const settings = applyDesktopSettings(target(), { notifications: { runFinished: 'sound-3', needsAttention: 'sound-4' } })
    expect(settings.notifications).toMatchObject({ runFinished: 'off', needsAttention: 'off' })
  })

  it('takes nothing else: the appearance stays the phone\'s own', () => {
    const settings = applyDesktopSettings(target(), { appearance: { theme: 'light' } })
    expect(settings.appearance.theme).toBe('dark')
  })
})

describe('the projects on offer', () => {
  it('names each by its folder, keeps its path, and marks the one open on the desktop', () => {
    const rows = projectRows(['/work/a', '/work/b'], '/work/b', (path) => path.split('/').pop())
    expect(rows).toEqual([
      { path: '/work/a', name: 'a', active: false },
      { path: '/work/b', name: 'b', active: true }
    ])
  })

  it('tells two projects with one folder name apart by path', () => {
    const rows = projectRows(['/x/app', '/y/app'], '/y/app', (path) => path.split('/').pop())
    expect(rows.map((row) => row.active)).toEqual([false, true])
  })

  it('has no rows for no projects', () => {
    expect(projectRows(undefined, null, String)).toEqual([])
  })
})
