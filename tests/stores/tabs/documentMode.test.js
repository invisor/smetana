import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../../support/stores.js'
import { fileText } from '../../support/fixtures.js'

/* The eye/code mode of an html tab: which tabs are shown as source, and what
   keeps that set in step with the row.

   It is a store test rather than a component one for the reason the whole of
   this directory is: the toggle itself is a `.vue` file no runner here can
   reach, so the whole of the state it changes lives in `tabs.js` and only the
   press lives in the component. */

let ipc
let files
let settings
let tabs

const state = () => settings.settings.project

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  files = loaded.stores.files
  settings = loaded.stores.settings
  tabs = loaded.stores.tabs
  files.setRoot('/project')
  ipc.on('files_read', (args) => fileText({ path: args.path, text: `text of ${args.path}` }))
})

const opened = async (path, options) => {
  tabs.openFile(path, options)
  await vi.waitFor(() => expect(tabs.buffers.get(path).loading).toBe(false))
}

describe('the mode of an html tab', () => {
  it('is the document until somebody says otherwise', async () => {
    await opened('docs/page.html', { permanent: true })

    expect(tabs.showsSource('docs/page.html')).toBe(false)
  })

  it('is answered for a path nothing has opened, since the view asks first', () => {
    expect(tabs.showsSource('docs/never-opened.html')).toBe(false)
  })

  it('turns to source on a press and back on the next one', async () => {
    await opened('docs/page.html', { permanent: true })

    tabs.toggleSource('docs/page.html')
    expect(tabs.showsSource('docs/page.html')).toBe(true)

    tabs.toggleSource('docs/page.html')
    expect(tabs.showsSource('docs/page.html')).toBe(false)
  })

  it('is one tab at a time and never the whole row', async () => {
    await opened('docs/one.html', { permanent: true })
    await opened('docs/two.html', { permanent: true })

    tabs.toggleSource('docs/one.html')

    expect(tabs.showsSource('docs/one.html')).toBe(true)
    expect(tabs.showsSource('docs/two.html')).toBe(false)
  })

  it('survives a switch to another tab and back, which is what storing it is for', async () => {
    await opened('docs/page.html', { permanent: true })
    await opened('notes.md', { permanent: true })
    tabs.toggleSource('docs/page.html')

    state().activeTab = 'notes.md'
    state().activeTab = 'docs/page.html'

    expect(tabs.showsSource('docs/page.html')).toBe(true)
  })
})

describe('what makes a tab stop being the tab it was', () => {
  it('closing it forgets the mode, so reopening the file shows the document', async () => {
    await opened('docs/page.html', { permanent: true })
    tabs.toggleSource('docs/page.html')

    tabs.closeTab('docs/page.html')
    await opened('docs/page.html', { permanent: true })

    expect(tabs.showsSource('docs/page.html')).toBe(false)
  })

  it('a rename carries the mode to the new path, the way the buffer travels', async () => {
    await opened('docs/page.html', { permanent: true })
    tabs.toggleSource('docs/page.html')

    tabs.renameTab('docs/page.html', 'docs/report.html')

    expect(tabs.showsSource('docs/page.html')).toBe(false)
    expect(tabs.showsSource('docs/report.html')).toBe(true)
  })

  it('a rename of a tab nobody toggled leaves the new path a document', async () => {
    await opened('docs/page.html', { permanent: true })

    tabs.renameTab('docs/page.html', 'docs/report.html')

    expect(tabs.showsSource('docs/report.html')).toBe(false)
  })

  it('a preview evicted from its slot takes its mode with it', async () => {
    // The evicted path is gone from the row entirely, so a mode left behind
    // would greet the next preview of that same file with somebody else's
    // choice about a tab that no longer exists.
    await opened('docs/page.html')
    tabs.toggleSource('docs/page.html')

    await opened('docs/other.html')
    await opened('docs/page.html')

    expect(tabs.showsSource('docs/page.html')).toBe(false)
  })

  it('a project switch empties the set with the buffers', async () => {
    await opened('docs/page.html', { permanent: true })
    tabs.toggleSource('docs/page.html')

    tabs.resetTabs()

    expect(tabs.showsSource('docs/page.html')).toBe(false)
  })
})
