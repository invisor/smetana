import { describe, expect, it } from 'vitest'
import {
  CLIENT_MACHINE_KINDS,
  visibleItems,
  withoutClientActions
} from '../../../src/components/overlays/clientActions.js'
import { fileMenuItems } from '../../../src/components/files/fileMenu.js'
import { tabMenuItems } from '../../../src/components/shell/tabMenu.js'
import { sessionMenuItems } from '../../../src/components/agent/sessionMenu.js'
import { linkMenuItems } from '../../../src/components/conversation/linkMenu.js'

const kinds = (items) => items.filter((item) => item.kind).map((item) => item.kind)
const bars = (items) => items.map((item) => (item.type === 'separator' ? '|' : 'x')).join('')

describe('hiding the rows that act on the client machine', () => {
  it('leaves a menu alone when the server is on this machine', () => {
    const items = fileMenuItems({ target: 'file' })
    expect(visibleItems(items, false)).toBe(items)
  })

  it('removes every row of the file tree that names the file manager and keeps the copy rows', () => {
    const items = fileMenuItems({ target: 'file' })
    expect(kinds(items)).toContain('reveal')
    const shown = visibleItems(items, true)
    expect(kinds(shown)).not.toContain('reveal')
    expect(kinds(shown)).toEqual(expect.arrayContaining(['open-terminal', 'copy-path', 'copy-relative-path', 'rename']))
  })

  it('removes the session log, its folder and the working directory but not the resume rows', () => {
    const shown = visibleItems(sessionMenuItems({ resume: { available: true }, fork: { available: true } }), true)
    for (const gone of ['open-log', 'reveal-log', 'open-cwd']) expect(kinds(shown), gone).not.toContain(gone)
    expect(kinds(shown)).toEqual(expect.arrayContaining(['copy-resume', 'copy-id', 'copy-path', 'delete']))
  })

  it('removes the reveal row of a tab and of a link and leaves no bar over nothing', () => {
    const tab = visibleItems(tabMenuItems({ kind: 'file' }).filter(Boolean), true)
    expect(kinds(tab)).not.toContain('reveal')
    const link = visibleItems(linkMenuItems({}), true)
    expect(kinds(link)).not.toContain('reveal')
    for (const items of [tab, link]) {
      expect(bars(items).startsWith('|')).toBe(false)
      expect(bars(items).endsWith('|')).toBe(false)
      expect(bars(items)).not.toContain('||')
    }
  })

  it('collapses the separators a hidden group leaves behind', () => {
    const items = [
      { kind: 'a' },
      { type: 'separator' },
      { kind: 'reveal' },
      { type: 'separator' },
      { kind: 'b' },
      { type: 'separator' },
      { kind: 'open-cwd' }
    ]
    expect(bars(withoutClientActions(items))).toBe('x|x')
  })

  it('names every kind the builders produce for the client machine', () => {
    // A row of this sort that is added to a builder without being added to the list would be drawn
    // for a remote server; this holds the list to the builders in this repository.
    const produced = new Set([
      ...kinds(fileMenuItems({ target: 'file' })),
      ...kinds(sessionMenuItems({})),
      ...kinds(linkMenuItems({}))
    ])
    for (const kind of CLIENT_MACHINE_KINDS) expect(produced.has(kind), kind).toBe(true)
  })
})
