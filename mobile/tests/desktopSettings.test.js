import { describe, expect, it } from 'vitest'
import { LOOK_MINOR, applyDesktopSettings, projectRows } from '../src/desktopSettings.js'

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
  appearance: { theme: 'system', density: 'comfortable', uiFontSize: 13 }
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

  it('takes the theme and the density from a desktop at the look minor, never the font size', () => {
    const settings = applyDesktopSettings(
      target(),
      { appearance: { theme: 'light', density: 'compact', uiFontSize: 18 } },
      { minor: LOOK_MINOR }
    )
    expect(settings.appearance).toEqual({ theme: 'light', density: 'compact', uiFontSize: 13 })
  })

  it('keeps the phone\'s own look from a desktop below the minor', () => {
    const settings = applyDesktopSettings(
      target(),
      { appearance: { theme: 'light', density: 'compact' } },
      { minor: LOOK_MINOR - 1 }
    )
    expect(settings.appearance).toMatchObject({ theme: 'system', density: 'comfortable' })
    expect(applyDesktopSettings(target(), { appearance: { theme: 'light' } }).appearance.theme).toBe('system')
  })

  it('takes the board\'s view whatever the minor', () => {
    const settings = applyDesktopSettings(target(), { kanban: { columns: 'non-empty' }, project: { columnOrder: ['done'] } }, { minor: 3 })
    expect(settings.kanban.columns).toBe('non-empty')
    expect(settings.project.columnOrder).toEqual(['done'])
  })
})

describe('the phone\'s own choices over the desktop\'s', () => {
  const phone = { theme: 'light', density: 'desktop', uiFontSize: 16, columns: 'non-empty', interval: 'desktop' }
  const stored = { appearance: { theme: 'dark', density: 'compact' }, kanban: { interval: 'week' } }

  it('wins where it chose, follows the desktop where it did not, and owns the font size', () => {
    const settings = applyDesktopSettings(target(), stored, { minor: LOOK_MINOR, phone })
    expect(settings.appearance).toEqual({ theme: 'light', density: 'compact', uiFontSize: 16 })
    expect(settings.kanban).toMatchObject({ columns: 'non-empty', interval: 'week' })
  })

  it('keeps the target\'s own density below the look minor', () => {
    const settings = applyDesktopSettings(target(), stored, { minor: LOOK_MINOR - 1, phone })
    expect(settings.appearance).toMatchObject({ theme: 'light', density: 'comfortable' })
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
