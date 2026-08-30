import { beforeEach, describe, expect, it } from 'vitest'
import { loadStores } from '../support/stores.js'
import { entry, fileText, listing } from '../support/fixtures.js'

let ipc
let files

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  files = loaded.stores.files
  files.setRoot('/project')
})

describe('error texts', () => {
  it('reading a file: a known kind and the general fallback', () => {
    expect(files.fileErrorText({ kind: 'binary' })).toBe('Binary file — not shown.')
    expect(files.fileErrorText({ kind: 'something-new' })).toBe('Could not read this file.')
    expect(files.fileErrorText(null)).toBe('Could not read this file.')
  })

  /* The kind comes from `vcs/model.rs` rather than from `files/model.rs`, and
     it reaches this table through the diff — `vcs_file_at_head` is git, and git
     has a ceiling. Nothing but a test notices a kind that was never added: the
     fallback answers, and the sentence is about a file rather than about the
     ceiling that stopped it. */
  it('a git call stopped on its ceiling says so, rather than falling back', () => {
    expect(files.fileErrorText({ kind: 'timeout' })).toBe('Git took too long and was stopped.')
  })

  /* The two kinds `files_copy` and `files_move` make. Nothing on screen calls
     either command yet, which is exactly why the pair is pinned here: the menu
     that will call them is a later task, and a kind missing from the table
     falls back to a sentence about reading a file with nothing failing. */
  it('a copy refused for its own reasons does not fall back to a sentence about reading', () => {
    expect(files.fileErrorText({ kind: 'intoSelf' })).toBe('A folder cannot be put inside itself.')
    expect(files.fileErrorText({ kind: 'tooBig' })).toBe('That is too much to copy at once.')
  })

  it('a write speaks about writing, not about reading', () => {
    expect(files.saveErrorText({ kind: 'denied' })).toBe('No permission to write this file.')
  })

  it('a write has no stale key: its own branch with buttons handles that', () => {
    expect(files.saveErrorText({ kind: 'stale' })).toBe('Could not save this file.')
  })

  it('a directory speaks about a directory', () => {
    expect(files.dirErrorText({ kind: 'notFound' })).toBe('This folder is gone from disk.')
  })

  /* The two kinds `files/model.rs` grew for the making verbs. Nothing but a
     test notices a kind that never reached this table: the fallback answers,
     and a person typing a name that is already taken would be told the app
     could not create it — true, and not the reason. */
  it('a name already taken says so, rather than falling back', () => {
    expect(files.makeErrorText({ kind: 'alreadyExists' })).toBe(
      'Something with that name is already there.'
    )
  })

  it('a name that cannot be used says so, rather than falling back', () => {
    expect(files.makeErrorText({ kind: 'badName' })).toBe('That name cannot be used.')
  })

  it('every refusal to make something says nothing was made', () => {
    for (const kind of ['notFound', 'denied', 'notAFile', 'outside', 'io', 'something-new']) {
      expect(files.makeErrorText({ kind })).not.toMatch(/read|write this file/)
    }
    expect(files.makeErrorText({ kind: 'something-new' })).toBe('Could not create it.')
  })

  it('deleting speaks about deleting, in words of its own', () => {
    // `badName` reaches this table for two reasons and neither is a name
    // somebody typed: the project's own root, and a last segment Rust will not
    // take as a name.
    expect(files.trashErrorText({ kind: 'badName' })).toBe('That name cannot be deleted.')
    expect(files.trashErrorText({ kind: 'badName' })).not.toBe(
      files.makeErrorText({ kind: 'badName' })
    )
    expect(files.trashErrorText({ kind: 'denied' })).toBe('No permission to delete this.')
    expect(files.trashErrorText(null)).toBe('Could not move it to the trash.')
  })
})

describe('small things', () => {
  it('basenameOf takes the last segment', () => {
    expect(files.basenameOf('src/stores/tabs.js')).toBe('tabs.js')
    expect(files.basenameOf('a.txt')).toBe('a.txt')
    expect(files.basenameOf('src/')).toBe('src')
  })

  it('isStubPath recognises a stub by its zero byte', () => {
    expect(files.isStubPath('src\u0000more')).toBe(true)
    expect(files.isStubPath('src/more')).toBe(false)
    expect(files.isStubPath(null)).toBe(false)
  })
})

describe('listDir', () => {
  it('puts a listing into the map under its own directory name', async () => {
    ipc.on('files_list', () => listing({ dir: '', entries: [entry()] }))

    await files.listDir('')

    expect(files.filesState.dirs.get('').entries).toHaveLength(1)
    expect(ipc.calls('files_list')).toEqual([{ root: '/project', dir: '' }])
  })

  it('a second read of the same directory does not start while the first is running', async () => {
    ipc.on('files_list', () => listing({ dir: 'src' }))

    await Promise.all([files.listDir('src'), files.listDir('src')])

    expect(ipc.calls('files_list')).toHaveLength(1)
  })

  it('an answer arriving after the root changed does not reach the new tree', async () => {
    ipc.on('files_list', () => listing({ dir: 'src', entries: [entry()] }))

    const pending = files.listDir('src')
    files.setRoot('/another')
    await pending

    expect(files.filesState.dirs.size).toBe(0)
  })

  it('a refusal leaves the tree as it was and puts a short phrase outwards', async () => {
    ipc.fail('files_list', { kind: 'denied', message: 'no permission' })

    await files.listDir('secret')

    expect(files.filesState.dirs.size).toBe(0)
    expect(files.filesState.lastError).toBe('No permission to read this folder.')
  })

  it('with no root it does not go to the disk at all', async () => {
    files.setRoot(null)
    await files.listDir('')

    expect(ipc.calls('files_list')).toHaveLength(0)
  })
})

describe('setRoot', () => {
  it('resets the tree, the reading flag and the error', async () => {
    ipc.on('files_list', () => listing({ dir: '', entries: [entry()] }))
    await files.listDir('')
    files.filesState.lastError = 'something happened'

    files.setRoot('/another')

    expect(files.filesState.root).toBe('/another')
    expect(files.filesState.dirs.size).toBe(0)
    expect(files.filesState.lastError).toBe(null)
  })
})

describe('refreshDirs', () => {
  it('re-reads only the directories that are already in the map', async () => {
    ipc.on('files_list', (args) => listing({ dir: args.dir }))
    await files.listDir('')

    await files.refreshDirs(['', 'nobody-expanded-this'])

    expect(ipc.calls('files_list').map((call) => call.dir)).toEqual(['', ''])
  })
})

describe('readFile and writeFile', () => {
  it('a read returns what the back end returned', async () => {
    ipc.on('files_read', fileText())

    await expect(files.readFile('a.txt')).resolves.toEqual(fileText())
  })

  it('a Tauri error passes through as is', async () => {
    ipc.fail('files_read', { kind: 'binary', message: 'binary' })

    await expect(files.readFile('a.png')).rejects.toEqual({ kind: 'binary', message: 'binary' })
  })

  it('a delivery error is reduced to the io kind', async () => {
    ipc.fail('files_read', new Error('the IPC did not come up'))

    await expect(files.readFile('a.txt')).rejects.toEqual({
      kind: 'io',
      message: 'the IPC did not come up'
    })
  })

  it('a write carries the expected timestamp and returns the new one', async () => {
    ipc.on('files_write', 11)

    await expect(files.writeFile('a.txt', 'new text', 10)).resolves.toBe(11)
    expect(ipc.calls('files_write')).toEqual([
      { root: '/project', path: 'a.txt', text: 'new text', expectedMtime: 10 }
    ])
  })
})

describe('createFile, createDir and trashPath', () => {
  it('sends the folder and the name apart, which is what the check in Rust is made of', async () => {
    ipc.on('files_create', 'src/main.rs')

    await expect(files.createFile('src', 'main.rs')).resolves.toBe('src/main.rs')
    expect(ipc.calls('files_create')).toEqual([{ root: '/project', dir: 'src', name: 'main.rs' }])
  })

  it('makes a folder through its own command and answers with the new path', async () => {
    ipc.on('files_mkdir', 'docs')

    await expect(files.createDir('', 'docs')).resolves.toBe('docs')
    expect(ipc.calls('files_mkdir')).toEqual([{ root: '/project', dir: '', name: 'docs' }])
  })

  it('a refusal to make something is thrown rather than parked in lastError', async () => {
    // A refused directory *read* is the tree's own state and shows as a strip;
    // this one is an answer to something somebody asked for a moment ago, and
    // it is owed a toast — which is the caller's to raise.
    ipc.fail('files_create', { kind: 'alreadyExists', message: 'src/main.rs' })

    await expect(files.createFile('src', 'main.rs')).rejects.toEqual({
      kind: 'alreadyExists',
      message: 'src/main.rs'
    })
    expect(files.filesState.lastError).toBe(null)
  })

  it('a delivery error while making something is reduced to the io kind', async () => {
    // This is what `npm run dev` answers: mockBackend refuses every write
    // loudly, and the refusal arrives as an Error rather than as a kind.
    ipc.fail('files_mkdir', new Error('mockBackend: "files_mkdir" is not implemented'))

    await expect(files.createDir('', 'docs')).rejects.toMatchObject({ kind: 'io' })
  })

  it('deletes by whole path, since the thing being deleted exists', async () => {
    ipc.on('files_trash', null)

    await expect(files.trashPath('src/main.rs')).resolves.toBeUndefined()
    expect(ipc.calls('files_trash')).toEqual([{ root: '/project', path: 'src/main.rs' }])
  })

  it('a refusal to delete is thrown, so nothing is closed on the strength of it', async () => {
    ipc.fail('files_trash', { kind: 'denied', message: 'src/main.rs' })

    await expect(files.trashPath('src/main.rs')).rejects.toEqual({
      kind: 'denied',
      message: 'src/main.rs'
    })
  })
})

describe('statFiles', () => {
  it('with no root it does not go to the disk, even when there are paths', async () => {
    files.setRoot(null)

    await expect(files.statFiles(['a.txt'])).resolves.toEqual([])
    expect(ipc.calls('files_stat')).toHaveLength(0)
  })

  it('with no paths it does not go to the disk, even when there is a root', async () => {
    await expect(files.statFiles([])).resolves.toEqual([])
    expect(ipc.calls('files_stat')).toHaveLength(0)
  })

  it('a refusal gives an empty list: the focus sweep is a convenience, no reason to drop the interface', async () => {
    ipc.fail('files_stat', { kind: 'io', message: 'the disk fell off' })

    await expect(files.statFiles(['a.txt'])).resolves.toEqual([])
  })
})

describe('treeNodes', () => {
  const fill = () => {
    files.filesState.dirs.set(
      '',
      listing({
        dir: '',
        entries: [
          entry({ name: 'src', path: 'src', kind: 'dir' }),
          entry({ name: 'a.txt', path: 'a.txt' })
        ]
      })
    )
    files.filesState.dirs.set(
      'src',
      listing({ dir: 'src', entries: [entry({ name: 'tabs.js', path: 'src/tabs.js' })] })
    )
  }

  it('an unexpanded directory returns no children', () => {
    fill()

    const nodes = files.treeNodes(new Set())

    expect(nodes[0].name).toBe('src')
    expect(nodes[0].children).toBe(undefined)
  })

  it('an expanded directory returns the children that were read', () => {
    fill()

    const nodes = files.treeNodes(new Set(['src']))

    expect(nodes[0].children).toHaveLength(1)
    expect(nodes[0].children[0].path).toBe('src/tabs.js')
  })

  it('a directory that is expanded but not yet read does not invent children', () => {
    files.filesState.dirs.set(
      '',
      listing({ entries: [entry({ name: 'src', path: 'src', kind: 'dir' })] })
    )

    const nodes = files.treeNodes(new Set(['src']))

    expect(nodes[0].children).toBe(undefined)
  })

  it('a truncated directory gets a stub row: silent truncation would lie', () => {
    files.filesState.dirs.set('', listing({ entries: [entry()], truncated: 7 }))

    const nodes = files.treeNodes(new Set())

    expect(nodes).toHaveLength(2)
    expect(nodes[1].name).toBe('…7 more')
    expect(files.isStubPath(nodes[1].path)).toBe(true)
  })

  it('an unread root gives an empty tree rather than a throw', () => {
    expect(files.treeNodes(new Set())).toEqual([])
  })

  /* The one value the tree's `git` prop is ever given in the product. The five
     other kinds it understands — modified, added, deleted, untracked, conflict —
     have no source, so `undefined` is the right answer for everything else and
     not a hole waiting to be filled. */
  it('an entry git ignores is marked, and everything else is left undefined', () => {
    files.filesState.dirs.set(
      '',
      listing({
        entries: [
          entry({ name: 'node_modules', path: 'node_modules', kind: 'dir', ignored: true }),
          entry({ name: 'src', path: 'src', kind: 'dir' }),
          entry({ name: 'package.json', path: 'package.json' })
        ]
      })
    )

    const nodes = files.treeNodes(new Set())

    expect(nodes.map((n) => n.git)).toEqual(['ignored', undefined, undefined])
  })

  /* Every listing answers for itself: git reports a name inside an ignored
     folder as ignored on its own, so nothing has to be carried down the tree. */
  it('the children of an ignored folder carry the mark themselves', () => {
    files.filesState.dirs.set(
      '',
      listing({
        entries: [entry({ name: 'node_modules', path: 'node_modules', kind: 'dir', ignored: true })]
      })
    )
    files.filesState.dirs.set(
      'node_modules',
      listing({
        dir: 'node_modules',
        entries: [entry({ name: '.bin', path: 'node_modules/.bin', kind: 'dir', ignored: true })]
      })
    )

    const nodes = files.treeNodes(new Set(['node_modules']))

    expect(nodes[0].children[0].git).toBe('ignored')
  })

  /* A project outside git, and the truncation stub beside it: neither has an
     `ignored` field at all, and both draw at full strength rather than throwing. */
  it('an entry with no ignored field at all is drawn at full strength', () => {
    files.filesState.dirs.set(
      '',
      listing({ entries: [{ name: 'src', path: 'src', kind: 'dir' }], truncated: 3 })
    )

    const nodes = files.treeNodes(new Set())

    expect(nodes[0].git).toBe(undefined)
    expect(nodes[1].git).toBe(undefined)
  })
})
