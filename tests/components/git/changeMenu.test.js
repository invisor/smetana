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
  it('offers six rows in four groups, the diff the click opens first', () => {
    const items = modified({ userAgent: MAC })
    expect(kinds(items)).toEqual([
      'open-changes',
      'open-file',
      'reveal',
      'copy-path',
      'copy-relative-path',
      'discard'
    ])
    expect(separators(items)).toBe(3)
  })

  it('keeps the one row that loses work alone at the foot, behind a separator', () => {
    // `branchMenu.js`'s own shape for `Delete this branch`: a pointer that
    // missed by a row must not land on the only act here with no undo.
    const items = modified()
    let at = -1
    items.forEach((item, i) => {
      if (item.type === 'separator') at = i
    })
    expect(items.slice(at + 1).map((item) => item.kind)).toEqual(['discard'])
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
    expect(icons).toEqual(['git-compare', 'file', 'folder-open', 'copy', 'copy', 'undo-2'])
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

  it('says nothing about a run or an operation in flight while nothing is going', () => {
    // The five rows that read have `branchMenu.js`'s fourth reach and keep it:
    // with nothing holding the repository there is no caption row at all.
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

  it('closes on every pick, since no row here asks a second time in the panel', () => {
    // `keepOpen` is the file tree's Delete alone. The discard asks twice as
    // well, but in a window of its own rather than in a menu that stays up, so
    // the panel closes on the pick like every other row here.
    expect(modified().some((item) => item.keepOpen)).toBe(false)
  })

  it('marks the discard as the one dangerous row and leaves the five readers plain', () => {
    const tones = verbs(modified()).map((item) => item.tone ?? null)
    expect(tones).toEqual([null, null, null, null, null, 'danger'])
  })
})

describe('the discard, and the two things that refuse it', () => {
  /* The one row of this menu that writes, so the one row the panel's own
     refusals reach. The sentences are `frozen`'s in `branchMenu.js`, which is
     where they are written, and they arrive here as a caption over the group
     rather than as a suffix: one fact refuses the whole group, which is the
     case a caption is for. */
  it('heads its group with `frozen`\'s sentence while a run holds the project', () => {
    const items = modified({ allowed: false })
    const caption = items.find((item) => item.type === 'label')
    expect(caption.label).toBe('A run is going in this project')
    expect(find(items, 'discard').disabled).toBe(true)
    // The row keeps its plain label: the reason is said once, above it.
    expect(find(items, 'discard').label).toBe('Discard changes')
  })

  it('says the other sentence while git is already working in this repository', () => {
    const items = modified({ busy: true })
    expect(items.find((item) => item.type === 'label').label).toBe(
      'Git is working in this repository'
    )
    expect(find(items, 'discard').disabled).toBe(true)
  })

  it('leaves the five rows that read live under that caption', () => {
    // The caption is read as being true of whatever is greyed below it, which
    // here is one row — the argument `branchMenu.js` records about its own
    // menu having no unbroken run of greyed rows either.
    expect(off(modified({ allowed: false, busy: true }))).toEqual(['discard'])
  })

  it('refuses a conflicted row in the label, since that fact is about the row', () => {
    const items = changeMenuItems({ path: 'src/main.js', kind: 'conflicted', userAgent: MAC })
    expect(off(items)).toEqual(['discard'])
    expect(find(items, 'discard').label).toBe('Discard changes — resolve the conflict first')
    // Nothing else on the row changes: a conflicted file still opens, still
    // reveals and is still copied.
    expect(items.some((item) => item.type === 'label')).toBe(false)
  })

  it('says the caption rather than the conflict when both are true', () => {
    // Two different reaches at once. The caption is the group's and the suffix
    // is the row's, so they do not compete: both are drawn, and the row is off
    // either way.
    const items = changeMenuItems({ path: 'src/main.js', kind: 'conflicted', allowed: false })
    expect(items.find((item) => item.type === 'label').label).toBe('A run is going in this project')
    expect(find(items, 'discard').label).toBe('Discard changes — resolve the conflict first')
    expect(find(items, 'discard').disabled).toBe(true)
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
      modified({ insideProject: false, userAgent: LINUX }),
      // The last group's own two sentences, the caption's and the suffix's.
      changeMenuItems({ path: 'src/main.js', kind: 'conflicted', allowed: false, busy: true })
    ]
      .flat()
      .filter((item) => item.label)
      .map((item) => item.label)
      .reduce((a, b) => (b.length > a.length ? b : a))
    expect(longest).toBe('Reveal in file manager — the file is gone from the working tree')
    expect(70 + longest.length * 5.7).toBeLessThan(CHANGE_MENU_W)
  })
})
