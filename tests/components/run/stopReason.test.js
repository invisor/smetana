import { describe, expect, it } from 'vitest'
import { REASONS, TONE, stopReason } from '../../../src/components/run/stopReason.js'

describe('how the run bar draws an ending', () => {
  it('paints a run that ran out of work as the ordinary ending', () => {
    expect(stopReason('queue_empty').tone).toBe(TONE.quiet)
    expect(stopReason('cancelled').tone).toBe(TONE.quiet)
    expect(stopReason('session_removed').tone).toBe(TONE.quiet)
  })

  it('paints the endings something went wrong in as failures', () => {
    expect(stopReason('crashed').tone).toBe(TONE.failed)
    expect(stopReason('no_progress').tone).toBe(TONE.failed)
    expect(stopReason('unreadable').tone).toBe(TONE.failed)
    expect(stopReason('max_iterations').tone).toBe(TONE.failed)
    expect(stopReason('preflight').tone).toBe(TONE.failed)
  })

  /* smetana-e3o. Nothing fell over here — the agent asked something and an
     unattended run had nobody to answer — and while loudness and failure were
     one flag, this bar drew it in exactly the colour of the two above it. */
  it('does not paint a run waiting for an answer as a failure', () => {
    expect(stopReason('needs_answer').tone).not.toBe(TONE.failed)
    expect(stopReason('needs_answer').tone).toBe(TONE.needsYou)
  })

  /* The bar and the agent row sit a few centimetres apart and must not
     disagree about what waiting for a person looks like, so the tone is the
     status system's own token rather than a colour of this component's. */
  it('waits in the colour the status system gives needs-you', () => {
    expect(TONE.needsYou).toBe('var(--status-needs-you-fg)')
  })

  it('tells the three tones apart', () => {
    const tones = [TONE.quiet, TONE.failed, TONE.needsYou]
    expect(new Set(tones).size).toBe(tones.length)
  })

  /* The whole point of naming the tone per ending: a new one added without a
     tone would otherwise inherit whatever the last default happened to be. */
  it('leaves no ending without a tone of its own', () => {
    for (const [kind, reason] of Object.entries(REASONS)) {
      expect(Object.values(TONE), kind).toContain(reason.tone)
    }
  })

  it('says plainly that a reason it has never heard of stopped the run', () => {
    expect(stopReason('rate_limited_forever').text).toBe('Stopped — rate limited forever')
    expect(stopReason(undefined).text).toBe('Stopped')
    expect(stopReason(undefined).tone).toBe(TONE.failed)
  })

  /* The glyph is the second signal, and colour is never alone here either: the
     ordinary ending, the one that waits and every failure differ by
     silhouette. */
  it('gives the endings a person acts on differently their own glyphs', () => {
    expect(stopReason('queue_empty').icon).toBe('check')
    expect(stopReason('needs_answer').icon).toBe('message-circle-question-mark')
    expect(stopReason('crashed').icon).toBeUndefined()
  })
})
