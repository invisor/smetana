import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../../support/stores.js'
import { buffer, fileText } from '../../support/fixtures.js'

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

describe('markStale', () => {
  it('raises the strip', () => {
    tabs.buffers.set('a.txt', buffer({ text: 'an edit' }))

    tabs.markStale('a.txt')

    expect(tabs.buffers.get('a.txt').stale).toBe(true)
  })

  it('clears a previous read refusal: the file is back, locking the field forever is not on', () => {
    tabs.buffers.set('a.txt', buffer({ text: 'an edit', error: { kind: 'notFound' } }))

    tabs.markStale('a.txt')

    expect(tabs.buffers.get('a.txt').error).toBe(null)
    expect(tabs.buffers.get('a.txt').stale).toBe(true)
  })

  it('does not touch a loading buffer', () => {
    tabs.buffers.set('a.txt', buffer({ loading: true }))

    tabs.markStale('a.txt')

    expect(tabs.buffers.get('a.txt').stale).toBe(false)
  })
})

describe('markGone', () => {
  it('attaches a read refusal — the lock and the explanation come with it', () => {
    tabs.buffers.set('a.txt', buffer())

    tabs.markGone('a.txt')

    expect(tabs.buffers.get('a.txt').error).toEqual({ kind: 'notFound' })
    expect(tabs.buffers.get('a.txt').stale).toBe(false)
  })

  it('does not overwrite a refusal that is already set', () => {
    tabs.buffers.set('a.txt', buffer({ error: { kind: 'binary' } }))

    tabs.markGone('a.txt')

    expect(tabs.buffers.get('a.txt').error).toEqual({ kind: 'binary' })
  })

  it('does not touch a loading buffer', () => {
    tabs.buffers.set('a.txt', buffer({ loading: true }))

    tabs.markGone('a.txt')

    expect(tabs.buffers.get('a.txt').error).toBe(null)
  })
})

describe('re-reading', () => {
  it('reloadTab gives the win to the disk: the edits go', async () => {
    tabs.buffers.set('a.txt', buffer({ text: 'my edit', original: 'original', stale: true }))

    await tabs.reloadTab('a.txt')

    const current = tabs.buffers.get('a.txt')
    expect(current.text).toBe('text of a.txt')
    expect(current.original).toBe('text of a.txt')
    expect(current.stale).toBe(false)
    expect(tabs.isDirty('a.txt')).toBe(false)
  })

  it('reloadTab of a tab that is not there does not go to the disk', async () => {
    await tabs.reloadTab('no-such-file.txt')

    expect(ipc.calls('files_read')).toHaveLength(0)
  })

  /* A DEFECT, not the intended behaviour. The comment in load() (tabs.js:96-106)
     promises that a re-read without force will not erase what was typed — but
     load() itself resets the buffer to empty before await readFile, and setText
     lets no edits in while loading is set. So line 103 always compares two empty
     strings, and on straight sequential paths (restoreTabs → load without force,
     racing nothing) the protective branch never fires; the same goes for the
     refusal branch (line 123). That does not mean the branch is entirely dead:
     it is reachable through the race with keepMine, which captures the buffer
     before its await and writes the captured one back — see the test below. The
     test here pins what the code does on the straight path, so a fix does not
     pass unnoticed. */
  it('a re-read without force erases what was typed: the protective branch never fires on straight paths', async () => {
    tabs.buffers.set('a.txt', buffer({ text: 'my edit', original: 'original', mtime: 1 }))
    state().openTabs = ['a.txt']

    await tabs.restoreTabs()

    const current = tabs.buffers.get('a.txt')
    expect(current.text).toBe('text of a.txt')
    expect(current.original).toBe('text of a.txt')
    expect(current.mtime).toBe(10)
    expect(tabs.isDirty('a.txt')).toBe(false)
  })

  /* A DEFECT. keepMine captures the buffer into a variable BEFORE its await
     readFile and writes that captured buffer back afterwards (only mtime and
     stale are taken from the answer). If between the capture and the return
     load() managed to reset and re-read the buffer (the disk won), keepMine
     clobbers the fresh disk content with the captured old one — resurrecting
     text that had already been displaced. On a project switch (projects.js
     moveTo: resetTabs() followed by restoreTabs()) this can drag one project's
     dirty buffer into another project's identically named file (README.md,
     package.json — an ordinary path collision).

     The race is assembled deterministically: the first files_read call (from
     keepMine) is held on a controlled promise and is not released until the
     second call (from reloadTab, standing in for load() with no race) has
     finished — the completion order is set explicitly, not by the order in
     which promises resolve. */
  it('a keepMine race with a parallel load resurrects text the disk had already displaced', async () => {
    let releaseKeepMineRead
    const keepMineRead = new Promise((resolve) => {
      releaseKeepMineRead = resolve
    })
    let calls = 0
    ipc.on('files_read', (args) => {
      calls += 1
      if (calls === 1) {
        /* The answer to the read started by keepMine: released by hand below,
           after reloadTab has already won with the disk. */
        return keepMineRead.then(() => fileText({ path: args.path, text: `text of ${args.path}`, mtime: 20 }))
      }
      /* The answer to the read started by reloadTab: returned at once. */
      return fileText({ path: args.path, text: `text of ${args.path}`, mtime: 10 })
    })
    tabs.buffers.set('a.txt', buffer({ text: 'my edit', original: 'original', stale: true, mtime: 1 }))

    const keepMinePromise = tabs.keepMine('a.txt')
    await tabs.reloadTab('a.txt')
    /* The disk won: the buffer is already clean and carries the disk's content. */
    expect(tabs.isDirty('a.txt')).toBe(false)

    releaseKeepMineRead()
    await keepMinePromise

    const current = tabs.buffers.get('a.txt')
    expect(current.text).toBe('my edit')
    expect(current.original).toBe('original')
    expect(current.mtime).toBe(20)
    expect(tabs.isDirty('a.txt')).toBe(true)
  })
})

describe('keepMine', () => {
  it('clears the strip and catches the mtime up, otherwise the next Cmd+S would refuse again', async () => {
    tabs.buffers.set('a.txt', buffer({ text: 'my edit', original: 'original', stale: true, mtime: 1 }))

    await tabs.keepMine('a.txt')

    const current = tabs.buffers.get('a.txt')
    expect(current.stale).toBe(false)
    expect(current.mtime).toBe(10)
    expect(current.text).toBe('my edit')
  })

  it('a read refusal is attached to the buffer while the text stays', async () => {
    ipc.fail('files_read', { kind: 'notFound', message: 'no such file' })
    tabs.buffers.set('a.txt', buffer({ text: 'my edit', stale: true }))

    await tabs.keepMine('a.txt')

    expect(tabs.buffers.get('a.txt').error).toEqual({ kind: 'notFound', message: 'no such file' })
    expect(tabs.buffers.get('a.txt').text).toBe('my edit')
  })
})

describe('restoreTabs', () => {
  it('reads everything that was open', async () => {
    state().openTabs = ['a.txt', 'b.txt']

    await tabs.restoreTabs()

    expect(tabs.buffers.get('a.txt').text).toBe('text of a.txt')
    expect(tabs.buffers.get('b.txt').text).toBe('text of b.txt')
  })

  it('a vanished path silently falls out of the list', async () => {
    ipc.on('files_read', (args) => {
      if (args.path === 'gone.txt') throw { kind: 'notFound', message: 'gone' }
      return fileText({ path: args.path })
    })
    state().openTabs = ['a.txt', 'gone.txt']
    state().activeTab = 'gone.txt'
    state().previewTab = 'gone.txt'

    await tabs.restoreTabs()

    expect(state().openTabs).toEqual(['a.txt'])
    expect(state().activeTab).toBe('kanban')
    expect(state().previewTab).toBe(null)
    expect(tabs.buffers.has('gone.txt')).toBe(false)
  })

  it('a path unreadable for another reason stays: only the vanished one disappears', async () => {
    ipc.on('files_read', (args) => {
      if (args.path === 'big.log') throw { kind: 'tooLarge', message: 'too large' }
      return fileText({ path: args.path })
    })
    state().openTabs = ['big.log']

    await tabs.restoreTabs()

    expect(state().openTabs).toEqual(['big.log'])
  })
})

describe('discarding unsaved work', () => {
  it('discardTabs touches only what was listed', () => {
    tabs.buffers.set('a.txt', buffer({ text: 'edit a', original: 'original' }))
    tabs.buffers.set('b.txt', buffer({ text: 'edit b', original: 'original' }))

    tabs.discardTabs(['a.txt'])

    expect(tabs.isDirty('a.txt')).toBe(false)
    expect(tabs.isDirty('b.txt')).toBe(true)
  })

  it('resetTabs clears the buffers but does not touch the tab list', () => {
    tabs.buffers.set('a.txt', buffer())
    state().openTabs = ['a.txt']

    tabs.resetTabs()

    expect(tabs.buffers.size).toBe(0)
    expect(state().openTabs).toEqual(['a.txt'])
  })
})

describe('confirmUnsaved', () => {
  it('with no dirty tabs it allows without asking', async () => {
    const asked = vi.fn(() => false)
    tabs.onUnsaved(asked)

    await expect(tabs.confirmUnsaved()).resolves.toBe(true)
    expect(asked).not.toHaveBeenCalled()
  })

  it('with no registered view it allows: locking up the app is worse', async () => {
    state().openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'an edit' }))

    await expect(tabs.confirmUnsaved()).resolves.toBe(true)
  })

  it('passes the list of dirty tabs to the view and returns its answer', async () => {
    state().openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'an edit' }))
    const asked = vi.fn(() => false)
    tabs.onUnsaved(asked)

    await expect(tabs.confirmUnsaved()).resolves.toBe(false)
    expect(asked).toHaveBeenCalledWith(['a.txt'])
  })

  it('asked about one tab, it touches only that one', async () => {
    state().openTabs = ['a.txt', 'b.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'edit a' }))
    tabs.buffers.set('b.txt', buffer({ text: 'edit b' }))
    const asked = vi.fn(() => true)
    tabs.onUnsaved(asked)

    await tabs.confirmUnsaved(['b.txt'])

    expect(asked).toHaveBeenCalledWith(['b.txt'])
  })
})
