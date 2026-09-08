import { describe, expect, it } from 'vitest'
import { iconNodes } from '../../../src/components/core/icons.js'
import {
  CHANGE_MENU_W,
  changeMenuItems,
  isFolderRecord
} from '../../../src/components/git/changeMenu.js'

const verbs = (items) => items.filter((item) => !item.type)
const kinds = (items) => verbs(items).map((item) => item.kind)
const find = (items, kind) => items.find((item) => item.kind === kind)
const off = (items) => verbs(items).filter((item) => item.disabled).map((item) => item.kind)
const separators = (items) => items.filter((item) => item.type === 'separator').length

/* The user agents of the three webviews Tauri runs in, verbatim enough for the
   one question asked of them — `fileMenu.test.js`'s own three. */
const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15'
const WINDOWS = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Edg/120.0'
const LINUX = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'

/* An ordinary row: a tracked file that has been edited, inside the project. */
const modified = (extra = {}) => changeMenuItems({ path: 'src/main.js', kind: 'modified', ...extra })

describe('changeMenuItems', () => {
  it('offers five rows in three groups, the diff the click opens first', () => {
    const items = modified({ userAgent: MAC })
    expect(kinds(items)).toEqual([
      'open-changes',
      'open-file',
      'reveal',
      'copy-path',
      'copy-relative-path'
    ])
    expect(separators(items)).toBe(2)
  })

  it('keeps the two that open something apart from the one that leaves the window', () => {
    const items = modified()
    const at = items.findIndex((item) => item.type === 'separator')
    expect(items.slice(0, at).map((item) => item.kind)).toEqual(['open-changes', 'open-file'])
    expect(items[at + 1].kind).toBe('reveal')
  })

  /* Every row carries a glyph, because a menu row without one leaves a hole in
     the gutter every other row fills. */
  it('names a glyph for every row, and one this app has registered', () => {
    const icons = verbs(modified()).map((item) => item.icon)
    expect(icons).toEqual(['git-compare', 'file', 'folder-open', 'copy', 'copy'])
    for (const name of icons) expect(iconNodes[name]).toBeTruthy()
  })

  it('leaves nothing greyed on a tracked file inside the project', () => {
    expect(off(modified({ userAgent: MAC }))).toEqual([])
  })

  it('draws the reveal row with the platform in it', () => {
    expect(find(modified({ userAgent: MAC }), 'reveal').label).toBe('Reveal in Finder')
    expect(find(modified({ userAgent: WINDOWS }), 'reveal').label).toBe('Reveal in Explorer')
    expect(find(modified({ userAgent: LINUX }), 'reveal').label).toBe('Reveal in file manager')
    expect(find(modified(), 'reveal').label).toBe('Reveal in file manager')
  })

  it('says nothing about a run or an operation in flight, because nothing here writes', () => {
    // The fourth reach of refusal `branchMenu.js` describes, and this whole
    // menu has it: there is no caption row at all, ever.
    expect(modified().some((item) => item.type === 'label')).toBe(false)
  })
})

describe('the three refusals', () => {
  it('greys the two that need a file when the row is an untracked folder', () => {
    const items = changeMenuItems({ path: 'vendor/', kind: 'untracked', userAgent: MAC })
    expect(off(items)).toEqual(['open-changes', 'open-file'])
    expect(find(items, 'open-changes').label).toBe('Open changes — no file behind a folder')
    expect(find(items, 'open-file').label).toBe('Open file — no file behind a folder')
    // A folder is still a place on the disk and still a string.
    expect(find(items, 'reveal').label).toBe('Reveal in Finder')
  })

  it('greys the file and the file manager on a deleted file, and keeps the diff', () => {
    const items = changeMenuItems({ path: 'src/old.js', kind: 'deleted', userAgent: MAC })
    expect(off(items)).toEqual(['open-file', 'reveal'])
    expect(find(items, 'open-file').label).toBe(
      'Open file — the file is gone from the working tree'
    )
    expect(find(items, 'reveal').label).toBe(
      'Reveal in Finder — the file is gone from the working tree'
    )
    // The diff is what shows the deletion, so it is the one row that still
    // means something on this row.
    expect(find(items, 'open-changes')).toMatchObject({ label: 'Open changes', disabled: false })
  })

  it('greys the editor and the relative path for a file outside the project', () => {
    const items = modified({ insideProject: false, userAgent: MAC })
    expect(off(items)).toEqual(['open-file', 'copy-relative-path'])
    expect(find(items, 'open-file').label).toBe('Open file — outside the project')
    expect(find(items, 'copy-relative-path').label).toBe('Copy relative path — outside the project')
    // The absolute path is the one row nothing can ever refuse.
    expect(find(items, 'copy-path')).toMatchObject({ label: 'Copy path', disabled: false })
  })

  it('says the most specific reason when a row is refused twice over', () => {
    // An untracked folder in a repository that sits outside the project: the
    // fact about the row itself is the one worth saying.
    const folder = changeMenuItems({ path: 'vendor/', kind: 'untracked', insideProject: false })
    expect(find(folder, 'open-file').label).toBe('Open file — no file behind a folder')

    const gone = changeMenuItems({ path: 'src/old.js', kind: 'deleted', insideProject: false })
    expect(find(gone, 'open-file').label).toBe(
      'Open file — the file is gone from the working tree'
    )
  })

  it('leaves every refusal in the label itself, never in a caption', () => {
    // A row here has no tooltip and no `title`, and the rows are refused for
    // different reasons — the one case a caption above a group cannot serve.
    for (const items of [
      changeMenuItems({ path: 'vendor/', kind: 'untracked' }),
      changeMenuItems({ path: 'src/old.js', kind: 'deleted' }),
      modified({ insideProject: false })
    ]) {
      expect(items.some((item) => item.type === 'label')).toBe(false)
      for (const item of verbs(items)) {
        expect(item.disabled).toBe(item.label.includes(' — '))
      }
    }
  })

  it('closes on every pick, since no row here asks a second time', () => {
    // `keepOpen` is the file tree Delete alone, and this menu has nothing that
    // loses work. Discard, when it arrives, is a separate task.
    expect(modified().some((item) => item.keepOpen)).toBe(false)
    expect(modified().some((item) => item.tone)).toBe(false)
  })
})

describe('isFolderRecord', () => {
  /* The one spelling of "there is no file behind this row", read by the greying
     here, by the click in `ChangeList.vue`, by Enter through `changeKeys.js`,
     by the glyph and the drawn name in that same component, and by the
     absolute-path join in `DesktopApp.vue` — six readers, which is the reason
     it is exported rather than written out at each. The module's own header
     carries the list, and so does `.claude/rules/vcs-panel.md`. */
  it('is true only of the trailing-slash record git reports a directory as', () => {
    expect(isFolderRecord('src/components/git/')).toBe(true)
    expect(isFolderRecord('vendor/')).toBe(true)
    expect(isFolderRecord('src/main.js')).toBe(false)
    expect(isFolderRecord('notes.txt')).toBe(false)
  })

  it('answers false for nothing at all rather than throwing', () => {
    expect(isFolderRecord()).toBe(false)
    expect(isFolderRecord('')).toBe(false)
  })

  it('is the very test the greying is built on', () => {
    // The menu and the click cannot disagree about what a folder is, which is
    // the whole reason this is one function.
    const greyed = (path) =>
      changeMenuItems({ path, kind: 'untracked' }).some(
        (item) => item.kind === 'open-changes' && item.disabled
      )
    for (const path of ['vendor/', 'src/main.js', 'notes.txt', 'a/b/']) {
      expect(greyed(path)).toBe(isFolderRecord(path))
    }
  })
})

describe('CHANGE_MENU_W', () => {
  it('is one number, and a number rather than a length', () => {
    // `PointerMenu` hands it to `ContextMenu`'s `width`, a Number prop that goes
    // into the placement arithmetic: a string of px would clip every long row
    // silently.
    expect(typeof CHANGE_MENU_W).toBe('number')
    expect(CHANGE_MENU_W).toBe(440)
  })

  it('stays wide enough for the longest label on the menu', () => {
    /* A test cannot measure a font, so what this holds is the arithmetic the
       ceiling was chosen by, `fileMenu.test.js`'s own: 70px of chrome and 5.7px
       a character. What it catches is the way this number goes wrong — a
       refusal reworded longer than the panel it has to be read in, with no
       tooltip behind it to recover the rest from. */
    const longest = [
      modified({ userAgent: LINUX }),
      changeMenuItems({ path: 'vendor/', kind: 'untracked', userAgent: LINUX }),
      changeMenuItems({ path: 'src/old.js', kind: 'deleted', userAgent: LINUX }),
      modified({ insideProject: false, userAgent: LINUX })
    ]
      .flat()
      .filter((item) => item.label)
      .map((item) => item.label)
      .reduce((a, b) => (b.length > a.length ? b : a))
    expect(longest).toBe('Reveal in file manager — the file is gone from the working tree')
    expect(70 + longest.length * 5.7).toBeLessThan(CHANGE_MENU_W)
  })
})
