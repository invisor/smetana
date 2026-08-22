import { describe, expect, it } from 'vitest'
import { deliveryFor } from '../../../src/components/run/reportDelivery.js'

/* A run as the worker hands one over: stopped, with an account behind it and
   the session that ran its last batch still named. `last_session` is kept in
   the fixture deliberately — the rule no longer reads it, and a fixture that
   dropped it would stop being able to say so. */
const stopped = (over = {}) => ({
  token: 7,
  project: '/p',
  state: { kind: 'stopped', reason: { kind: 'batch_done' } },
  last_session: 3,
  summary: { report: '/p/.smetana/reports/2026-08-15-101500.html', seconds: 90, tasks: null },
  ...over
})

describe('deliveryFor', () => {
  it('opens the tab as soon as the run is over', () => {
    expect(deliveryFor(stopped(), true, new Set())).toBe('tab')
  })

  it('opens it whichever agent the person happened to have selected', () => {
    // The one condition is the setting. Which agent was selected when the run
    // stopped was the invisible reason a report "sometimes" appeared, and it
    // was removed rather than kept under the switch.
    expect(deliveryFor(stopped({ last_session: 4 }), true, new Set())).toBe('tab')
    expect(deliveryFor(stopped({ last_session: null }), true, new Set())).toBe('tab')
    expect(deliveryFor(stopped({ last_session: undefined }), true, new Set())).toBe('tab')
  })

  it('says nothing about a run that is still going', () => {
    expect(deliveryFor(stopped({ state: { kind: 'working', iteration: 1 } }), true, new Set())).toBe(
      null
    )
  })

  it('leaves it to the bell when there is no document to open', () => {
    // A card with no button is what the bell already draws for this, and a tab
    // is the one delivery that cannot happen without a file. The switch cannot
    // cancel this: it decides whether the report is shown, and a run that never
    // wrote one has nothing to show either way.
    expect(deliveryFor(stopped({ summary: null }), true, new Set())).toBe('bell')
    expect(deliveryFor(stopped({ summary: { report: null, seconds: 90 } }), true, new Set())).toBe(
      'bell'
    )
  })

  it('shows nothing at all with the switch off, not even the bell', () => {
    // A card is a delivery of the same report — it is a button onto that very
    // document — so leaving it up would answer somebody who asked not to be
    // shown their reports with a smaller version of the thing they declined.
    expect(deliveryFor(stopped(), false, new Set())).toBe('none')
  })

  it('shows nothing with the switch off even where there was no document', () => {
    expect(deliveryFor(stopped({ summary: null }), false, new Set())).toBe('none')
  })

  it('delivers once, however often the list is read again', () => {
    // `loadRun` replaces the whole list on focus and on a project switch, so
    // without this the same ending would open its tab again every time.
    expect(deliveryFor(stopped(), true, new Set([7]))).toBe(null)
    expect(deliveryFor(stopped(), false, new Set([7]))).toBe(null)
  })

  it('says nothing rather than throwing for a run in a shape it cannot read', () => {
    expect(deliveryFor(null, true, new Set())).toBe(null)
    expect(deliveryFor({}, true, new Set())).toBe(null)
  })
})
