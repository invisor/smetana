import { describe, expect, it } from 'vitest'
import { iconNodes } from '../../../src/components/core/icons.js'
import {
  absolutePath,
  fileManagerName,
  fileMenuItems,
  folderOf,
  parentOf,
  relativePath
} from '../../../src/components/files/fileMenu.js'

const kinds = (items) => items.filter((i) => i.type !== 'separator').map((i) => i.kind)
const find = (items, kind) => items.find((i) => i.kind === kind)
const separators = (items) => items.filter((i) => i.type === 'separator').length

/* The user agents of the three webviews Tauri runs in, verbatim enough for the
   one question asked of them. */
const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15'
const WINDOWS = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Edg/120.0'
const LINUX = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'

describe('fileManagerName', () => {
  it('is Finder on macOS', () => {
    expect(fileManagerName(MAC)).toBe('Finder')
  })

  it('is Explorer on Windows', () => {
    expect(fileManagerName(WINDOWS)).toBe('Explorer')
  })

  it('is the plain noun everywhere else', () => {
    expect(fileManagerName(LINUX)).toBe('file manager')
  })

  it('names the plain noun rather than guessing when there is nothing to read', () => {
    expect(fileManagerName()).toBe('file manager')
    expect(fileManagerName('')).toBe('file manager')
  })
})

describe('fileMenuItems', () => {
  it('offers eight rows in five groups', () => {
    const items = fileMenuItems({ target: 'file', canAttach: true, userAgent: MAC })
    expect(kinds(items)).toEqual([
      'open-terminal',
      'reveal',
      'new-file',
      'new-folder',
      'copy-path',
      'copy-relative-path',
      'attach',
      'delete'
    ])
    expect(separators(items)).toBe(4)
  })

  it('draws the reveal row with the platform in it', () => {
    expect(find(fileMenuItems({ userAgent: WINDOWS }), 'reveal').label).toBe('Reveal in Explorer')
    expect(find(fileMenuItems({ userAgent: MAC }), 'reveal').label).toBe('Reveal in Finder')
  })

  it('offers a folder exactly what it offers a file', () => {
    const file = fileMenuItems({ target: 'file', canAttach: true, hasLiveAgent: true })
    const dir = fileMenuItems({ target: 'dir', canAttach: true, hasLiveAgent: true })
    expect(dir).toEqual(file)
  })

  it('leaves nothing on the menu greyed, now that the three writing rows write', () => {
    const items = fileMenuItems({ target: 'file', canAttach: true })
    const off = items.filter((item) => item.disabled).map((item) => item.kind)
    expect(off).toEqual([])
  })

  it('draws deletion last and in the danger tone', () => {
    const items = fileMenuItems({ target: 'file' })
    expect(items.at(-1)).toMatchObject({ kind: 'delete', tone: 'danger' })
  })

  it('asks a second time in the row itself rather than closing on the first pick', () => {
    // The panel stays up on the first pick and that is what `keepOpen` says.
    // Without it PointerMenu closes before it emits, and the question would be
    // asked by a panel that is already gone.
    const armed = find(fileMenuItems({ target: 'file', confirmingDelete: true }), 'delete')
    const idle = find(fileMenuItems({ target: 'file' }), 'delete')

    expect(idle).toMatchObject({ label: 'Delete', keepOpen: true })
    expect(armed).toMatchObject({ label: 'Click again to confirm', keepOpen: false })
    expect(armed.tone).toBe('danger')
  })

  it('arms nothing else on the menu, whatever Delete is doing', () => {
    const idle = fileMenuItems({ target: 'file', canAttach: true })
    const armed = fileMenuItems({ target: 'file', canAttach: true, confirmingDelete: true })
    const others = (items) => items.filter((item) => item.kind !== 'delete')
    expect(others(armed)).toEqual(others(idle))
    expect(others(armed).some((item) => item.keepOpen)).toBe(false)
  })

  it('leaves the root menu closing on every pick, since it has no Delete at all', () => {
    // The two menus that existed before this one — a project row and a branch
    // row — set the flag nowhere either, and this is the one list here that can
    // be checked for it in the same way.
    const items = fileMenuItems({ target: 'root', confirmingDelete: true })
    expect(items.some((item) => item.keepOpen)).toBe(false)
  })

  it('gives the two making verbs a glyph now that they do something', () => {
    const items = fileMenuItems({ target: 'root' })
    expect(find(items, 'new-file')).toEqual({ kind: 'new-file', label: 'New file', icon: 'file-plus' })
    expect(find(items, 'new-folder')).toEqual({
      kind: 'new-folder',
      label: 'New folder',
      icon: 'folder-plus'
    })
  })

  it('says the bare verb when the selected agent can take the path', () => {
    const items = fileMenuItems({ target: 'file', canAttach: true, hasLiveAgent: true })
    expect(find(items, 'attach')).toMatchObject({ label: 'Attach to agent', disabled: false })
  })

  it('greys Attach to agent with the reason in the row when there is no agent', () => {
    // A row here has no tooltip and no title, so a reason kept anywhere but the
    // label is a reason nobody can read.
    const items = fileMenuItems({ target: 'file', canAttach: false, hasLiveAgent: false })
    expect(find(items, 'attach')).toMatchObject({
      label: 'Attach to agent — no agent to type into',
      disabled: true
    })
  })

  it('says the way out instead when there is an agent but it is not the selected one', () => {
    // The verb types into the agent the centre is showing, so the row can be
    // refused with one running a column over — and "no agent to type into"
    // would be plainly false while a person is looking at one.
    const items = fileMenuItems({ target: 'file', canAttach: false, hasLiveAgent: true })
    expect(find(items, 'attach')).toMatchObject({
      label: 'Attach to agent — select an agent first',
      disabled: true
    })
  })

  it('lets the live agent decide the words and never the state', () => {
    const off = [
      find(fileMenuItems({ target: 'file', canAttach: false, hasLiveAgent: false }), 'attach'),
      find(fileMenuItems({ target: 'file', canAttach: false, hasLiveAgent: true }), 'attach')
    ]
    expect(off.map((item) => item.disabled)).toEqual([true, true])
    expect(new Set(off.map((item) => item.label)).size).toBe(2)

    const on = [
      find(fileMenuItems({ target: 'file', canAttach: true, hasLiveAgent: false }), 'attach'),
      find(fileMenuItems({ target: 'file', canAttach: true, hasLiveAgent: true }), 'attach')
    ]
    expect(on.map((item) => item.label)).toEqual(['Attach to agent', 'Attach to agent'])
    expect(on.map((item) => item.disabled)).toEqual([false, false])
  })

  it('leaves the two row-only verbs out of the root menu rather than greying them', () => {
    const items = fileMenuItems({ target: 'root', canAttach: true })
    expect(kinds(items)).toEqual([
      'open-terminal',
      'reveal',
      'new-file',
      'new-folder',
      'copy-path',
      'copy-relative-path'
    ])
    expect(separators(items)).toBe(2)
  })

  it('still offers the root both copies', () => {
    const items = fileMenuItems({ target: 'root' })
    expect(find(items, 'copy-path').disabled).toBeFalsy()
    expect(find(items, 'copy-relative-path').disabled).toBeFalsy()
  })

  it('names only glyphs the registry actually holds', () => {
    // Against `iconNodes` itself rather than against a list written out here:
    // a copy of the names would restate this module instead of checking the one
    // thing that can go wrong, which is `Icon` warning in dev and drawing
    // nothing at all for a name nobody registered.
    const named = [
      ...fileMenuItems({ target: 'file', canAttach: true }),
      ...fileMenuItems({ target: 'root' })
    ]
      .filter((item) => item.icon)
      .map((item) => item.icon)
    expect(named.length).toBeGreaterThan(0)
    for (const name of named) expect(iconNodes).toHaveProperty(name)
  })
})

describe('folderOf', () => {
  it('answers a folder with itself', () => {
    expect(folderOf({ path: 'src/components', target: 'dir' })).toBe('src/components')
  })

  it('answers a file with the folder holding it', () => {
    expect(folderOf({ path: 'src/components/Icon.vue', target: 'file' })).toBe('src/components')
  })

  it('answers a file at the top level with the project root', () => {
    expect(folderOf({ path: 'Cargo.toml', target: 'file' })).toBe('')
  })

  it('answers the root menu with the root', () => {
    expect(folderOf({ path: '', target: 'root' })).toBe('')
  })
})

describe('parentOf', () => {
  it('is where a path lives, whatever the path is', () => {
    // The difference from folderOf, and the reason both exist: a folder that has
    // just been deleted answers that one with itself, and the folder to re-read
    // is the one it was in.
    expect(parentOf('src/components/Icon.vue')).toBe('src/components')
    expect(parentOf('src/components')).toBe('src')
    expect(parentOf('Cargo.toml')).toBe('')
    expect(parentOf('')).toBe('')
  })
})

describe('absolutePath', () => {
  it('joins the project root and the tree path', () => {
    expect(absolutePath('/Users/you/dev/app', 'src/main.rs')).toBe('/Users/you/dev/app/src/main.rs')
  })

  it('is the root itself for the root', () => {
    expect(absolutePath('/Users/you/dev/app', '')).toBe('/Users/you/dev/app')
  })

  it('does not double a separator the root already ends in', () => {
    expect(absolutePath('/Users/you/dev/app/', 'src')).toBe('/Users/you/dev/app/src')
  })

  it('writes a Windows path in one separator rather than two', () => {
    // Everything relative in stores/files.js is written with "/" whatever the
    // platform, and the root arrives from Rust in the platform's own form.
    expect(absolutePath('C:\\Users\\you\\app', 'src/main.rs')).toBe('C:\\Users\\you\\app\\src\\main.rs')
  })

  it('keeps a forward slash for a root that has one, whatever else it holds', () => {
    expect(absolutePath('/Users/you/a\\b', 'src')).toBe('/Users/you/a\\b/src')
  })

  it('is the path alone when there is no project to hang it off', () => {
    expect(absolutePath(null, 'src/main.rs')).toBe('src/main.rs')
  })
})

describe('relativePath', () => {
  it('is what the tree already holds', () => {
    expect(relativePath('src/main.rs')).toBe('src/main.rs')
  })

  it('is a dot at the root, never the empty string', () => {
    // An empty clipboard cannot be told apart from a copy that failed.
    expect(relativePath('')).toBe('.')
  })
})
