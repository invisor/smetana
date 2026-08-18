import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'
import { buffer, listing, snapshot } from '../support/fixtures.js'

let ipc
let settings
let projects
let tabs

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  settings = loaded.stores.settings
  projects = loaded.stores.projects
  tabs = loaded.stores.tabs

  ipc.on('settings_load', {})
  ipc.on('settings_save', null)
  ipc.on('project_root', (args) => args.path)
  ipc.on('files_list', (args) => listing({ dir: args.dir }))
  ipc.on('tracker_set_project', snapshot())
  ipc.on('tracker_probe', (args) => args.paths.map((path) => ({ path, tracked: true })))

  /* Mandatory: the settings watcher is installed only inside loadSettings.
     Without it flushPending has nothing to flush, settings_save never happens,
     and the "write before the board" ordering check would compare two -1s, that
     is, pass for nothing. */
  await settings.loadSettings()
})

describe('basename', () => {
  it('splits on both separators: WebView2 is among the target webviews', () => {
    expect(projects.basename('/home/someone/project')).toBe('project')
    expect(projects.basename('C:\\Users\\someone\\project')).toBe('project')
    expect(projects.basename('/project/')).toBe('project')
  })
})

describe('projectRows', () => {
  it('before the probe answers a row counts as tracked', () => {
    settings.settings.openProjects = ['/a']

    expect(projects.projectRows.value).toEqual([{ path: '/a', name: 'a', tracked: true }])
  })

  it('the probe\'s answer lowers the flag where there is no tracker', async () => {
    settings.settings.openProjects = ['/a', '/b']
    ipc.on('tracker_probe', (args) =>
      args.paths.map((path) => ({ path, tracked: path === '/a' }))
    )

    await projects.refreshProbes()

    expect(projects.projectRows.value.map((row) => row.tracked)).toEqual([true, false])
  })
})

describe('switchTo', () => {
  it('moves: layout, tree, tabs, board', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'

    await projects.switchTo('/b')

    expect(settings.settings.activeProject).toBe('/b')
    expect(ipc.calls('tracker_set_project')).toEqual([{ path: '/b' }])
    expect(ipc.calls('files_list').some((call) => call.dir === '')).toBe(true)
  })

  it('the departing project\'s state lands on disk before the board changes', async () => {
    settings.settings.activeProject = '/a'
    settings.settings.project.sideTab = 'agents'

    await projects.switchTo('/b')

    const commands = ipc.commands()
    /* Without this, indexOf on a settings_save that never happened would give
       -1, and -1 < N would be true in any order — the assert below would pass
       for nothing. */
    expect(commands).toContain('settings_save')
    expect(commands.indexOf('settings_save')).toBeLessThan(
      commands.indexOf('tracker_set_project')
    )
  })

  /* The ordering the Agent tab's repair rests on, and it is a repair that has
     to see the *new* project's sessions. `restoreTabs` sends an `activeTab` of
     "terminal" back to the board when the project has no agent — sessions do not
     survive a restart, so the remembered value usually names a tab that cannot
     exist yet. Read against the previous project's list it would do the
     opposite of its job: a project with a live agent, left on the Agent tab,
     would open on the board and have that written into its settings.

     The list answers late here on purpose. In the app it is `terminal_list`
     queueing behind a spawn — about a second — against two quick directory
     reads, which is the race this ordering exists to lose safely. */
  it('a project with a live agent keeps its Agent tab even when the list answers late', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'
    ipc.on('settings_load', (args) =>
      args.project === '/b' ? { project: { activeTab: 'terminal' } } : {}
    )
    ipc.on(
      'terminal_list',
      () =>
        new Promise((resolve) =>
          setTimeout(
            () =>
              resolve([
                {
                  id: 1,
                  agent: 'claude',
                  cwd: '/b',
                  project: '/b',
                  state: 'running',
                  question: null,
                  startedAt: '2026-08-19T10:00:00Z',
                  exitCode: null,
                  work: { kind: 'bare' }
                }
              ]),
            20
          )
        )
    )

    await projects.switchTo('/b')

    expect(settings.settings.project.activeTab).toBe('terminal')
    expect(tabs.hasAgentTab.value).toBe(true)
  })

  /* The other half, and the reason the repair exists at all: the same remembered
     value, in a project whose sessions really are gone. */
  it('a project with no agent left on the Agent tab opens on the board', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'
    ipc.on('settings_load', (args) =>
      args.project === '/b' ? { project: { activeTab: 'terminal' } } : {}
    )
    ipc.on('terminal_list', [])

    await projects.switchTo('/b')

    expect(settings.settings.project.activeTab).toBe('kanban')
  })

  it('moving to the current project does nothing', async () => {
    settings.settings.activeProject = '/a'

    await projects.switchTo('/a')

    expect(ipc.calls('tracker_set_project')).toHaveLength(0)
  })

  it('a second click during a move is ignored — the last answer must not win', async () => {
    settings.settings.activeProject = '/a'

    const first = projects.switchTo('/b')
    const second = projects.switchTo('/c')
    await Promise.all([first, second])

    expect(ipc.calls('tracker_set_project')).toEqual([{ path: '/b' }])
    expect(settings.settings.activeProject).toBe('/b')
  })

  it('"the person changed their mind" cancels the move entirely', async () => {
    settings.settings.activeProject = '/a'
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'an edit' }))
    tabs.onUnsaved(() => false)

    await projects.switchTo('/b')

    expect(settings.settings.activeProject).toBe('/a')
    expect(ipc.calls('tracker_set_project')).toHaveLength(0)
  })

  /* The name is broader than the check: switchTo holds no catch, and
     tracker.setProject swallows the error itself (see tracker.js) — there is no
     branch with a real throw inside switchTo in the code. What the test really
     pins is only that moving is cleared in finally and that a second move after
     a failed first one is not blocked. */
  it('a move after a failed setProject is not blocked — moving is cleared in finally', async () => {
    settings.settings.activeProject = '/a'
    ipc.fail('tracker_set_project', new Error('no such folder'))
    await projects.switchTo('/b')

    ipc.on('tracker_set_project', snapshot())
    await projects.switchTo('/c')

    expect(settings.settings.activeProject).toBe('/c')
  })
})

describe('addProject', () => {
  it('adds the chosen folder and moves into it', async () => {
    ipc.on('plugin:dialog|open', '/new')

    const added = await projects.addProject()

    expect(added).toBe('/new')
    expect(settings.settings.openProjects).toEqual(['/new'])
    expect(settings.settings.activeProject).toBe('/new')
  })

  it('a subfolder of a tracked repository is normalized to its root once', async () => {
    ipc.on('plugin:dialog|open', '/repository/src/stores')
    ipc.on('project_root', '/repository')

    const added = await projects.addProject()

    expect(added).toBe('/repository')
    expect(settings.settings.openProjects).toEqual(['/repository'])
    expect(ipc.calls('project_root')).toEqual([{ path: '/repository/src/stores' }])
  })

  it('cancelling the dialog touches nothing', async () => {
    ipc.on('plugin:dialog|open', null)

    const added = await projects.addProject()

    expect(added).toBeNull()
    expect(settings.settings.openProjects).toEqual([])
    expect(ipc.calls('tracker_set_project')).toHaveLength(0)
  })

  it('a failed dialog does not break the store', async () => {
    ipc.fail('plugin:dialog|open', new Error('the dialog did not open'))

    await expect(projects.addProject()).resolves.toBeNull()
    expect(settings.settings.openProjects).toEqual([])
  })

  it('choosing the already-active project stages no move and asks nothing about tabs', async () => {
    settings.settings.activeProject = '/a'
    settings.settings.openProjects = ['/a']
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'an edit' }))
    const asked = vi.fn(() => true)
    tabs.onUnsaved(asked)
    ipc.on('plugin:dialog|open', '/a')

    const added = await projects.addProject()

    expect(added).toBe('/a')
    expect(asked).not.toHaveBeenCalled()
    expect(ipc.calls('tracker_set_project')).toHaveLength(0)
    expect(settings.settings.openProjects).toEqual(['/a'])
  })

  /* The dialog itself takes as long as it takes (the comment in addProject): a
     move may have started and finished while the person was picking a folder,
     so moving is checked again after it returns, not only on the way into the
     function. The race is assembled deterministically with the same trick as
     the keepMine race in tests/stores/tabs/freshness.test.js: the dialog is held
     on a controlled promise, and in that time switchTo manages to raise moving
     and get stuck on its own held promise (tracker_set_project) — so moving is
     guaranteed to still be up when the dialog is released. */
  it('a move that started while the dialog was open cancels the addition — moving is checked again after it returns', async () => {
    settings.settings.openProjects = ['/a']
    settings.settings.activeProject = '/a'

    let releaseDialog
    const dialogHeld = new Promise((resolve) => {
      releaseDialog = resolve
    })
    ipc.on('plugin:dialog|open', () => dialogHeld.then(() => '/new'))

    let releaseSetProject
    const setProjectHeld = new Promise((resolve) => {
      releaseSetProject = resolve
    })
    ipc.on('tracker_set_project', () => setProjectHeld.then(() => snapshot()))

    const addPromise = projects.addProject()
    /* switchTo raises moving synchronously, before its first await — the call
       below is guaranteed to catch addProject inside await open(...). */
    const switchPromise = projects.switchTo('/b')

    releaseDialog()
    const added = await addPromise

    /* moving is still up (switchTo is stuck on the held tracker_set_project):
       the repeat check must have cancelled the addition. */
    expect(added).toBeNull()
    expect(settings.settings.openProjects).toEqual(['/a'])

    releaseSetProject()
    await switchPromise

    expect(settings.settings.activeProject).toBe('/b')
  })

  it('a project that is already open but inactive is not duplicated in the list', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'
    ipc.on('plugin:dialog|open', '/b')

    const added = await projects.addProject()

    expect(added).toBe('/b')
    expect(settings.settings.openProjects).toEqual(['/a', '/b'])
    expect(settings.settings.activeProject).toBe('/b')
  })
})

describe('removeProject', () => {
  it('the next project becomes active', async () => {
    settings.settings.openProjects = ['/a', '/b', '/c']
    settings.settings.activeProject = '/b'

    await projects.removeProject('/b')

    expect(settings.settings.openProjects).toEqual(['/a', '/c'])
    expect(settings.settings.activeProject).toBe('/c')
  })

  it('for the last row it is the previous one', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/b'

    await projects.removeProject('/b')

    expect(settings.settings.activeProject).toBe('/a')
  })

  it('an emptied list leaves the window without a project — that is a normal state', async () => {
    settings.settings.openProjects = ['/a']
    settings.settings.activeProject = '/a'

    await projects.removeProject('/a')

    expect(settings.settings.openProjects).toEqual([])
    expect(settings.settings.activeProject).toBe(null)
  })

  it('it does not ask about an inactive row: "don\'t save" would erase edits for nothing', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'an edit' }))
    const asked = vi.fn(() => true)
    tabs.onUnsaved(asked)

    await projects.removeProject('/b')

    expect(asked).not.toHaveBeenCalled()
    expect(settings.settings.openProjects).toEqual(['/a'])
    expect(settings.settings.activeProject).toBe('/a')
  })

  it('it does not touch a row that is not there', async () => {
    settings.settings.openProjects = ['/a']
    settings.settings.activeProject = '/a'

    await projects.removeProject('/nope')

    expect(settings.settings.openProjects).toEqual(['/a'])
    expect(settings.settings.activeProject).toBe('/a')
    expect(ipc.calls('tracker_set_project')).toHaveLength(0)
  })
})

describe('the first run', () => {
  it('an active project that is not in the list gets into it', () => {
    settings.settings.activeProject = '/found'
    settings.settings.openProjects = []

    projects.adoptInitialProject()

    expect(settings.settings.openProjects).toEqual(['/found'])
  })

  it('an empty list with no active project is left empty', () => {
    settings.settings.activeProject = null

    projects.adoptInitialProject()

    expect(settings.settings.openProjects).toEqual([])
  })
})

describe('bd init', () => {
  it('success refreshes the board and the folder probe', async () => {
    ipc.on('tracker_init', snapshot({ generation: 3 }))
    settings.settings.openProjects = ['/a']

    await projects.initActive()

    expect(ipc.calls('tracker_init')).toHaveLength(1)
    expect(ipc.calls('tracker_probe').length).toBeGreaterThan(0)
  })

  it('the refusal is swallowed: the toast has already shown the message', async () => {
    ipc.fail('tracker_init', new Error('bd init did not work out'))

    await expect(projects.initActive()).resolves.toBeUndefined()
  })
})
