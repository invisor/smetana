import { describe, expect, it } from 'vitest'
import { REASONS, TONE, endingDetail, stopReason } from '../../../src/components/run/stopReason.js'

describe('how the run bar draws an ending', () => {
  it('paints a run that ran out of work as the ordinary ending', () => {
    expect(stopReason('queue_empty').tone).toBe(TONE.quiet)
    expect(stopReason('cancelled').tone).toBe(TONE.quiet)
    expect(stopReason('session_removed').tone).toBe(TONE.quiet)
  })

  /* A Crew run that took its one batch and merged it did exactly what it was
     asked, so its ending is as quiet as running out of work — but it is not
     that sentence: tasks may still sit in Ready, and "nothing left to take"
     would be a lie about them. */
  it('says a one-batch run finished its batch rather than that it ran out of work', () => {
    expect(stopReason('batch_done').tone).toBe(TONE.quiet)
    expect(stopReason('batch_done').text).toBe('Done — the batch is finished')
    expect(stopReason('batch_done').text).not.toBe(stopReason('queue_empty').text)
  })

  it('paints the endings something went wrong in as failures', () => {
    expect(stopReason('crashed').tone).toBe(TONE.failed)
    expect(stopReason('no_progress').tone).toBe(TONE.failed)
    expect(stopReason('unreadable').tone).toBe(TONE.failed)
    expect(stopReason('max_iterations').tone).toBe(TONE.failed)
    expect(stopReason('preflight').tone).toBe(TONE.failed)
  })

  /* The pair the worker's `StopReason::NothingDone` exists to keep apart: one
     is a batch that ran and left the board stuck, which sends somebody to the
     tracker, and the other is a session that came back having done nothing,
     which sends them to the agent. Both are failures and both are drawn in one
     colour, so the sentence is the whole of the difference — and until this
     entry existed the second fell through to the unknown-reason fallback and
     read as "Stopped — nothing done" beside "Stuck — a whole batch changed
     nothing". */
  it('tells a run whose agent did nothing from one whose board is stuck', () => {
    expect(stopReason('nothing_done').tone).toBe(TONE.failed)
    expect(stopReason('nothing_done').text).toBe('Stopped — the agent came back having done nothing')
    expect(stopReason('nothing_done').text).not.toBe(stopReason('no_progress').text)
    /* Not the fallback's sentence, which is the version-skew path and says so
       of itself: a build that knows this ending must not read as one that does
       not. */
    expect(stopReason('nothing_done').text).not.toBe(stopReason('nothing_done_x').text)
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
    expect(stopReason('crashed').icon).toBe('square')
  })

  /* The same argument the tone test above makes, and it was a live duplication
     before: while an entry could leave its glyph out, each caller supplied its
     own `?? 'square'`, so the bar and the bell's card each kept a copy of the
     glyph seven of the ten endings actually draw. Nothing may need one now. */
  it('leaves no ending without a glyph, and no caller a default to write', () => {
    for (const [kind, reason] of Object.entries(REASONS)) {
      expect(reason.icon, kind).toBeTruthy()
    }
    expect(stopReason('rate_limited_forever').icon).toBe('square')
    expect(stopReason(undefined).icon).toBe('square')
  })
})

describe('what the second line says about an ending', () => {
  /* The defect this was written for: the worker names what would not come up
     — `sh: docker: command not found` — and the bar drew the target branch
     instead, so "Could not start into develop" pointed at a branch that had
     nothing to do with it and the one sentence explaining the failure was on
     the wire and nowhere on screen. */
  it('says what would not start rather than which branch it was aimed at', () => {
    expect(endingDetail({ kind: 'preflight', detail: '`docker compose up -d` exited 127' }, 'into develop')).toBe(
      '`docker compose up -d` exited 127'
    )
  })

  /* The same field carries a batch that could not be spawned at all, which is
     the other thing the worker reports as `preflight` — a machine with no
     agent installed says so here or nowhere. */
  it('carries a batch that could not be started either', () => {
    expect(endingDetail({ kind: 'preflight', detail: 'no coding agent is installed' }, 'into main')).toBe(
      'no coding agent is installed'
    )
  })

  /* The question outranks it: an ending that has both is the agent waiting,
     and what it asked is what decides whether somebody goes and answers. */
  it('prefers the question an agent is waiting on', () => {
    expect(endingDetail({ kind: 'needs_answer', question: 'Trust this folder?' }, 'into develop')).toBe(
      'Trust this folder?'
    )
  })

  /* The one payload that is a number rather than a sentence. Without it the
     ending fell through to the branch and drew "…having done nothing into
     main", which is the same garden path the `preflight` defect above made —
     and the count is the whole of what somebody wants to know next. One batch
     is said as one batch, because a run allowed a single batch stops on its
     first empty one and a streak of one would name a threshold nobody
     reached. */
  it('counts the batches that did nothing, and says one of them as one', () => {
    expect(endingDetail({ kind: 'nothing_done', batches: 3 }, 'into main')).toBe(
      '3 batches in a row, none of which did anything'
    )
    expect(endingDetail({ kind: 'nothing_done', batches: 1 }, 'into main')).toBe(
      'one batch, and it did nothing at all'
    )
    /* A count this build cannot make sense of is not a line: better the branch
       than "undefined batches in a row". */
    expect(endingDetail({ kind: 'nothing_done' }, 'into main')).toBe('into main')
  })

  /* Every other ending keeps the line it has always had. */
  it('falls back to the branch, and to nothing when there is none', () => {
    expect(endingDetail({ kind: 'queue_empty' }, 'into develop')).toBe('into develop')
    expect(endingDetail({ kind: 'crashed', attempts: 3 }, '')).toBe('')
    expect(endingDetail(undefined, 'into develop')).toBe('into develop')
  })
})
