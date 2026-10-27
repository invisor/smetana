import { describe, expect, it } from 'vitest'
import { TRACKER_WRITES_MINOR, canWriteTracker, cardScope, runnable, sheetRows } from '../src/taskSheet.js'

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

  it('drops the three write rows and the separator on a desktop before 1.13', () => {
    expect(kinds(rows({ status: online(12) }))).toEqual(['view', 'run', 'ask-agent', 'follow-up'])
    expect(kinds(rows({ status: online(0) }))).toEqual(['view', 'run', 'ask-agent', 'follow-up'])
    expect(kinds(rows({ status: { state: 'connecting' } }))).toEqual(['view', 'run', 'ask-agent', 'follow-up'])
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

  it('greys every row while a write is in flight', () => {
    const list = rows({ busy: true }).filter((row) => row.kind && row.kind !== 'view')
    expect(list.every((row) => row.disabled)).toBe(true)
  })
})

describe('cardScope', () => {
  it('is an epic when a child is not finished, a task otherwise', () => {
    const issues = new Map([
      ['x-1', issue('x-1')],
      ['x-2', issue('x-2', { parent: 'x-1' })],
    ])
    expect(cardScope(issues, 'x-1')).toEqual({ kind: 'epic', id: 'x-1' })
    expect(cardScope(issues, 'x-2')).toEqual({ kind: 'task', id: 'x-2' })
    expect(cardScope([issue('x-1'), issue('x-2', { status: 'closed', parent: 'x-1' })], 'x-1').kind).toBe('task')
  })
})

describe('runnable', () => {
  it('refuses a run on done, blocked, parked and unconfigured', () => {
    expect(runnable(card(), 'ok')).toBe(true)
    expect(runnable(card(), 'broken')).toBe(true)
    expect(runnable(card({ status: 'done', bdStatus: 'closed' }), 'ok')).toBe(false)
    expect(runnable(card({ status: 'blocked', bdStatus: 'blocked' }), 'ok')).toBe(false)
    expect(runnable(card({ status: 'parked', bdStatus: 'parked' }), 'ok')).toBe(false)
    expect(runnable(card(), 'missing')).toBe(false)
  })

  it('refuses a card that waits on a blocker and would have to be moved', () => {
    expect(runnable(card({ status: 'blocked', bdStatus: 'open', blockedBy: 1 }), 'ok')).toBe(false)
    expect(runnable(card({ status: 'running', bdStatus: 'in_progress', blockedBy: 1 }), 'ok')).toBe(true)
  })
})
