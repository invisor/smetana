import { describe, expect, it } from 'vitest'
import {
  absolutePath,
  fileManagerName,
  fileMenuItems,
  relativePath,
  shellFolder
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
    const items = fileMenuItems({ target: 'file', hasAgentSession: true, userAgent: MAC })
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
    const file = fileMenuItems({ target: 'file', hasAgentSession: true })
    const dir = fileMenuItems({ target: 'dir', hasAgentSession: true })
    expect(dir).toEqual(file)
  })

  it('greys the three rows the second half of this work makes live', () => {
    const items = fileMenuItems({ target: 'file', hasAgentSession: true })
    for (const kind of ['new-file', 'new-folder', 'delete']) {
      expect(find(items, kind).disabled).toBe(true)
    }
  })

  it('draws deletion last and in the danger tone', () => {
    const items = fileMenuItems({ target: 'file' })
    expect(items.at(-1)).toMatchObject({ kind: 'delete', tone: 'danger' })
  })

  it('greys Attach to agent with the reason in the row', () => {
    // A row here has no tooltip and no title, so a reason kept anywhere but the
    // label is a reason nobody can read.
    const items = fileMenuItems({ target: 'file', hasAgentSession: false })
    expect(find(items, 'attach')).toMatchObject({
      label: 'Attach to agent — no agent to type into',
      disabled: true
    })
  })

  it('says the bare verb when there is an agent', () => {
    const items = fileMenuItems({ target: 'file', hasAgentSession: true })
    expect(find(items, 'attach')).toMatchObject({ label: 'Attach to agent', disabled: false })
  })

  it('leaves the two row-only verbs out of the root menu rather than greying them', () => {
    const items = fileMenuItems({ target: 'root', hasAgentSession: true })
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

  it('draws every live row with a glyph registered in core/icons.js', () => {
    const items = fileMenuItems({ target: 'file', hasAgentSession: true })
    const icons = items.filter((i) => i.icon).map((i) => i.icon)
    expect(icons).toEqual(['terminal', 'folder-open', 'copy', 'copy', 'paperclip', 'trash-2'])
  })
})

describe('shellFolder', () => {
  it('opens a folder in itself', () => {
    expect(shellFolder({ path: 'src/components', target: 'dir' })).toBe('src/components')
  })

  it('opens a file in the folder holding it', () => {
    expect(shellFolder({ path: 'src/components/Icon.vue', target: 'file' })).toBe('src/components')
  })

  it('opens a file at the top level in the project root', () => {
    expect(shellFolder({ path: 'Cargo.toml', target: 'file' })).toBe('')
  })

  it('opens the root menu in the root', () => {
    expect(shellFolder({ path: '', target: 'root' })).toBe('')
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
