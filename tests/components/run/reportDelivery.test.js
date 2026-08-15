import { describe, expect, it } from 'vitest'
import { deliveryFor } from '../../../src/components/run/reportDelivery.js'

/* A run as the worker hands one over: stopped, with an account behind it and
   the session that ran its last batch still named. */
const stopped = (over = {}) => ({
  token: 7,
  project: '/p',
  state: { kind: 'stopped', reason: { kind: 'batch_done' } },
  last_session: 3,
  summary: { report: '/p/.smetana/reports/2026-08-15-101500.html', seconds: 90, tasks: null },
  ...over
})

describe('deliveryFor', () => {
  it('opens the tab when the agent that did the work is the one selected', () => {
    expect(deliveryFor(stopped(), 3, new Set())).toBe('tab')
  })

  it('leaves it to the bell while another agent is selected', () => {
    expect(deliveryFor(stopped(), 4, new Set())).toBe('bell')
  })

  it('leaves it to the bell while no agent is selected at all', () => {
    expect(deliveryFor(stopped(), null, new Set())).toBe('bell')
  })

  it('never reads two absent sessions as the same agent', () => {
    // The trap in the obvious `run.last_session === selected`: a run from a
    // worker too old to name its session, met by a window with nothing
    // selected, would open a tab neither of them asked for.
    expect(deliveryFor(stopped({ last_session: null }), null, new Set())).toBe('bell')
  })

  it('leaves it to the bell for a run whose worker never named a session', () => {
    expect(deliveryFor(stopped({ last_session: undefined }), 3, new Set())).toBe('bell')
  })

  it('says nothing about a run that is still going', () => {
    expect(deliveryFor(stopped({ state: { kind: 'working', iteration: 1 } }), 3, new Set())).toBe(
      null
    )
  })

  it('leaves it to the bell when there is no document to open', () => {
    // A card with no button is what the bell already draws for this, and a tab
    // is the one delivery that cannot happen without a file.
    expect(deliveryFor(stopped({ summary: null }), 3, new Set())).toBe('bell')
    expect(deliveryFor(stopped({ summary: { report: null, seconds: 90 } }), 3, new Set())).toBe(
      'bell'
    )
  })

  it('delivers once, however often the list is read again', () => {
    // `loadRun` replaces the whole list on focus and on a project switch, so
    // without this the same ending would open its tab again every time.
    expect(deliveryFor(stopped(), 3, new Set([7]))).toBe(null)
  })

  it('says nothing rather than throwing for a run in a shape it cannot read', () => {
    expect(deliveryFor(null, 3, new Set())).toBe(null)
    expect(deliveryFor({}, 3, new Set())).toBe(null)
  })
})
