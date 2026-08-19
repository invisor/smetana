import { beforeEach, describe, expect, it } from 'vitest'
import { loadStores } from '../../support/stores.js'
import { fileText } from '../../support/fixtures.js'

/* The two tabs in the centre that are derived rather than remembered: the Agent
   tab, which exists exactly while the project has an agent, and one tab per
   shell session.

   What is worth testing here is the deriving itself. Neither tab is in
   `openTabs`, so neither survives a restart — and both have to be right without
   anybody resetting them when the project changes, which is the whole reason
   they are computed from the session list instead of held beside it. */

let ipc
let settings
let tabs
let terminals

const state = () => settings.settings.project

const session = (over = {}) => ({
  id: 1,
  agent: 'claude',
  cwd: '/p',
  project: '/p',
  state: 'running',
  question: null,
  startedAt: '2026-08-18T10:00:00Z',
  exitCode: null,
  work: { kind: 'bare' },
  ...over
})

const shell = (over = {}) => session({ agent: '/bin/zsh', work: { kind: 'shell' }, ...over })

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  settings = loaded.stores.settings
  tabs = loaded.stores.tabs
  terminals = loaded.stores.terminals
  loaded.stores.files.setRoot('/p')
  ipc.on('files_read', (args) => fileText({ path: args.path, text: `text of ${args.path}` }))
  ipc.on('terminal_list', [])
  ipc.on('terminal_remove', null)
})

const listed = async (...sessions) => {
  ipc.on('terminal_list', sessions)
  await terminals.loadSessions('/p')
}

const ids = () => tabs.tabList.value.map((tab) => tab.id)

describe('the Agent tab', () => {
  it('is not there in a project with nothing running', async () => {
    await listed()

    expect(ids()).toEqual(['kanban'])
    expect(tabs.hasAgentTab.value).toBe(false)
  })

  it('is there, straight after the board, as soon as the project has an agent', async () => {
    await listed(session({ id: 4 }))

    expect(ids()).toEqual(['kanban', 'terminal'])
    // The board is the fixed point of the row and stays its first entry.
    expect(tabs.tabList.value[0]).toMatchObject({ kind: 'pinned', label: 'Kanban' })
    // Sans label and no close button, which is what `pinned` means to Tab.vue.
    expect(tabs.tabList.value[1]).toMatchObject({ kind: 'pinned', label: 'Agent' })
  })

  /* A spawn takes about a second. A tab that appeared only when the worker
     answered would leave the button somebody pressed doing nothing visible for
     that second — the same reason the agents panel draws a row for a start. */
  it('stands in place while the agent is still being spawned', async () => {
    await listed()
    ipc.on('terminal_create', () => new Promise(() => {}))

    terminals.createSession('/p', { kind: 'bare' })

    expect(ids()).toEqual(['kanban', 'terminal'])
  })

  it('goes with the last agent, and a person on it lands on the board', async () => {
    await listed(session({ id: 4 }))
    state().activeTab = 'terminal'

    await terminals.removeSession(4)
    tabs.dropAgentTab()

    expect(ids()).toEqual(['kanban'])
    expect(state().activeTab).toBe('kanban')
  })

  /* Only the tab that disappeared. Somebody reading a file while the last agent
     exits is not moved off it. */
  it('leaves whatever else was open alone', async () => {
    await listed(session({ id: 4 }))
    tabs.openFile('a.txt')

    await terminals.removeSession(4)
    tabs.dropAgentTab()

    expect(state().activeTab).toBe('a.txt')
  })

  /* Sessions do not survive a restart, so a project last left on the Agent tab
     comes back naming a tab that cannot exist yet. Rust passes the value
     through on purpose — see `ProjectState::validate` — so this is where it is
     repaired. */
  it('a project remembered as active on it opens on the board instead', async () => {
    await listed()
    state().activeTab = 'terminal'

    await tabs.restoreTabs()

    expect(state().activeTab).toBe('kanban')
  })

  /* The other half of that guard: a project switch inside one session can
     arrive at a project that does have agents running, and taking a person to
     the board then would repair something that was not broken.

     This is the repair in isolation, with the session list already loaded. That
     the *app* loads it first — `moveTo` awaits `loadSessions` before this, and
     the race it would otherwise lose is real — is pinned over in
     tests/stores/projects.test.js, where the list deliberately answers late. */
  it('a project that does have an agent stays on it', async () => {
    await listed(session({ id: 4 }))
    state().activeTab = 'terminal'

    await tabs.restoreTabs()

    expect(state().activeTab).toBe('terminal')
  })
})

describe('a terminal tab', () => {
  it('appears for a shell session, after the file tabs, with its own kind', async () => {
    tabs.openFile('a.txt')
    await listed(shell({ id: 2 }))

    expect(tabs.tabList.value.map((tab) => tab.kind)).toEqual(['pinned', 'preview', 'terminal'])
    expect(tabs.tabList.value.at(-1)).toMatchObject({ label: 'Terminal 1', icon: 'terminal' })
  })

  /* After the diffs as well as after the files, which is the decision rather
     than an accident of there being nothing else in the row: both lists are
     things nobody remembers, and the file order is the person's own, so neither
     belongs inside it. A diff has to be present for that half to be checked at
     all. */
  it('sits after the diff tabs too', async () => {
    ipc.on('vcs_file_at_head', 'HEAD of it')
    tabs.openFile('a.txt')
    tabs.openDiff('/p', 'src/main.rs')
    await listed(shell({ id: 2 }))

    expect(tabs.tabList.value.map((tab) => tab.kind)).toEqual([
      'pinned',
      'preview',
      'diff',
      'terminal'
    ])
  })

  /* The id shares the tab row with paths and can land in `project.activeTab`
     beside them, so it has to be a string no file can be called. A zero byte is
     what makes that true rather than nearly true. */
  it('is identified by something no file path can be', async () => {
    await listed(shell({ id: 7 }))

    const id = tabs.tabList.value.at(-1).id
    expect(id.startsWith('\u0000')).toBe(true)
    expect(tabs.isTerminalTab(id)).toBe(true)
    expect(tabs.isTerminalTab('src/main.rs')).toBe(false)
    expect(tabs.isDiffTab(id)).toBe(false)
    // The session behind it, which is what the pane is drawn from.
    expect(tabs.terminalTab(id).session).toBe(7)
  })

  it('numbers the shells as a person counts them, agents in between or not', async () => {
    await listed(shell({ id: 1 }), session({ id: 2 }), shell({ id: 3 }))

    expect(tabs.tabList.value.map((tab) => tab.label)).toEqual([
      'Kanban',
      'Agent',
      'Terminal 1',
      'Terminal 2'
    ])
  })

  it('leaves no trace in the remembered tab list', async () => {
    await listed(shell({ id: 2 }))
    state().activeTab = tabs.tabList.value.at(-1).id

    expect(state().openTabs).toEqual([])
    expect(state().previewTab).toBe(null)
    // And a restart with that id in `activeTab` opens on something that exists.
    await tabs.restoreTabs()
    expect(state().activeTab).toBe('kanban')
  })

  /* Closing the tab and killing the shell are one act, exactly as closing a
     terminal window is: a tab that only hid a live shell would leave a process
     nobody can see and nobody will remember to stop. */
  it('closing it kills the session, through the one path that kills', async () => {
    await listed(shell({ id: 2 }))
    const id = tabs.tabList.value.at(-1).id
    state().activeTab = id

    await tabs.closeTerminalTab(id)

    expect(ipc.calls('terminal_remove')).toEqual([{ id: 2 }])
    expect(ids()).toEqual(['kanban'])
    expect(state().activeTab).toBe('kanban')
  })

  it('closing one of two lands on its neighbour', async () => {
    await listed(shell({ id: 2 }), shell({ id: 3 }))
    const [first, second] = tabs.tabList.value.slice(-2).map((tab) => tab.id)
    state().activeTab = first

    await tabs.closeTerminalTab(first)

    expect(state().activeTab).toBe(second)
    expect(tabs.tabList.value.at(-1).label).toBe('Terminal 1')
  })

  /* A refusal leaves the shell running and its tab in the row, and moving away
     from a tab that is still there would be a second failure on top of the
     first. */
  it('a refused kill leaves the tab alone', async () => {
    await listed(shell({ id: 2 }))
    const id = tabs.tabList.value.at(-1).id
    state().activeTab = id
    ipc.fail('terminal_remove', new Error('boom'))

    await tabs.closeTerminalTab(id)

    expect(state().activeTab).toBe(id)
    expect(ids()).toContain(id)
  })

  /* Deriving instead of storing is what makes this free: `loadSessions` brings
     the new project's sessions and the tabs follow, with nothing to reset. */
  it('does not follow the person to another project', async () => {
    await listed(shell({ id: 2 }))
    expect(ids()).toHaveLength(2)

    ipc.on('terminal_list', [])
    await terminals.loadSessions('/elsewhere')

    expect(ids()).toEqual(['kanban'])
  })
})

describe('a shell is not an agent', () => {
  it('has no row in the agents panel and no Agent tab behind it', async () => {
    await listed(shell({ id: 2 }), shell({ id: 3 }))

    expect(terminals.agentRows.value).toEqual([])
    expect(tabs.hasAgentTab.value).toBe(false)
    expect(ids()).toEqual(['kanban', '\u0000term:2', '\u0000term:3'])
  })

  it('opening one does not move the selected agent', async () => {
    await listed(session({ id: 4 }))
    ipc.on('terminal_shell', shell({ id: 5 }))

    await terminals.createShell('/p')

    expect(terminals.terminalState.activeId).toBe(4)
    expect(ipc.calls('terminal_shell')).toEqual([{ project: '/p' }])
    expect(tabs.tabList.value.map((tab) => tab.label)).toEqual(['Kanban', 'Agent', 'Terminal 1'])
  })
})

/* The one thing about a shell tab that is only true in a running app: the pane
   is handed a session id as a prop, so two open shells cannot show one
   scrollback. There is no component test runner here, so what is pinned is the
   store's half — the two tabs name two different sessions. */
describe('two shells', () => {
  it('name two different sessions', async () => {
    await listed(shell({ id: 2 }), shell({ id: 3 }))

    const shells = tabs.tabList.value.slice(-2).map((tab) => tabs.terminalTab(tab.id).session)
    expect(shells).toEqual([2, 3])
  })
})
