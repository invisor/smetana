import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../../support/stores.js'
import { fileText } from '../../support/fixtures.js'

/* The store's half of the tab row's arrangement: `tabList` laid out by the
   project's `tabOrder`, the preview tab keeping its place in it, and the one
   neighbour rule the three close paths now share.

   The rule itself is `components/shell/tabOrder.js`'s and is tested there. What
   is worth pinning here is that this store asks it about the row it draws
   rather than about the lists the row is glued from — the whole reason the
   three copies were collapsed into one. */

let ipc
let settings
let tabs
let terminals

const state = () => settings.settings.project

const ids = () => tabs.tabList.value.map((tab) => tab.id)

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
  ipc.on('vcs_file_at_head', 'HEAD of it')
  ipc.on('terminal_list', [])
  /* Read beside the session list, in the same `loadSessions`, so every test
     here reaches it too. Answered with nothing: what a project offers back
     after a restart is `terminals.test.js`'s subject, and an unregistered
     command would have this file exercising a caught failure instead. */
  ipc.on('terminal_restorable', [])
  ipc.on('terminal_remove', null)
})

const opened = async (path) => {
  tabs.openFile(path, { permanent: true })
  await vi.waitFor(() => expect(tabs.buffers.get(path).loading).toBe(false))
}

const listed = async (...sessions) => {
  ipc.on('terminal_list', sessions)
  await terminals.loadSessions('/p')
}

describe('the row is drawn in the stored order', () => {
  it('leaves the row as it grew when nothing was ever rearranged', async () => {
    await opened('a.js')
    await opened('b.js')

    expect(ids()).toEqual(['kanban', 'a.js', 'b.js'])
  })

  it('draws the file tabs in it', async () => {
    await opened('a.js')
    await opened('b.js')
    await opened('c.js')
    state().tabOrder = ['c.js', 'a.js', 'b.js']

    expect(ids()).toEqual(['kanban', 'c.js', 'a.js', 'b.js'])
    // And `openTabs` is untouched by it: the two lists answer different questions.
    expect(state().openTabs).toEqual(['a.js', 'b.js', 'c.js'])
  })

  /* The order is common and not per group, which is the whole of the request: a
     shell can stand between two files and a diff in front of all of them. */
  it('puts a terminal between two files and a diff before them', async () => {
    await opened('a.js')
    await opened('b.js')
    tabs.openDiff('/p', 'src/main.rs')
    await listed(shell({ id: 2 }))
    const diff = tabs.diffTabs[0].id
    const term = ids().at(-1)

    state().tabOrder = [diff, 'a.js', term, 'b.js']

    expect(ids()).toEqual(['kanban', diff, 'a.js', term, 'b.js'])
  })

  it('never moves the pinned run, whatever the order says', async () => {
    await listed(session({ id: 4 }))
    await opened('a.js')
    state().tabOrder = ['a.js', 'kanban', 'terminal']

    expect(ids()).toEqual(['kanban', 'terminal', 'a.js'])
  })

  it('puts a newly opened tab at the end of the row', async () => {
    await opened('a.js')
    await opened('b.js')
    state().tabOrder = ['b.js', 'a.js']

    await opened('c.js')

    expect(ids()).toEqual(['kanban', 'b.js', 'a.js', 'c.js'])
  })

  /* After a restart only the file tabs come back, so every diff and shell in the
     stored order names nothing. Those entries shift nobody, and they are not
     swept on the way in — the next drag rewrites the field whole. */
  it('shifts nobody for an id no tab matches', async () => {
    await opened('a.js')
    await opened('b.js')
    state().tabOrder = ['\u0000term:9', 'b.js', '\u0000diff:/p\u0000gone.rs', 'a.js']

    expect(ids()).toEqual(['kanban', 'b.js', 'a.js'])
  })
})

describe('a preview tab keeps its place', () => {
  it('the next single click replaces it where it stands in the arranged row', async () => {
    await opened('a.js')
    await opened('b.js')
    tabs.openFile('preview.js')
    await vi.waitFor(() => expect(tabs.buffers.get('preview.js').loading).toBe(false))
    // Dragged to the front of the row, the way a person would.
    state().tabOrder = ['preview.js', 'a.js', 'b.js']

    tabs.openFile('next.js')

    expect(ids()).toEqual(['kanban', 'next.js', 'a.js', 'b.js'])
    expect(state().tabOrder).toEqual(['next.js', 'a.js', 'b.js'])
  })

  it('writes nothing to the order when nobody has rearranged anything', async () => {
    tabs.openFile('a.js')
    tabs.openFile('b.js')

    expect(state().tabOrder).toEqual([])
    expect(ids()).toEqual(['kanban', 'b.js'])
  })

  /* A path can already be in the order from a drag before it was closed —
     closing a tab prunes nothing — and the older entry is as likely to sit in
     front of the preview's slot as behind it. `orderTabs` takes the first
     mention, so a survivor to the left would draw the incoming file at the old
     position rather than in the slot the outgoing preview held, and `sane_list`
     in Rust would drop the second on the way to disk and cement it. */
  it('leaves no second mention of a file the order already knew, on either side', async () => {
    await opened('b.js')
    tabs.openFile('p.js')
    await vi.waitFor(() => expect(tabs.buffers.get('p.js').loading).toBe(false))
    // `x.js` was dragged to the front of the row and closed since; the preview
    // sits behind `b.js`, which is where the incoming tab has to land.
    state().tabOrder = ['x.js', 'b.js', 'p.js']

    tabs.openFile('x.js')

    expect(state().tabOrder).toEqual(['b.js', 'x.js'])
    expect(ids()).toEqual(['kanban', 'b.js', 'x.js'])
  })

  it('and the same when the older mention sits behind the slot', async () => {
    await opened('b.js')
    tabs.openFile('p.js')
    await vi.waitFor(() => expect(tabs.buffers.get('p.js').loading).toBe(false))
    state().tabOrder = ['p.js', 'b.js', 'x.js']

    tabs.openFile('x.js')

    expect(state().tabOrder).toEqual(['x.js', 'b.js'])
    expect(ids()).toEqual(['kanban', 'x.js', 'b.js'])
  })
})

describe('what becomes active when a tab is closed', () => {
  it('is the neighbour on the right of the drawn row, not of openTabs', async () => {
    await opened('a.js')
    await opened('b.js')
    await opened('c.js')
    state().tabOrder = ['c.js', 'b.js', 'a.js']
    state().activeTab = 'b.js'

    tabs.closeTab('b.js')

    expect(state().activeTab).toBe('a.js')
  })

  it('is the neighbour on the left for the last tab of the drawn row', async () => {
    await opened('a.js')
    await opened('b.js')
    state().tabOrder = ['b.js', 'a.js']
    state().activeTab = 'a.js'

    tabs.closeTab('a.js')

    expect(state().activeTab).toBe('b.js')
  })

  it('is the board with nothing else left in the row', async () => {
    await opened('a.js')
    state().activeTab = 'a.js'

    tabs.closeTab('a.js')

    expect(state().activeTab).toBe('kanban')
  })

  /* The one behaviour that changes without anybody dragging anything, and it is
     the correct reading rather than a side effect: the neighbour on the right is
     the terminal, which is what the comment always promised and what the person
     sees beside the tab they closed. */
  it('crosses from a file to the terminal beside it', async () => {
    await opened('a.js')
    await listed(shell({ id: 2 }))
    const term = ids().at(-1)
    state().activeTab = 'a.js'

    tabs.closeTab('a.js')

    expect(state().activeTab).toBe(term)
  })

  it('answers the same way for a diff tab', async () => {
    await opened('a.js')
    tabs.openDiff('/p', 'src/main.rs')
    await listed(shell({ id: 2 }))
    const diff = tabs.diffTabs[0].id
    const term = ids().at(-1)
    state().activeTab = diff

    tabs.closeDiff(diff)

    expect(state().activeTab).toBe(term)
  })

  it('answers the same way for a terminal tab', async () => {
    await opened('a.js')
    await listed(shell({ id: 2 }))
    const term = ids().at(-1)
    // Dragged in front of the file, so the neighbour on the right is that file.
    state().tabOrder = [term, 'a.js']
    state().activeTab = term

    await tabs.closeTerminalTab(term)

    expect(state().activeTab).toBe('a.js')
  })

  /* The pinned run is deliberately not a neighbour: with the board always in the
     row there would be no "nobody left" case at all, and closing the last file
     while an agent runs would land on the Agent tab rather than on the board. */
  it('does not fall onto the Agent tab when the row empties', async () => {
    await listed(session({ id: 4 }))
    await opened('a.js')
    state().activeTab = 'a.js'

    tabs.closeTab('a.js')

    expect(state().activeTab).toBe('kanban')
  })
})
