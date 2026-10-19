import { describe, expect, it } from 'vitest'
import {
  dayOf,
  displayCommand,
  splitHead,
  timeOf,
  timelineRows,
  toolLabel,
  toolTone
} from '../../../src/components/agent/batchTimeline.js'

const at = (h, m, s, day = 19) => new Date(2026, 9, day, h, m, s).toISOString()
const A = at(10, 0, 0)

const tool = (seq, extra = {}) => ({ seq, at: A, kind: 'tool', id: `t${seq}`, tool: 'Bash', command: 'cat a', cwd: null, ...extra })
const result = (seq, id, extra = {}) => ({ seq, at: A, kind: 'result', id, ok: true, summary: '3 lines', ...extra })
const opts = { project: '/p', exited: false }

describe('timelineRows', () => {
  it('folds a result into its call and draws no row for it', () => {
    const rows = timelineRows([tool(1), result(2, 't1')], opts)
    expect(rows).toHaveLength(1)
    expect(rows[0]).toMatchObject({ kind: 'tool', live: false, result: { ok: true, summary: '3 lines' } })
  })

  it('carries a failed result as it is', () => {
    const rows = timelineRows([tool(1), result(2, 't1', { ok: false, summary: '1 line' })], opts)
    expect(rows[0].result).toEqual({ ok: false, summary: '1 line' })
  })

  it('marks the open call live until the session exits', () => {
    expect(timelineRows([tool(1)], opts)[0]).toMatchObject({ live: true, result: null })
    expect(timelineRows([tool(1)], { ...opts, exited: true })[0]).toMatchObject({ live: false, result: null })
  })

  it('ignores a result whose call was trimmed away', () => {
    expect(timelineRows([result(5, 'gone')], opts)).toEqual([])
  })

  it('inserts a day row where the local date changes', () => {
    const rows = timelineRows([tool(1, { at: at(23, 59, 0) }), tool(2, { at: at(0, 1, 0, 20) })], opts)
    expect(rows.map((r) => r.kind)).toEqual(['tool', 'day', 'tool'])
    expect(rows[1].date).toBe(dayOf(at(0, 1, 0, 20)))
  })

  it('does not open with a day row', () => {
    expect(timelineRows([tool(1)], opts).map((r) => r.kind)).toEqual(['tool'])
  })

  it('splits the command at its first whitespace and keeps the whole command', () => {
    const [row] = timelineRows([tool(1, { command: 'git status\n  --short' })], opts)
    expect(row).toMatchObject({ head: 'git', rest: 'status\n  --short', command: 'git status\n  --short' })
  })

  it('shows a file tool path relative to the project and the whole path expanded', () => {
    const [row] = timelineRows([tool(1, { tool: 'Read', command: '/p/src/x.js' })], opts)
    expect(row).toMatchObject({ head: 'src/x.js', rest: '', command: '/p/src/x.js' })
    const [outside] = timelineRows([tool(1, { tool: 'Read', command: '/etc/hosts' })], opts)
    expect(outside.head).toBe('/etc/hosts')
  })

  it('leaves a Bash command that mentions the project alone', () => {
    expect(displayCommand({ tool: 'Bash', command: '/p/x' }, '/p')).toBe('/p/x')
  })

  it('carries say and note rows with their time', () => {
    const rows = timelineRows(
      [
        { seq: 1, at: at(9, 8, 7), kind: 'note', text: 'session start' },
        { seq: 2, at: at(9, 8, 8), kind: 'say', text: 'Hello `x`' }
      ],
      opts
    )
    expect(rows).toEqual([
      { key: 1, kind: 'note', time: '09:08:07', text: 'session start' },
      { key: 2, kind: 'say', time: '09:08:08', text: 'Hello `x`' }
    ])
  })

  it('gives a tool row its label and tone', () => {
    const [row] = timelineRows([tool(1, { tool: 'ScheduleWakeup', command: 'in 5m' })], opts)
    expect(row).toMatchObject({ toolLabel: 'Wakeup', tone: 'var(--status-blocked-fg)', time: '10:00:00' })
  })
})

describe('labels and tones', () => {
  it('labels and tones the tools', () => {
    expect(toolLabel('ScheduleWakeup')).toBe('Wakeup')
    expect(toolLabel('Task')).toBe('Agent')
    expect(toolLabel('AskUserQuestion')).toBe('Ask')
    expect(toolLabel('NotebookEdit')).toBe('Notebook')
    expect(toolLabel('Bash')).toBe('Bash')
    expect(toolTone('Read')).toBe('var(--status-ready-fg)')
    expect(toolTone('Grep')).toBe('var(--status-ready-fg)')
    expect(toolTone('Task')).toBe('var(--status-blocked-fg)')
    expect(toolTone('Edit')).toBe('var(--status-done-fg)')
    expect(toolTone('MultiEdit')).toBe('var(--status-done-fg)')
    expect(toolTone('Bash')).toBe('var(--text-secondary)')
    expect(toolTone('Whatever')).toBe('var(--text-secondary)')
  })
})

describe('small helpers', () => {
  it('splits head and rest', () => {
    expect(splitHead('cat a b')).toEqual({ head: 'cat', rest: 'a b' })
    expect(splitHead('cat')).toEqual({ head: 'cat', rest: '' })
    expect(splitHead('')).toEqual({ head: '', rest: '' })
  })

  it('reads local time and date, and nothing from garbage', () => {
    expect(timeOf(at(1, 2, 3))).toBe('01:02:03')
    expect(dayOf(at(1, 2, 3))).toBe('2026-10-19')
    expect(timeOf('nope')).toBe('')
  })
})
