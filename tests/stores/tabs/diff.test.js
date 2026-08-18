import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../../support/stores.js'
import { fileText } from '../../support/fixtures.js'

/* The Git panel's diff tabs. What they are worth testing for is not the diff —
   that is CodeMirror's, and a `.vue` file is out of reach here anyway — but the
   one property they were built around: a diff is not a file, and nothing about
   it may reach `settings.json`, where it would come back next launch as a tab
   trying to read a file by that name. */

let ipc
let files
let settings
let tabs

const state = () => settings.settings.project

const REPO = '/project/admin'

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  files = loaded.stores.files
  settings = loaded.stores.settings
  tabs = loaded.stores.tabs
  files.setRoot('/project')
  ipc.on('files_read', (args) => fileText({ path: args.path, text: `working tree of ${args.path}` }))
  ipc.on('vcs_file_at_head', (args) => `HEAD of ${args.path}`)
})

const opened = async (repo, path) => {
  tabs.openDiff(repo, path)
  const id = settings.settings.project.activeTab
  await vi.waitFor(() => expect(tabs.diffTab(id).loading).toBe(false))
  return tabs.diffTab(id)
}

describe('opening a changed file as a diff', () => {
  it('reads HEAD from git and the working tree from the project root', async () => {
    const tab = await opened(REPO, 'src/main.rs')

    expect(tab.head).toBe('HEAD of src/main.rs')
    expect(tab.work).toBe('working tree of admin/src/main.rs')
    expect(tab.error).toBe(null)
    expect(state().activeTab).toBe(tab.id)
  })

  it('leaves no trace in the remembered tab list', async () => {
    // The whole point of the transient list. `openTabs` is written to
    // settings.json and read back on the next launch.
    const tab = await opened(REPO, 'src/main.rs')

    expect(state().openTabs).toEqual([])
    expect(state().previewTab).toBe(null)
    expect(tabs.tabList.value.map((entry) => entry.id)).toContain(tab.id)
  })

  it('appears in the tab row after the files, with its own kind', async () => {
    tabs.openFile('a.txt')
    await opened(REPO, 'src/main.rs')

    expect(tabs.tabList.value.map((entry) => entry.kind)).toEqual(['pinned', 'preview', 'diff'])
  })

  it('clicking the same row again lands on the tab already open and re-reads it', async () => {
    await opened(REPO, 'src/main.rs')
    ipc.on('vcs_file_at_head', () => 'HEAD, one commit later')
    tabs.openDiff(REPO, 'src/main.rs')
    await vi.waitFor(() => expect(tabs.diffTabs[0].head).toBe('HEAD, one commit later'))

    expect(tabs.diffTabs).toHaveLength(1)
  })

  it('two files of two repositories are two tabs', async () => {
    await opened(REPO, 'src/main.rs')
    await opened('/project', 'src/main.rs')

    expect(tabs.diffTabs).toHaveLength(2)
  })
})

describe('a file HEAD does not have', () => {
  it('opens against an empty left side and says which emptiness it is', async () => {
    // An added or untracked file: `null` is the answer and not a failure.
    ipc.on('vcs_file_at_head', () => null)
    const tab = await opened(REPO, 'notes/todo.txt')

    expect(tab.head).toBe('')
    expect(tab.missingAtHead).toBe(true)
    expect(tab.error).toBe(null)
  })

  it('a file the working tree does not have is an empty right side, not a refusal', async () => {
    // The diff of a deletion is exactly this.
    ipc.fail('files_read', { kind: 'notFound', message: 'no such file' })
    const tab = await opened(REPO, 'src/gone.rs')

    expect(tab.work).toBe('')
    expect(tab.error).toBe(null)
  })
})

describe('what a diff refuses', () => {
  it('carries the refusal for the view to caption', async () => {
    ipc.fail('vcs_file_at_head', { kind: 'binary', message: 'binary file: assets/logo.png' })
    const tab = await opened(REPO, 'assets/logo.png')

    expect(tab.error).toEqual({ kind: 'binary', message: 'binary file: assets/logo.png' })
  })

  it('a repository outside the project is refused here rather than by the disk', async () => {
    // `files_read` takes a path relative to the project root and would be right
    // to refuse this one; `[project].repos` may name a folder anywhere.
    const tab = await opened('/elsewhere/admin', 'src/main.rs')

    expect(tab.error.kind).toBe('outside')
    expect(ipc.commands()).not.toContain('files_read')
  })
})

describe('closing and leaving', () => {
  it('a close takes the tab out and moves back to the files', async () => {
    tabs.openFile('a.txt', { permanent: true })
    const tab = await opened(REPO, 'src/main.rs')
    tabs.closeDiff(tab.id)

    expect(tabs.diffTabs).toHaveLength(0)
    expect(state().activeTab).toBe('a.txt')
  })

  it('with nothing else open the board takes over', async () => {
    const tab = await opened(REPO, 'src/main.rs')
    tabs.closeDiff(tab.id)

    expect(state().activeTab).toBe('kanban')
  })

  it('switching project takes the diffs with the buffers', async () => {
    await opened(REPO, 'src/main.rs')
    tabs.resetTabs()

    expect(tabs.diffTabs).toHaveLength(0)
  })
})

describe('a restart', () => {
  it('an activeTab naming a diff tab falls back rather than drawing nothing', async () => {
    // `activeTab` is remembered and the diff is not, so this state is reachable
    // from a settings file — and from the browser mock, which answers
    // settings_load itself and has no such check.
    const tab = await opened(REPO, 'src/main.rs')
    const stale = tab.id
    tabs.resetTabs()
    state().activeTab = stale
    state().openTabs = ['a.txt']
    await tabs.restoreTabs()

    expect(state().activeTab).toBe('a.txt')
  })

  it('with no files open either, it falls back to the board', async () => {
    const tab = await opened(REPO, 'src/main.rs')
    const stale = tab.id
    tabs.resetTabs()
    state().activeTab = stale
    state().openTabs = []
    await tabs.restoreTabs()

    expect(state().activeTab).toBe('kanban')
  })
})
