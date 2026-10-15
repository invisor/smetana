import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { createTracker } from '../../src/bridge/sessionState.js'

/* The same file `session::model::state_of` is checked against in src-tauri. */
const vector = JSON.parse(readFileSync(resolve(process.cwd(), 'test-vectors/session-state.json'), 'utf8'))

const withSeq = (events) => events.map((event, i) => ({ ...event, seq: i + 1 }))

describe('the session state fold against the shared vector', () => {
  it('holds sequences', () => {
    expect(vector.sequences.length).toBeGreaterThan(0)
  })

  for (const seq of vector.sequences) {
    if (seq.alive === false) {
      it(`${seq.name}: a snapshot that reports the end keeps it`, () => {
        expect(createTracker().seed(1, withSeq(seq.events), seq.final)).toBe(seq.final)
      })
      continue
    }
    it(`${seq.name}: state after every event, seeded whole`, () => {
      const events = withSeq(seq.events)
      expect(createTracker().seed(1, [], 'starting')).toBe(seq.initial ?? 'starting')
      seq.after.forEach((want, n) => {
        expect(createTracker().seed(1, events.slice(0, n + 1), 'running'), `after event ${n + 1}`).toBe(want)
      })
    })

    it(`${seq.name}: state after every event, absorbed one by one`, () => {
      const tracker = createTracker()
      let last = tracker.seed(1, [], 'starting')
      withSeq(seq.events).forEach((event, n) => {
        const said = tracker.absorb(1, [event])
        if (said !== null) last = said
        expect(last, `after event ${n + 1}`).toBe(seq.after[n])
      })
    })
  }
})
