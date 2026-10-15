import { describe, expect, it } from 'vitest'
import { agentRowsFor, drivenRecords, entryFor, loud } from '../src/agentList.js'

const terminalRow = (id, over = {}) => ({
  id,
  conversation: null,
  label: 'Agent',
  tasks: [],
  state: 'running',
  elapsed: '4m',
  startedAt: 1000 * id,
  ...over
})
const clocks = { statusOf: (state) => (state === 'exited' ? 'done' : state), elapsed: () => '1m' }
const driven = (id, over = {}) => ({
  id,
  project: '/p',
  state: 'ready',
  startedAt: 100000 + id,
  conversation: null,
  work: { kind: 'bare' },
  title: null,
  ...over
})

describe('the rows of the agents list', () => {
  it('shows the sessions of the project in front and no others', () => {
    const records = drivenRecords([driven(1), driven(2, { project: '/other' })], '/p', clocks)
    expect(records.map((record) => record.id)).toEqual([1])
  })

  it('says a driven session\'s state in the design system\'s words', () => {
    const [record] = drivenRecords([driven(1, { state: 'exited' })], '/p', clocks)
    expect(record.state).toBe('done')
    expect(record.elapsed).toBe('1m')
  })

  it('draws a conversation and a terminal session in one list, newest first', () => {
    const rows = agentRowsFor([terminalRow(1), terminalRow(2)], drivenRecords([driven(1)], '/p', clocks))
    expect(rows).toHaveLength(3)
    expect(rows[0].id).toBe('conversation:1')
  })

  it('does not draw a start that has no session yet, nor an offer to resume a closed one', () => {
    const rows = agentRowsFor(
      [terminalRow('start-1', { starting: true }), terminalRow(5, { restored: true }), terminalRow(2)],
      []
    )
    expect(rows.map((row) => row.id)).toEqual([2])
  })

  it('has no rows for nothing', () => {
    expect(agentRowsFor(undefined, undefined)).toEqual([])
  })
})

describe('how a row opens', () => {
  it('opens a conversation by the session number the worker knows it under', () => {
    const [row] = agentRowsFor([], drivenRecords([driven(7)], '/p', clocks))
    expect(entryFor(row)).toEqual({ screen: 'conversation', id: 7, driven: true })
  })

  it('opens a terminal session read-only, under its own counter', () => {
    expect(entryFor(terminalRow(7))).toEqual({ screen: 'conversation', id: 7, driven: false })
  })

  it('keeps conversation 7 and terminal 7 apart', () => {
    const rows = agentRowsFor([terminalRow(7)], drivenRecords([driven(7)], '/p', clocks))
    const entries = rows.map(entryFor)
    expect(entries).toContainEqual({ screen: 'conversation', id: 7, driven: true })
    expect(entries).toContainEqual({ screen: 'conversation', id: 7, driven: false })
  })
})

describe('the row that shouts', () => {
  it('is the one waiting on a person, by the design system\'s own rule', () => {
    expect(loud({ state: 'needs-you' })).toBe(true)
    expect(loud({ state: 'running' })).toBe(false)
    expect(loud({ state: 'ready' })).toBe(false)
  })

  it('includes a failed one, which the design system also calls loud', () => {
    expect(loud({ state: 'failed' })).toBe(true)
  })
})
