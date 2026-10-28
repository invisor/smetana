import { describe, expect, it } from 'vitest'
import { OLD_SERVER_NOTE, TRACKER_WRITES_MINOR, canWriteTracker, sheetNote, sheetRows } from '../src/taskSheet.js'

const online = (minor) => ({ state: 'online', protocol_minor: minor })
const card = (extra = {}) => ({ id: 'x-1', status: 'ready', bdStatus: 'open', blockedBy: 0, ...extra })
const issue = (id, extra = {}) => ({ id, status: 'open', ...extra })
const rows = (extra = {}) =>
  sheetRows({ card: card(), issues: [issue('x-1')], runs: [], configState: 'ok', status: online(13), busy: false, ...extra })
const kinds = (list) => list.map((row) => row.type ?? row.kind)

describe('the sheet rows', () => {
  it('puts View task first and the desktop menu after it', () => {
    const list = rows()
    expect(list[0]).toEqual({ kind: 'view', label: 'View task', icon: 'eye' })
    expect(kinds(list)).toEqual(['view', 'run', 'ask-agent', 'follow-up', 'lock', 'move', 'separator', 'delete'])
  })

  it('shows Fix this and no Run or Edit on a done card, and Answer questions first on a parked one', () => {
    const done = rows({ card: card({ status: 'done', bdStatus: 'closed' }) })
    expect(kinds(done)).toEqual(['view', 'fix', 'follow-up', 'move', 'separator', 'delete'])
    const parked = rows({ card: card({ status: 'parked', bdStatus: 'parked' }) })
    expect(kinds(parked)[1]).toBe('resolve')
  })

  it('offers Unblock on a bd blocked card, greyed when its epic is blocked', () => {
    const list = rows({
      card: card({ status: 'blocked', bdStatus: 'blocked' }),
      issues: [issue('x-0', { status: 'blocked' }), issue('x-1', { status: 'blocked', parent: 'x-0' })]
    })
    const lock = list.find((row) => row.kind === 'lock')
    expect(lock.label).toBe('Unblock — its epic is blocked')
    expect(lock.disabled).toBe(true)
  })

  it('greys the three write rows and keeps the separator on a desktop before 1.13', () => {
    for (const status of [online(12), online(0), { state: 'connecting' }]) {
      const list = rows({ status })
      const writes = list.filter((row) => ['lock', 'move', 'delete'].includes(row.kind))
      expect(writes.map((row) => row.kind)).toEqual(['lock', 'move', 'delete'])
      expect(writes.every((row) => row.disabled === true)).toBe(true)
      expect(list.some((row) => row.type === 'separator')).toBe(true)
      expect(sheetNote(status)).toBe(OLD_SERVER_NOTE)
    }
    expect(rows({ status: online(12) }).find((row) => row.kind === 'move').children.length).toBeGreaterThan(0)
  })

  it('words the old-server note exactly', () => {
    expect(OLD_SERVER_NOTE).toBe(
      "Block, Move to... and Delete need the desktop's server restarted: it is older than the desktop app."
    )
  })

  it('leaves the write rows live and the note empty at 1.13', () => {
    const writes = rows({ status: online(13) }).filter((row) => ['lock', 'move', 'delete'].includes(row.kind))
    expect(writes).toHaveLength(3)
    expect(writes.every((row) => !row.disabled)).toBe(true)
    expect(sheetNote(online(13))).toBe('')
  })

  it('keeps them on a desktop at 1.13', () => {
    expect(TRACKER_WRITES_MINOR).toBe(13)
    expect(canWriteTracker(online(13))).toBe(true)
    expect(canWriteTracker(online(14))).toBe(true)
    expect(canWriteTracker(online(12))).toBe(false)
    expect(kinds(rows({ status: online(13) }))).toContain('delete')
  })

  it('greys Run while a run over the same scope is going', () => {
    const runs = [{ state: { kind: 'working' }, settings: { scope: { kind: 'task', id: 'x-1' } } }]
    const run = rows({ runs }).find((row) => row.kind === 'run')
    expect(run.disabled).toBe(true)
    expect(run.label).toContain('— a run over task x-1 is already going')
    expect(rows({ runs: [] }).find((row) => row.kind === 'run').disabled).toBe(false)
  })

  it('offers no Run row that works while the run configuration is missing', () => {
    expect(rows({ configState: 'missing' }).find((row) => row.kind === 'run').disabled).toBe(true)
    expect(rows({ configState: 'broken' }).find((row) => row.kind === 'run').disabled).toBe(false)
  })

  it('greys every row while a write is in flight', () => {
    const list = rows({ busy: true }).filter((row) => row.kind && row.kind !== 'view')
    expect(list.every((row) => row.disabled)).toBe(true)
  })
})
