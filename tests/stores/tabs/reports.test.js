import { beforeEach, describe, expect, it } from 'vitest'
import { loadStores } from '../../support/stores.js'

/* The Reports tab: the sixth kind, and the first that names no file, no
   repository and no session. What is worth testing here is the same thing
   `diff.test.js` tests for a diff tab — it is not remembered, it is closed
   like a file through the row's own `neighbourIn` rule, and a project switch
   takes it away. */

let ipc
let settings
let tabs

const state = () => settings.settings.project
const ids = () => tabs.tabList.value.map((tab) => tab.id)

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  settings = loaded.stores.settings
  tabs = loaded.stores.tabs
})

describe('opening the Reports tab', () => {
  it('is not there before it is opened', () => {
    expect(ids()).toEqual(['kanban'])
    expect(tabs.reportsTabOpen.value).toBe(false)
  })

  it('appears at the end of the row and becomes active', () => {
    tabs.openFile('a.txt')

    tabs.openReportsTab()

    expect(ids()).toEqual(['kanban', 'a.txt', tabs.REPORTS_TAB_ID])
    expect(tabs.tabList.value.at(-1)).toMatchObject({
      kind: 'reports',
      label: 'Reports',
      icon: 'scroll-text'
    })
    expect(state().activeTab).toBe(tabs.REPORTS_TAB_ID)
    expect(tabs.reportsTabOpen.value).toBe(true)
  })

  /* Sits after the terminals too, the same "after everything nothing
     remembers" place the diffs and the shells already share. */
  it('sits after the diffs and the terminals', async () => {
    ipc.on('vcs_file_at_head', 'HEAD of it')
    ipc.on('terminal_list', [])
    tabs.openFile('a.txt')
    tabs.openDiff('/p', 'src/main.rs')
    await tabs.restoreTabs()

    tabs.openReportsTab()

    expect(tabs.tabList.value.map((tab) => tab.kind)).toEqual([
      'pinned',
      'preview',
      'diff',
      'reports'
    ])
  })

  it('pressing the button again while it is open activates it rather than opening a second one', () => {
    tabs.openFile('a.txt')
    tabs.openReportsTab()
    state().activeTab = 'a.txt'

    tabs.openReportsTab()

    expect(ids().filter((id) => id === tabs.REPORTS_TAB_ID)).toHaveLength(1)
    expect(state().activeTab).toBe(tabs.REPORTS_TAB_ID)
  })

  it('leaves no trace in the remembered tab list', () => {
    tabs.openReportsTab()

    expect(state().openTabs).toEqual([])
    expect(state().previewTab).toBe(null)
  })

  it('is identified by something no file path can be', () => {
    tabs.openReportsTab()

    expect(tabs.REPORTS_TAB_ID.startsWith('\u0000')).toBe(true)
    expect(tabs.isReportsTab(tabs.REPORTS_TAB_ID)).toBe(true)
    expect(tabs.isReportsTab('src/main.rs')).toBe(false)
  })
})

describe('closing the Reports tab', () => {
  it('with no neighbour, lands on the board', () => {
    tabs.openReportsTab()

    tabs.closeReportsTab()

    expect(ids()).toEqual(['kanban'])
    expect(state().activeTab).toBe('kanban')
    expect(tabs.reportsTabOpen.value).toBe(false)
  })

  it('closing the active tab lands on its neighbour', () => {
    tabs.openFile('a.txt')
    tabs.openReportsTab()

    tabs.closeReportsTab()

    expect(ids()).toEqual(['kanban', 'a.txt'])
    expect(state().activeTab).toBe('a.txt')
  })

  /* Closing a tab that is not the active one must not move the person off
     whatever they are looking at — the same rule `closeTab` and `closeDiff`
     already keep. */
  it('closing it while looking at something else does not move the selection', () => {
    tabs.openFile('a.txt')
    tabs.openReportsTab()
    state().activeTab = 'a.txt'

    tabs.closeReportsTab()

    expect(state().activeTab).toBe('a.txt')
    expect(ids()).toEqual(['kanban', 'a.txt'])
  })

  it('closing it twice is harmless', () => {
    tabs.openReportsTab()
    tabs.closeReportsTab()

    tabs.closeReportsTab()

    expect(ids()).toEqual(['kanban'])
    expect(state().activeTab).toBe('kanban')
  })
})

describe('a project switch', () => {
  it('takes the tab away, the same as a diff', () => {
    tabs.openReportsTab()
    expect(tabs.reportsTabOpen.value).toBe(true)

    tabs.resetTabs()

    expect(tabs.reportsTabOpen.value).toBe(false)
    expect(ids()).toEqual(['kanban'])
  })
})
