import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../../support/stores.js'
import { buffer, fileText } from '../../support/fixtures.js'

let ipc
let files
let settings
let tabs

/* The project's state is settings.project: that is what tabs.js reads and
   writes. */
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

describe('a single click', () => {
  it('opens the file as a preview tab and makes it active', async () => {
    await opened('a.txt')

    expect(state().openTabs).toEqual(['a.txt'])
    expect(state().previewTab).toBe('a.txt')
    expect(state().activeTab).toBe('a.txt')
    expect(tabs.buffers.get('a.txt').text).toBe('text of a.txt')
  })

  it('the next click replaces the preview in place rather than growing the row', async () => {
    await opened('a.txt')
    await opened('b.txt')

    expect(state().openTabs).toEqual(['b.txt'])
    expect(state().previewTab).toBe('b.txt')
    expect(tabs.buffers.has('a.txt')).toBe(false)
  })

  it('the replacement happens at the same position in the row', async () => {
    await opened('a.txt', { permanent: true })
    await opened('b.txt')
    await opened('c.txt')

    expect(state().openTabs).toEqual(['a.txt', 'c.txt'])
  })
})

describe('a double click', () => {
  it("opens a permanent tab next to it and does not evict somebody else's preview", async () => {
    await opened('a.txt')
    await opened('b.txt', { permanent: true })

    expect(state().openTabs).toEqual(['a.txt', 'b.txt'])
    expect(state().previewTab).toBe('a.txt')
  })

  it('makes permanent a tab that is already open as a preview', async () => {
    await opened('a.txt')
    tabs.openFile('a.txt', { permanent: true })

    expect(state().previewTab).toBe(null)
    expect(state().openTabs).toEqual(['a.txt'])
  })

  it('promote drops the temporary flag and makes the tab active', async () => {
    await opened('a.txt')
    tabs.promote('a.txt')

    expect(state().previewTab).toBe(null)
    expect(state().activeTab).toBe('a.txt')
  })
})

describe('reopening what is already open', () => {
  it('only switches the active tab and does not re-read the file', async () => {
    await opened('a.txt', { permanent: true })
    await opened('b.txt', { permanent: true })
    const before = ipc.calls('files_read').length

    tabs.openFile('a.txt')

    expect(state().activeTab).toBe('a.txt')
    expect(ipc.calls('files_read')).toHaveLength(before)
  })
})

describe('the first edit makes the tab permanent', () => {
  it('setText drops the temporary flag — that is what makes "a preview tab is never dirty" true', async () => {
    await opened('a.txt')

    tabs.setText('a.txt', 'edited')

    expect(state().previewTab).toBe(null)
    expect(tabs.isDirty('a.txt')).toBe(true)
  })

  it('an edit into a buffer whose first read has not returned does not go through', async () => {
    tabs.openFile('a.txt')
    expect(tabs.buffers.get('a.txt').loading).toBe(true)

    tabs.setText('a.txt', 'too early')

    expect(tabs.buffers.get('a.txt').text).toBe('')
    await vi.waitFor(() => expect(tabs.buffers.get('a.txt').loading).toBe(false))
    expect(tabs.buffers.get('a.txt').text).toBe('text of a.txt')
  })

  it('an edit into a buffer with a read refusal does not go through', async () => {
    ipc.fail('files_read', { kind: 'binary', message: 'binary' })
    await opened('a.png')

    tabs.setText('a.png', 'something')

    expect(tabs.buffers.get('a.png').text).toBe('')
  })
})

describe('closing', () => {
  const openThree = async () => {
    await opened('a.txt', { permanent: true })
    await opened('b.txt', { permanent: true })
    await opened('c.txt', { permanent: true })
  }

  it('the neighbour on the right becomes active', async () => {
    await openThree()
    state().activeTab = 'b.txt'

    tabs.closeTab('b.txt')

    expect(state().openTabs).toEqual(['a.txt', 'c.txt'])
    expect(state().activeTab).toBe('c.txt')
  })

  it('for the last one it is the neighbour on the left', async () => {
    await openThree()
    state().activeTab = 'c.txt'

    tabs.closeTab('c.txt')

    expect(state().activeTab).toBe('b.txt')
  })

  it('an emptied row returns to the board', async () => {
    await opened('a.txt', { permanent: true })

    tabs.closeTab('a.txt')

    expect(state().openTabs).toEqual([])
    expect(state().activeTab).toBe('kanban')
    expect(tabs.buffers.has('a.txt')).toBe(false)
  })

  it('closing an inactive tab does not move the active one', async () => {
    await openThree()
    state().activeTab = 'a.txt'

    tabs.closeTab('c.txt')

    expect(state().activeTab).toBe('a.txt')
  })

  it('closing something that is not there does nothing', async () => {
    await opened('a.txt', { permanent: true })

    tabs.closeTab('no-such-file.txt')

    expect(state().openTabs).toEqual(['a.txt'])
  })
})

describe('tabList', () => {
  it('the pinned tabs come first and cannot be closed', async () => {
    await opened('a.txt')

    const list = tabs.tabList.value

    expect(list[0]).toMatchObject({ id: 'kanban', kind: 'pinned' })
  })

  it('a preview tab is marked with its own kind', async () => {
    await opened('a.txt')

    expect(tabs.tabList.value[1]).toMatchObject({ id: 'a.txt', kind: 'preview', label: 'a.txt' })
  })

  it('a tab with a read refusal carries the lock and its reason', async () => {
    ipc.fail('files_read', { kind: 'tooLarge', message: 'too large' })
    await opened('big.log')

    expect(tabs.tabList.value[1]).toMatchObject({
      readOnly: true,
      readOnlyHint: 'File is too large to open here.'
    })
  })

  it('the first read gets no lock: a lock blinking on every open would lie', async () => {
    tabs.openFile('a.txt')

    /* Without this, readOnly === false would be true even when the buffer is
       not in the map at all (readOnlyHint(undefined) gives null just as
       readOnlyHint without an error does) — the assert below would be checking
       emptiness rather than that the tab really is in the loading state with no
       lock. */
    expect(tabs.buffers.get('a.txt').loading).toBe(true)
    expect(tabs.tabList.value[1].readOnly).toBe(false)
  })

  it("a tab's label is only the last segment of the path", async () => {
    await opened('src/stores/tabs.js')

    expect(tabs.tabList.value[1].label).toBe('tabs.js')
  })

  /* The board is the one tab a project always has. The Agent tab used to sit
     beside it and is now derived from the sessions — see
     tests/stores/tabs/terminal.test.js, which is where both halves of that
     live. */
  it('the board is the only tab that is always there', async () => {
    const { stores } = await loadStores()
    expect(stores.tabs.PINNED.map((t) => t.id)).toEqual(['kanban'])
  })
})

describe('dirtiness', () => {
  it('a buffer with a read refusal does not count as dirty on its own', async () => {
    ipc.fail('files_read', { kind: 'notFound', message: 'gone' })
    await opened('a.txt')

    expect(tabs.isDirty('a.txt')).toBe(false)
  })

  it('text typed before a read refusal has to count as unsaved', async () => {
    tabs.buffers.set('a.txt', buffer({ text: 'typed', original: '', error: { kind: 'io' } }))

    expect(tabs.isDirty('a.txt')).toBe(true)
  })

  it('dirtyPaths lists only the dirty ones among the open tabs', async () => {
    await opened('a.txt', { permanent: true })
    await opened('b.txt', { permanent: true })
    tabs.setText('b.txt', 'an edit')

    expect(tabs.dirtyPaths.value).toEqual(['b.txt'])
  })
})
