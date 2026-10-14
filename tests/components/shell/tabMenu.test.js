import { describe, expect, it } from 'vitest'
import { iconNodes } from '../../../src/components/core/icons.js'
import { FILE_MENU_W, fileMenuItems } from '../../../src/components/files/fileMenu.js'
import {
  TAB_MENU_W,
  CLOSE_TERMINALS_TITLE,
  allOf,
  butTerminals,
  closeTerminalsText,
  othersOf,
  rightOf,
  savedOf,
  tabMenuItems,
  terminalsIn
} from '../../../src/components/shell/tabMenu.js'

const kinds = (items) => items.filter((i) => i.type !== 'separator').map((i) => i.kind)
const find = (items, kind) => items.find((i) => i.kind === kind)
const separators = (items) => items.filter((i) => i.type === 'separator').length

const CLOSE_GROUP = [
  'close',
  'close-others',
  'close-right',
  'close-saved',
  'close-but-terminals',
  'close-all'
]

describe('TAB_MENU_W', () => {
  it('is a number and covers the longest label on the menu', () => {
    expect(typeof TAB_MENU_W).toBe('number')
    expect(TAB_MENU_W).toBe(FILE_MENU_W)
    const longest = [
      ...tabMenuItems({ kind: 'file' }),
      ...tabMenuItems({ kind: 'file', hasLiveAgent: true }),
      ...tabMenuItems({ kind: 'file', canAttach: true, hasTerminals: true })
    ]
      .filter((item) => item.label)
      .map((item) => item.label)
      .reduce((a, b) => (b.length > a.length ? b : a))
    expect(70 + longest.length * 5.7).toBeLessThan(TAB_MENU_W)
  })
})

describe('tabMenuItems', () => {
  it('draws a file tab in the designed order and grouping', () => {
    const items = tabMenuItems({ kind: 'file' })
    expect(kinds(items)).toEqual([
      ...CLOSE_GROUP,
      'copy-path',
      'copy-relative-path',
      'reveal',
      'attach',
      'keep-open'
    ])
    expect(separators(items)).toBe(3)
    expect(items.map((i) => i.type ?? 'row')).toEqual([
      'row', 'row', 'row', 'row', 'row', 'row',
      'separator', 'row', 'row',
      'separator', 'row', 'row',
      'separator', 'row'
    ])
  })

  it('gives a preview tab the same rows as a file tab', () => {
    expect(kinds(tabMenuItems({ kind: 'preview' }))).toEqual(kinds(tabMenuItems({ kind: 'file' })))
  })

  it('offers only the close group on a diff, a terminal and the Reports page', () => {
    for (const kind of ['diff', 'terminal', 'reports']) {
      const items = tabMenuItems({ kind, hasTerminals: true })
      expect(kinds(items)).toEqual(CLOSE_GROUP)
      expect(separators(items)).toBe(0)
    }
  })

  it('offers three rows on a pinned tab, with no Close and no Close to the right', () => {
    expect(kinds(tabMenuItems({ kind: 'pinned' }))).toEqual([
      'close-others',
      'close-but-terminals',
      'close-all'
    ])
  })

  it('greys Close all but terminals when there is no shell, and only then', () => {
    expect(find(tabMenuItems({ kind: 'file' }), 'close-but-terminals').disabled).toBe(true)
    expect(find(tabMenuItems({ kind: 'file', hasTerminals: true }), 'close-but-terminals').disabled).toBe(false)
    expect(find(tabMenuItems({ kind: 'pinned' }), 'close-but-terminals').disabled).toBe(true)
  })

  it('greys Keep open on a permanent tab and lights it on a preview', () => {
    expect(find(tabMenuItems({ kind: 'file' }), 'keep-open').disabled).toBe(true)
    expect(find(tabMenuItems({ kind: 'preview' }), 'keep-open').disabled).toBe(false)
  })

  it('takes Attach to agent and its two reasons from the tree menu', () => {
    const tree = (args) => find(fileMenuItems({ target: 'file', ...args }), 'attach')
    for (const args of [{}, { hasLiveAgent: true }, { canAttach: true }]) {
      const tab = find(tabMenuItems({ kind: 'file', ...args }), 'attach')
      expect(tab.label).toBe(tree(args).label)
      expect(tab.disabled).toBe(tree(args).disabled)
    }
    expect(find(tabMenuItems({ kind: 'file' }), 'attach').disabled).toBe(true)
    expect(find(tabMenuItems({ kind: 'file', canAttach: true }), 'attach').disabled).toBe(false)
  })

  it('names the file manager after the platform', () => {
    const mac = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)'
    expect(find(tabMenuItems({ kind: 'file', userAgent: mac }), 'reveal').label).toBe('Reveal in Finder')
  })

  it('uses only registered icons', () => {
    for (const kind of ['file', 'preview', 'diff', 'pinned']) {
      for (const item of tabMenuItems({ kind })) {
        if (item.icon) expect(iconNodes, item.icon).toHaveProperty(item.icon)
      }
    }
  })
})

/* The drawn row, deliberately not in the order a store would keep the files:
   `b` was dragged to the front, so "right of `b`" is what is on screen. */
const row = [
  { id: 'kanban', kind: 'pinned' },
  { id: 'agent', kind: 'pinned' },
  { id: 'b.js', kind: 'file', dirty: true },
  { id: 'a.js', kind: 'file' },
  { id: 'c.js', kind: 'preview' },
  { id: 'diff:1', kind: 'diff' },
  { id: 'term:1', kind: 'terminal' },
  { id: 'reports', kind: 'reports' }
]
const ids = (tabs) => tabs.map((t) => t.id)

describe('othersOf', () => {
  it('is every closable tab but this one', () => {
    expect(ids(othersOf(row, 'a.js'))).toEqual(['b.js', 'c.js', 'diff:1', 'term:1', 'reports'])
  })

  it('never includes a pinned tab, even when the menu is on one', () => {
    expect(ids(othersOf(row, 'kanban'))).toEqual(['b.js', 'a.js', 'c.js', 'diff:1', 'term:1', 'reports'])
  })
})

describe('rightOf', () => {
  it('follows the drawn row', () => {
    expect(ids(rightOf(row, 'b.js'))).toEqual(['a.js', 'c.js', 'diff:1', 'term:1', 'reports'])
    expect(ids(rightOf(row, 'c.js'))).toEqual(['diff:1', 'term:1', 'reports'])
  })

  it('is empty for the last tab and for an id that is not in the row', () => {
    expect(rightOf(row, 'reports')).toEqual([])
    expect(rightOf(row, 'gone')).toEqual([])
  })
})

describe('savedOf', () => {
  it('keeps clean files, diffs and Reports, and leaves dirty files and shells', () => {
    const dirty = (id) => row.find((t) => t.id === id)?.dirty === true
    expect(ids(savedOf(row, dirty))).toEqual(['a.js', 'c.js', 'diff:1', 'reports'])
  })
})

describe('butTerminals', () => {
  it('is everything closable except the shells', () => {
    expect(ids(butTerminals(row))).toEqual(['b.js', 'a.js', 'c.js', 'diff:1', 'reports'])
  })
})

describe('allOf and terminalsIn', () => {
  it('allOf drops only the pinned tabs', () => {
    expect(ids(allOf(row))).toHaveLength(6)
    expect(ids(allOf(row))).not.toContain('kanban')
  })

  it('terminalsIn finds the shells in a set', () => {
    expect(ids(terminalsIn(allOf(row)))).toEqual(['term:1'])
    expect(terminalsIn(butTerminals(row))).toEqual([])
  })
})

describe('closeTerminalsText', () => {
  it('says one terminal in the singular and the rest in the plural', () => {
    expect(CLOSE_TERMINALS_TITLE).toBe('Close terminals too?')
    expect(closeTerminalsText(1)).toBe('1 terminal will be closed and its shell ended.')
    expect(closeTerminalsText(3)).toBe('3 terminals will be closed and their shells ended.')
  })
})
