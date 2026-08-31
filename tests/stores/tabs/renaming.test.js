import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../../support/stores.js'
import { fileText } from '../../support/fixtures.js'

/* A file that has been renamed or moved on disk, and the tab standing over it.
   The id changes and nothing else does — which is the decision this file exists
   to pin. Closing the tab was the alternative, the way a delete closes it, and
   it throws away a person's place in the file for nothing: the file is still
   there, its mtime has not moved, and a dirty buffer is as valid as it was a
   moment ago. */

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

describe('renameTab', () => {
  it('a renamed file keeps its buffer, its unsaved text and its dirtiness', async () => {
    await opened('src/a.txt', { permanent: true })
    tabs.setText('src/a.txt', 'half a sentence')

    tabs.renameTab('src/a.txt', 'src/b.txt')

    expect(tabs.buffers.get('src/b.txt').text).toBe('half a sentence')
    expect(tabs.buffers.has('src/a.txt')).toBe(false)
    expect(tabs.isDirty('src/b.txt')).toBe(true)
  })

  it('carries the timestamp across, since the file is the same file', async () => {
    // What makes the buffer worth keeping at all: a rename does not touch the
    // mtime, so the next save is not stale and asks nothing.
    ipc.on('files_read', () => fileText({ text: 'on disk', mtime: 4242 }))
    await opened('src/a.txt', { permanent: true })

    tabs.renameTab('src/a.txt', 'src/b.txt')

    expect(tabs.buffers.get('src/b.txt').mtime).toBe(4242)
    expect(tabs.isDirty('src/b.txt')).toBe(false)
  })

  it('keeps the tab where it was in the row, and active if it was active', async () => {
    await opened('a.txt', { permanent: true })
    await opened('b.txt', { permanent: true })
    await opened('c.txt', { permanent: true })

    tabs.renameTab('b.txt', 'renamed.txt')

    expect(state().openTabs).toEqual(['a.txt', 'renamed.txt', 'c.txt'])
    expect(state().activeTab).toBe('c.txt')

    tabs.renameTab('c.txt', 'last.txt')
    expect(state().activeTab).toBe('last.txt')
  })

  it('follows the arranged order rather than sending the tab to the end of the row', async () => {
    await opened('a.js', { permanent: true })
    await opened('b.js', { permanent: true })
    state().tabOrder = ['b.js', 'a.js']

    tabs.renameTab('b.js', 'c.js')

    expect(state().tabOrder).toEqual(['c.js', 'a.js'])
    expect(tabs.tabList.value.map((tab) => tab.id)).toEqual(['kanban', 'c.js', 'a.js'])
  })

  it('takes an older mention of the new name out of the order, so the tab is at one position', async () => {
    // `orderTabs` reads the first mention. Closing a tab prunes nothing from
    // the stored order, so a name used before can still be sitting in it.
    await opened('b.js', { permanent: true })
    state().tabOrder = ['c.js', 'b.js', 'a.js']

    tabs.renameTab('b.js', 'c.js')

    expect(state().tabOrder).toEqual(['c.js', 'a.js'])
  })

  it('a renamed preview tab is still the preview, and still italic', async () => {
    await opened('a.txt')

    tabs.renameTab('a.txt', 'b.txt')

    expect(state().previewTab).toBe('b.txt')
    expect(tabs.tabList.value.find((tab) => tab.id === 'b.txt').kind).toBe('preview')
  })

  it('does nothing at all for a path no tab is open over', async () => {
    await opened('a.txt', { permanent: true })

    tabs.renameTab('elsewhere.txt', 'other.txt')

    expect(state().openTabs).toEqual(['a.txt'])
    expect(tabs.buffers.has('other.txt')).toBe(false)
  })

  it('refuses to land on a path that already has a tab, rather than merging two buffers', async () => {
    // Neither back-end verb can produce this — a rename onto a taken name is
    // refused and a move lands on a free one — and there is no right answer to
    // which of the two texts would survive.
    await opened('a.txt', { permanent: true })
    await opened('b.txt', { permanent: true })
    tabs.setText('b.txt', 'typed into b')

    tabs.renameTab('a.txt', 'b.txt')

    expect(state().openTabs).toEqual(['a.txt', 'b.txt'])
    expect(tabs.buffers.get('b.txt').text).toBe('typed into b')
  })

  it('asks again under the new name when the first read was still in flight', async () => {
    // The read was fired against the old path and its guard on the way back is
    // `buffers.has(from)`, which the rename has just made false. Without the
    // second read the tab would sit loading for ever.
    tabs.openFile('src/a.txt', { permanent: true })
    tabs.renameTab('src/a.txt', 'src/b.txt')

    await vi.waitFor(() => expect(tabs.buffers.get('src/b.txt').loading).toBe(false))
    expect(tabs.buffers.get('src/b.txt').text).toBe('text of src/b.txt')
  })
})
