import { describe, expect, it } from 'vitest'
import { iconNodes } from '../../../src/components/core/icons.js'
import {
  FILE_MENU_W,
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

describe('FILE_MENU_W', () => {
  it('is one number, and a number rather than a length', () => {
    // `PointerMenu` hands it to `ContextMenu`'s `width`, a Number prop that goes
    // into the placement arithmetic: a string of px would clip every long row
    // silently.
    expect(typeof FILE_MENU_W).toBe('number')
    expect(FILE_MENU_W).toBe(320)
  })

  it('stays wide enough for the longest label on the menu', () => {
    /* A test cannot measure a font, so what this holds is the arithmetic the
       ceiling was chosen by: 70px of chrome — the glyph, the gaps and the
       padding, the figure `sessionMenu.test.js` uses for the same panel — and
       5.7px a character, which is what this menu's one recorded measurement
       comes to (292px for the 39 characters of "Attach to agent — no agent to
       type into"). What it catches is the way this number goes wrong: a refusal
       reworded longer than the panel it has to be read in, with no tooltip
       behind it to recover the rest from. */
    const longest = [
      ...fileMenuItems({ target: 'file' }),
      /* With a live agent that is not the selected one, which is the only way
         to reach the second of Attach to agent's two sentences: it is one of
         the four labels in the file that carry a reason, and a set built
         without it would leave that one unmeasured. */
      ...fileMenuItems({ target: 'file', hasLiveAgent: true }),
      ...fileMenuItems({ target: 'dir', pasteReason: 'intoSelf' }),
      ...fileMenuItems({ target: 'file', confirmingDelete: true }),
      ...fileMenuItems({ target: 'root' })
    ]
      .filter((item) => item.label)
      .map((item) => item.label)
      .reduce((a, b) => (b.length > a.length ? b : a))
    expect(70 + longest.length * 5.7).toBeLessThan(FILE_MENU_W)
  })
})

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
  it('offers thirteen rows in six groups', () => {
    const items = fileMenuItems({ target: 'file', canAttach: true, userAgent: MAC })
    expect(kinds(items)).toEqual([
      'open-terminal',
      'reveal',
      'new-file',
      'new-folder',
      'cut',
      'copy',
      'paste',
      'duplicate',
      'rename',
      'copy-path',
      'copy-relative-path',
      'attach',
      'delete'
    ])
    expect(separators(items)).toBe(5)
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

  it('leaves nothing on the menu greyed once every row has something to act on', () => {
    // Paste is the one row that is greyed rather than absent where it cannot
    // act, so it is given something to paste here: what is being checked is
    // that no *other* row is off in the ordinary state.
    const items = fileMenuItems({ target: 'file', canAttach: true, canPaste: true })
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

  it('leaves the row-only verbs out of the root menu rather than greying them', () => {
    const items = fileMenuItems({ target: 'root', canAttach: true })
    expect(kinds(items)).toEqual([
      'open-terminal',
      'reveal',
      'new-file',
      'new-folder',
      'paste',
      'copy-path',
      'copy-relative-path'
    ])
    expect(separators(items)).toBe(3)
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

describe('the clipboard group', () => {
  it('offers all five verbs on a file and the same five on a folder', () => {
    const file = kinds(fileMenuItems({ target: 'file', canPaste: true }))
    const dir = kinds(fileMenuItems({ target: 'dir', canPaste: true }))
    for (const present of [file, dir]) {
      expect(present).toEqual(
        expect.arrayContaining(['cut', 'copy', 'paste', 'duplicate', 'rename'])
      )
    }
  })

  it('offers only paste on the root, and leaves the other four out entirely', () => {
    // The same choice Attach to agent and Delete already made here: absent
    // rather than greyed, because a greyed row says "not now" and these say
    // "never" — cut, copy, duplicate and rename mean nothing about a project's
    // own root.
    const present = kinds(fileMenuItems({ target: 'root', canPaste: true }))
    expect(present).toContain('paste')
    expect(present).not.toContain('cut')
    expect(present).not.toContain('copy')
    expect(present).not.toContain('duplicate')
    expect(present).not.toContain('rename')
  })

  it('sits between the making rows and the two that copy a path', () => {
    const present = kinds(fileMenuItems({ target: 'file', canPaste: true }))
    expect(present.indexOf('cut')).toBe(present.indexOf('new-folder') + 1)
    expect(present.indexOf('copy-path')).toBe(present.indexOf('rename') + 1)
  })

  it('says in the label why paste is off when nothing was copied', () => {
    // A row here has no tooltip and no title: a reason kept anywhere but the
    // label is a reason nobody reads.
    const paste = find(fileMenuItems({ target: 'file', canPaste: false, pasteReason: 'empty' }), 'paste')
    expect(paste).toMatchObject({ label: 'Paste — nothing copied yet', disabled: true })
  })

  it('says in the label why paste is off inside the copied folder', () => {
    const paste = find(
      fileMenuItems({ target: 'dir', canPaste: false, pasteReason: 'intoSelf' }),
      'paste'
    )
    expect(paste).toMatchObject({ label: 'Paste — cannot paste a folder into itself', disabled: true })
  })

  it('greys paste on the root with its reason too, since that is the one row there', () => {
    const paste = find(fileMenuItems({ target: 'root' }), 'paste')
    expect(paste).toMatchObject({ label: 'Paste — nothing copied yet', disabled: true })
  })

  it('says the bare verb when there is something to paste', () => {
    const paste = find(fileMenuItems({ target: 'file', canPaste: true }), 'paste')
    expect(paste).toMatchObject({ label: 'Paste', disabled: false })
  })

  it('greys nothing but paste, whatever the clipboard holds', () => {
    for (const canPaste of [true, false]) {
      const off = fileMenuItems({ target: 'file', canAttach: true, canPaste })
        .filter((item) => item.disabled)
        .map((item) => item.kind)
      expect(off).toEqual(canPaste ? [] : ['paste'])
    }
  })

  it('gives duplicate a glyph of its own rather than the one copy already wears twice', () => {
    // Copy and Copy path are three rows apart under `copy`; a third row wearing
    // it would leave the group told apart by its labels alone.
    const items = fileMenuItems({ target: 'file' })
    expect(find(items, 'duplicate').icon).toBe('copy-plus')
    expect(find(items, 'copy').icon).toBe('copy')
    expect(find(items, 'copy-path').icon).toBe('copy')
  })

  it('closes the panel on every one of the five: none of them asks a second time', () => {
    const group = ['cut', 'copy', 'paste', 'duplicate', 'rename']
    const items = fileMenuItems({ target: 'file', canPaste: true })
    for (const kind of group) expect(find(items, kind).keepOpen).toBeUndefined()
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

describe('relativePath', () => {
  it('is what the tree already holds', () => {
    expect(relativePath('src/main.rs')).toBe('src/main.rs')
  })

  it('is a dot at the root, never the empty string', () => {
    // An empty clipboard cannot be told apart from a copy that failed.
    expect(relativePath('')).toBe('.')
  })
})
