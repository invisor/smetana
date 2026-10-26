import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'
import { buffer, listing, snapshot } from '../support/fixtures.js'

let ipc
let emit
let settings
let projects
let tabs

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  emit = loaded.emit
  settings = loaded.stores.settings
  projects = loaded.stores.projects
  tabs = loaded.stores.tabs

  ipc.on('settings_load', {})
  ipc.on('settings_save', null)
  ipc.on('project_root', (args) => args.path)
  ipc.on('files_list', (args) => listing({ dir: args.dir }))
  ipc.on('tracker_set_project', snapshot())
  ipc.on('tracker_probe', (args) => args.paths.map((path) => ({ path, tracked: true })))
  /* A move loads the new project's sessions before restoring its tabs, so every
     test in this file reaches this command whether or not it is about agents.
     Answered with nothing rather than left to refuse: an unregistered command
     rejects, `loadSessions` swallows it, and the move would carry on through a
     caught error — which works, and which would make the two tests below that
     answer this deliberately look like nothing special. */
  ipc.on('terminal_list', [])
  /* Read beside the session list, in the same `loadSessions`, so every test
     here reaches it too. Answered with nothing: what a project offers back
     after a restart is `terminals.test.js`'s subject, and an unregistered
     command would have this file exercising a caught failure instead. */
  ipc.on('terminal_restorable', [])

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

  /* The other half of the tree read above, and it is about a project folder that
     is not there at all — an unmounted volume, or a worktree removed while it
     was open here. `resolve_within` (`files/fs.rs`) canonicalizes the **root**
     before the path inside it, so `files_list` refuses every `dir` and not only
     `''`. Fired as one `Promise.all`, each of those refusals reads to
     `stores/files.js` as a folder somebody deleted and is folded out of
     `expanded` — a persistent setting, so the person would come back to a flat
     tree for good, behind a single toast. The root is read first and the folders
     inside it only if it answered, which is `refreshDirs`' rule and is copied
     here; this test is what holds that copy in place. */
  it('a project whose folder has gone keeps its expanded folders', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'
    ipc.on('settings_load', (args) =>
      args.project === '/b' ? { project: { expanded: ['src', 'src/stores', 'docs'] } } : {}
    )
    ipc.fail('files_list', { kind: 'notFound', message: 'no such file or directory' })

    await projects.switchTo('/b')

    expect(settings.settings.project.expanded).toEqual(['src', 'src/stores', 'docs'])
    // The three are never asked about: the root's own refusal stopped the read.
    expect(ipc.calls('files_list').map((call) => call.dir)).toEqual([''])
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

  /* Without a `defaultPath` macOS opens the panel in Recents, which is every
     application's files at once, and the previews it draws there are charged to
     this unsandboxed app — four consent prompts from a development tool that
     wants none of it. The starting point is the folder the current project sits
     in, where a person's other projects usually are. */
  it('the folder panel starts in the folder the current project sits in', async () => {
    settings.settings.activeProject = '/Users/you/Projects/smetana'
    ipc.on('plugin:dialog|open', null)

    await projects.addProject()

    expect(ipc.calls('plugin:dialog|open')[0].options.defaultPath).toBe('/Users/you/Projects')
  })

  it('with no project open the option is absent rather than empty', async () => {
    ipc.on('plugin:dialog|open', null)

    await projects.addProject()

    expect(ipc.calls('plugin:dialog|open')[0].options.defaultPath).toBeUndefined()
  })

  /* The parent of what the panel handed back, not of the root it normalizes to:
     that folder is the one that was on screen, and where the tracked root turns
     out to be is a different question. */
  it('the next open starts beside what was picked, not beside the tracked root', async () => {
    settings.settings.activeProject = '/Users/you/Projects/smetana'
    ipc.on('plugin:dialog|open', '/elsewhere/repository/src/stores')
    ipc.on('project_root', '/elsewhere/repository')
    await projects.addProject()

    ipc.on('plugin:dialog|open', null)
    await projects.addProject()

    expect(ipc.calls('plugin:dialog|open')[1].options.defaultPath).toBe(
      '/elsewhere/repository/src'
    )
  })

  it('cancelling chooses nothing and therefore moves nothing', async () => {
    settings.settings.activeProject = '/Users/you/Projects/smetana'
    ipc.on('plugin:dialog|open', null)

    await projects.addProject()
    await projects.addProject()

    expect(ipc.calls('plugin:dialog|open')[1].options.defaultPath).toBe('/Users/you/Projects')
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

describe('a project switched on the phone', () => {
  it('moves the window to it the way a click on the list does', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'
    await projects.initProjectFollow()

    await emit('remote:project-switched', { path: '/b' })
    await vi.waitFor(() => expect(settings.settings.activeProject).toBe('/b'))

    expect(ipc.calls('tracker_set_project')).toEqual([{ path: '/b' }])
    expect(ipc.calls('files_list').some((call) => call.dir === '')).toBe(true)
  })

  it('puts a folder that is not on the list onto it', async () => {
    settings.settings.openProjects = ['/a']
    settings.settings.activeProject = '/a'
    await projects.initProjectFollow()

    await emit('remote:project-switched', { path: '/c' })
    await vi.waitFor(() => expect(settings.settings.activeProject).toBe('/c'))

    expect(settings.settings.openProjects).toEqual(['/a', '/c'])
  })

  it('does nothing about the project the window is already on, so the announcement cannot loop', async () => {
    settings.settings.openProjects = ['/a']
    settings.settings.activeProject = '/a'
    await projects.initProjectFollow()

    await emit('remote:project-switched', { path: '/a' })
    await new Promise((resolve) => setTimeout(resolve, 20))

    expect(ipc.calls('tracker_set_project')).toEqual([])
    expect(settings.settings.activeProject).toBe('/a')
  })

  it('tells the worker where the window is when the phone empties the tracker', async () => {
    settings.settings.openProjects = ['/a']
    settings.settings.activeProject = '/a'
    await projects.initProjectFollow()

    await emit('remote:project-switched', { path: null })
    await vi.waitFor(() => expect(ipc.calls('tracker_set_project')).toEqual([{ path: '/a' }]))

    expect(settings.settings.activeProject).toBe('/a')
  })

  it('puts the worker back on the window\'s project before the question and keeps it there when the person cancels', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'an edit' }))
    let answer
    tabs.onUnsaved(() => new Promise((resolve) => (answer = resolve)))
    await projects.initProjectFollow()

    await emit('remote:project-switched', { path: '/b' })
    await vi.waitFor(() => expect(answer).toBeTypeOf('function'))
    /* The question is up, and the board is already back on the window's project. */
    expect(ipc.calls('tracker_set_project')).toEqual([{ path: '/a' }])

    answer(false)
    await new Promise((resolve) => setTimeout(resolve, 20))

    expect(settings.settings.activeProject).toBe('/a')
    expect(ipc.calls('tracker_set_project').at(-1)).toEqual({ path: '/a' })
  })

  it('moves the worker to the phone\'s project only once the person agrees', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'an edit' }))
    tabs.onUnsaved(() => true)
    await projects.initProjectFollow()

    await emit('remote:project-switched', { path: '/b' })
    await vi.waitFor(() => expect(settings.settings.activeProject).toBe('/b'))

    expect(ipc.calls('tracker_set_project').at(-1)).toEqual({ path: '/b' })
  })

  it('an announcement that lands while the window is moving is replayed, and ends with window and worker together', async () => {
    settings.settings.openProjects = ['/a', '/b', '/c']
    settings.settings.activeProject = '/a'
    await projects.initProjectFollow()
    /* The window's own move to /c is held in the middle of its tracker call. */
    let release
    ipc.on('tracker_set_project', (args) =>
      args.path === '/c' ? new Promise((resolve) => (release = () => resolve(snapshot()))) : snapshot()
    )
    const own = projects.switchTo('/c')
    await vi.waitFor(() => expect(release).toBeTypeOf('function'))

    await emit('remote:project-switched', { path: '/b' })
    release()
    await own
    await vi.waitFor(() => expect(settings.settings.activeProject).toBe('/b'))

    expect(ipc.calls('tracker_set_project').at(-1)).toEqual({ path: '/b' })
  })

  it('an announcement that lands during a removal is not lost either', async () => {
    settings.settings.openProjects = ['/a', '/b', '/c']
    settings.settings.activeProject = '/a'
    await projects.initProjectFollow()
    tabs.onUnsaved(() => true)
    const removal = projects.removeProject('/c')
    await emit('remote:project-switched', { path: '/b' })
    await removal
    await vi.waitFor(() => expect(settings.settings.activeProject).toBe('/b'))
    expect(ipc.calls('tracker_set_project').at(-1)).toEqual({ path: '/b' })
  })

  it('takes the tracked root of the phone\'s path, as adding a project does, so no second row appears', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'
    ipc.on('project_root', (args) => (args.path.startsWith('/b') ? '/b' : args.path))
    await projects.initProjectFollow()

    await emit('remote:project-switched', { path: '/b/src/deep/' })
    await vi.waitFor(() => expect(settings.settings.activeProject).toBe('/b'))

    expect(settings.settings.openProjects).toEqual(['/a', '/b'])
    expect(ipc.calls('tracker_set_project').at(-1)).toEqual({ path: '/b' })
  })

  it('puts the worker back when the phone chose a subfolder of the window\'s project', async () => {
    settings.settings.openProjects = ['/a']
    settings.settings.activeProject = '/a'
    ipc.on('project_root', () => '/a')
    await projects.initProjectFollow()

    await emit('remote:project-switched', { path: '/a/sub' })
    await vi.waitFor(() => expect(ipc.calls('tracker_set_project')).toEqual([{ path: '/a' }]))

    expect(settings.settings.activeProject).toBe('/a')
  })

  it('keeps the worker on the window\'s project while a question is up, however often the phone switches', async () => {
    settings.settings.openProjects = ['/a', '/b', '/c']
    settings.settings.activeProject = '/a'
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'an edit' }))
    let answer
    tabs.onUnsaved(() => new Promise((resolve) => (answer = resolve)))
    await projects.initProjectFollow()
    /* The person's own move is the one asking. */
    const own = projects.switchTo('/c')
    await vi.waitFor(() => expect(answer).toBeTypeOf('function'))

    await emit('remote:project-switched', { path: '/b' })
    await emit('remote:project-switched', { path: '/b' })
    await vi.waitFor(() => expect(ipc.calls('tracker_set_project').length).toBeGreaterThanOrEqual(2))
    await new Promise((resolve) => setTimeout(resolve, 20))

    expect(ipc.calls('tracker_set_project').at(-1)).toEqual({ path: '/a' })
    answer(false)
    await own
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(ipc.calls('tracker_set_project').at(-1)).toEqual({ path: '/a' })
  })
})


describe('adding a project on a server that is not on this machine', () => {
  it('adds a folder the server browser chose without opening the operating system panel', async () => {
    const added = await projects.addProjectAt('/home/dev/app')

    expect(added).toBe('/home/dev/app')
    expect(settings.settings.openProjects).toEqual(['/home/dev/app'])
    expect(settings.settings.activeProject).toBe('/home/dev/app')
    expect(ipc.calls('plugin:dialog|open')).toHaveLength(0)
    expect(ipc.calls('project_root')).toEqual([{ path: '/home/dev/app' }])
  })

  it('lists folders by the path the server gave and passes the root as null', async () => {
    const listing = { root: '/home/dev', path: '/home/dev', parent: null, entries: [], truncated: false }
    ipc.on('folders_list', listing)

    expect(await projects.listServerFolders()).toBe(listing)
    await projects.listServerFolders('/home/dev/app')

    expect(ipc.calls('folders_list')).toEqual([{ path: null }, { path: '/home/dev/app' }])
  })

  it('says what the server refused a folder with', async () => {
    ipc.on('folders_list', () => {
      throw { kind: 'outside', message: '/etc is outside the folders this server lets a client reach' }
    })

    await expect(projects.listServerFolders('/etc')).rejects.toThrow(/outside the folders/)
  })

  it('clones into the folder it was given and answers the new path', async () => {
    ipc.on('project_clone', { path: '/home/dev/app' })

    const path = await projects.cloneOnServer({ url: 'https://host/team/app.git', parent: '/home/dev' })

    expect(path).toBe('/home/dev/app')
    expect(ipc.calls('project_clone')).toEqual([{ url: 'https://host/team/app.git', parent: '/home/dev', name: null }])
  })

  it('shows a failed clone in git\'s own words', async () => {
    ipc.on('project_clone', () => {
      throw { kind: 'clone', message: 'fatal: repository not found' }
    })

    await expect(
      projects.cloneOnServer({ url: 'https://host/x.git', parent: '/home/dev', name: 'x' })
    ).rejects.toThrow('fatal: repository not found')
  })
})

describe('moving to another server', () => {
  const state = (over) => ({ mode: 'split', state: 'connected', epoch: 1, local: true, target: 'local', ...over })

  async function onLocalServer() {
    const loaded = await loadStores()
    const { ipc, emit, stores } = loaded
    ipc.on('connection_state', state())
    ipc.on('settings_load', ({ project }) => ({
      ...stores.settings.defaults(),
      openProjects: ['/Users/me/work'],
      activeProject: '/Users/me/work',
      lastProject: '/Users/me/work',
      project: { runSettings: null }
    }))
    ipc.on('device_settings_load', {})
    ipc.on('device_settings_save', null)
    ipc.on('settings_patch', null)
    ipc.on('project_root', (args) => args.path)
    ipc.on('files_list', (args) => listing({ dir: args.dir }))
    ipc.on('tracker_set_project', snapshot())
    ipc.on('tracker_snapshot', snapshot())
    ipc.on('tracker_probe', (args) => args.paths.map((path) => ({ path, tracked: true })))
    ipc.on('terminal_list', [])
    ipc.on('terminal_restorable', [])
    await stores.settings.loadSettings()
    await stores.connection.initConnection()
    return { ...loaded }
  }

  const settled = async (loaded) => {
    for (let i = 0; i < 20; i++) await loaded.nextTick()
    await new Promise((resolve) => setTimeout(resolve, 20))
    await loaded.stores.settings.flushPending()
  }

  it('takes the new server\'s project list and open project, and sends it nothing of this machine\'s', async () => {
    const loaded = await onLocalServer()
    const { ipc, emit, stores } = loaded
    expect(stores.settings.settings.activeProject).toBe('/Users/me/work')

    // The person chooses a server on another machine; it has its own projects.
    ipc.on('settings_load', () => ({
      ...stores.settings.defaults(),
      agent: 'codex',
      openProjects: ['/home/dev/app'],
      activeProject: '/home/dev/app',
      lastProject: '/home/dev/app',
      project: { runSettings: null }
    }))
    await emit('connection:state', state({ epoch: 2, local: false, target: 'node-vps' }))
    await settled(loaded)

    expect(stores.settings.settings.openProjects).toEqual(['/home/dev/app'])
    expect(stores.settings.settings.activeProject).toBe('/home/dev/app')
    expect(stores.settings.settings.agent).toBe('codex')
    expect(ipc.calls('tracker_set_project')).toEqual([{ path: '/home/dev/app' }])
    // Nothing of this machine's - a project path, a setting - was sent to the server, before or after.
    expect(ipc.calls('settings_patch')).toEqual([])
    expect(JSON.stringify(ipc.calls('settings_patch'))).not.toContain('/Users/me')
  })

  it('shows the empty state when the new server has no project open', async () => {
    const loaded = await onLocalServer()
    const { ipc, emit, stores } = loaded
    ipc.on('settings_load', () => ({ ...stores.settings.defaults(), openProjects: [], activeProject: null }))
    await emit('connection:state', state({ epoch: 2, local: false, target: 'node-vps' }))
    await settled(loaded)

    expect(stores.settings.settings.activeProject).toBeNull()
    expect(stores.settings.settings.openProjects).toEqual([])
    expect(ipc.calls('settings_patch')).toEqual([])
  })

  it('does nothing of the kind when the same server reconnects', async () => {
    const loaded = await onLocalServer()
    const { ipc, emit, stores } = loaded
    ipc.on('settings_load', () => ({ ...stores.settings.defaults(), openProjects: ['/elsewhere'], activeProject: '/elsewhere' }))
    await emit('connection:state', state({ epoch: 2 }))
    await settled(loaded)

    expect(ipc.calls('tracker_set_project')).toEqual([])
    expect(stores.settings.settings.activeProject).toBe('/Users/me/work')
  })
})
