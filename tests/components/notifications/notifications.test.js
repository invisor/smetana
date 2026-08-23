import { describe, expect, it } from 'vitest'
import {
  MIB,
  THRESHOLDS_MIB,
  crossedThreshold,
  formatDuration,
  reachedThreshold,
  rememberAfter,
  runNotification,
  stillOver,
  storageNotification,
  updateNotification
} from '../../../src/components/notifications/notifications.js'
import { REASONS } from '../../../src/components/run/stopReason.js'
import { iconNodes } from '../../../src/components/core/icons.js'

const mib = (n) => n * MIB

describe('the ladder', () => {
  it('is the three thresholds the design names, in order', () => {
    expect(THRESHOLDS_MIB).toEqual([10, 50, 100])
  })

  it('reaches nothing under the first step', () => {
    expect(reachedThreshold(0)).toBe(null)
    expect(reachedThreshold(mib(3))).toBe(null)
    expect(reachedThreshold(mib(10) - 1)).toBe(null)
  })

  it('reaches a step exactly on it', () => {
    expect(reachedThreshold(mib(10))).toBe(10)
    expect(reachedThreshold(mib(50))).toBe(50)
    expect(reachedThreshold(mib(100))).toBe(100)
  })

  it('reaches the highest step below the size, not the nearest one', () => {
    expect(reachedThreshold(mib(49))).toBe(10)
    expect(reachedThreshold(mib(99))).toBe(50)
    expect(reachedThreshold(mib(400))).toBe(100)
  })

  it('has no answer for a size nobody measured', () => {
    expect(reachedThreshold(null)).toBe(null)
    expect(reachedThreshold(undefined)).toBe(null)
    expect(reachedThreshold(-1)).toBe(null)
    expect(reachedThreshold('12')).toBe(null)
  })
})

describe('what is worth announcing', () => {
  it('says nothing about a folder under every threshold', () => {
    expect(crossedThreshold(mib(3), null)).toBe(null)
    expect(crossedThreshold(mib(9), null)).toBe(null)
  })

  it('announces the first crossing of each step once', () => {
    expect(crossedThreshold(mib(12), null)).toBe(10)
    expect(crossedThreshold(mib(60), 10)).toBe(50)
    expect(crossedThreshold(mib(120), 50)).toBe(100)
  })

  it('stays quiet for the whole of the gap above an announced step', () => {
    expect(crossedThreshold(mib(12), 10)).toBe(null)
    expect(crossedThreshold(mib(49), 10)).toBe(null)
    expect(crossedThreshold(mib(99), 50)).toBe(null)
    expect(crossedThreshold(mib(4000), 100)).toBe(null)
  })

  it('skips the step nobody was around to hear', () => {
    // Straight from nothing to 120 MiB — one card, naming where the size is
    // now, rather than three cards for three steps crossed while the app was
    // closed.
    expect(crossedThreshold(mib(120), null)).toBe(100)
  })

  it('says nothing when there was no measurement', () => {
    expect(crossedThreshold(null, 10)).toBe(null)
    expect(crossedThreshold(undefined, null)).toBe(null)
  })
})

describe('what the project remembers afterwards', () => {
  it('remembers the step just announced', () => {
    expect(rememberAfter(mib(12), null)).toBe(10)
    expect(rememberAfter(mib(60), 10)).toBe(50)
  })

  it('re-arms the ladder when the folder falls back below a step', () => {
    expect(rememberAfter(mib(3), 10)).toBe(null)
    expect(rememberAfter(mib(12), 100)).toBe(10)
    // And the re-armed step speaks again.
    expect(crossedThreshold(mib(12), rememberAfter(mib(3), 10))).toBe(10)
  })

  it('keeps what it knew when the measurement did not happen', () => {
    expect(rememberAfter(null, 50)).toBe(50)
    expect(rememberAfter(undefined, null)).toBe(null)
  })

  it('forgets a number that is not one of the steps as soon as it is measured', () => {
    // A hand-edited file is Rust's to refuse, but nothing here leans on the
    // stored number being on the ladder: it is only ever compared.
    expect(rememberAfter(mib(12), 37)).toBe(10)
    expect(crossedThreshold(mib(12), 37)).toBe(null)
  })
})

describe('whether a card is still true', () => {
  it('stands while the folder still reaches the step it was announced at', () => {
    expect(stillOver(mib(12), 10)).toBe(true)
    expect(stillOver(mib(10), 10)).toBe(true)
    expect(stillOver(mib(400), 10)).toBe(true)
  })

  it('goes the moment the folder falls under it', () => {
    expect(stillOver(mib(9), 10)).toBe(false)
    expect(stillOver(0, 10)).toBe(false)
  })

  it('is not true of a measurement that never happened', () => {
    expect(stillOver(null, 10)).toBe(false)
  })
})

describe('the card', () => {
  const card = storageNotification('/Users/you/Projects/smetana', mib(12) + 512 * 1024, 10)

  it('is one card per project and step, so a repeat replaces rather than piles up', () => {
    expect(card.id).toBe('storage:/Users/you/Projects/smetana:10')
    expect(storageNotification('/Users/you/Projects/other', mib(12), 10).id).not.toBe(card.id)
  })

  it('names the folder, the size and the step', () => {
    expect(card.body).toContain('smetana')
    expect(card.body).toContain('12.5 MiB')
    expect(card.body).toContain('10 MiB')
  })

  it('says where the button leads rather than promising a deletion', () => {
    expect(card.actionLabel).toBe('Clean up')
    expect(card.body).toContain('Storage in settings')
  })

  it('carries the source it came from, which is what a second one would plug into', () => {
    expect(card.source).toBe('storage')
  })
})

/* A run the worker has finished with, in the shape it crosses the wire in. */
const stopped = (over = {}) => ({
  token: 3,
  project: '/Users/you/Projects/smetana',
  state: { kind: 'stopped', reason: { kind: 'queue_empty' } },
  summary: {
    seconds: 8040,
    tasks: { closed: [{ id: 'a-1', title: 'One' }], parked: [] },
    report: '/Users/you/Projects/smetana/.smetana/reports/2026-08-12-143155.html'
  },
  ...over
})

describe('how long the run took', () => {
  it('reads in hours, minutes and seconds', () => {
    expect(formatDuration(8040)).toBe('2h 14m')
    expect(formatDuration(840)).toBe('14m')
    expect(formatDuration(48)).toBe('48s')
  })

  it('keeps the minutes beside a whole hour, so the number cannot be misread', () => {
    expect(formatDuration(3600)).toBe('1h 0m')
  })

  it('has no answer for a length nobody measured', () => {
    // The same distinction projectBytes draws: not measured is not zero.
    expect(formatDuration(NaN)).toBe(null)
    expect(formatDuration(-1)).toBe(null)
    expect(formatDuration('60')).toBe(null)
    expect(formatDuration(undefined)).toBe(null)
    expect(formatDuration(Infinity)).toBe(null)
  })
})

describe('the card for a run that is over', () => {
  it('says the ending, the counts and the duration and nothing else', () => {
    const card = runNotification(stopped())
    expect(card.id).toBe('run:3')
    expect(card.source).toBe('run')
    expect(card.token).toBe(3)
    expect(card.title).toBe('Run finished')
    expect(card.body).toContain('1 closed')
    expect(card.body).toContain('0 parked')
    expect(card.body).toContain('2h 14m')
    expect(card.actionLabel).toBe('Show details')
  })

  it('takes the ending word for word from the table the run bar draws', () => {
    // One authored copy of the wording, so the card and the bar a few
    // centimetres away cannot describe the same run differently.
    expect(runNotification(stopped()).body).toContain(REASONS.queue_empty.text)
    expect(runNotification(stopped()).icon).toBe(REASONS.queue_empty.icon)
  })

  it('announces the unhappy endings too, which are the ones worth reading', () => {
    const crashed = runNotification(
      stopped({ state: { kind: 'stopped', reason: { kind: 'crashed', attempts: 5 } } })
    )
    expect(crashed.body).toContain(REASONS.crashed.text)
    expect(crashed.body).toContain('1 closed')
  })

  it('draws an ending it has never heard of without borrowing a glyph', () => {
    const card = runNotification(
      stopped({ state: { kind: 'stopped', reason: { kind: 'something_new' } } })
    )
    expect(card.icon).toBe('square')
    expect(card.body).toContain('something new')
  })

  it('is not made for a run that is still going', () => {
    expect(runNotification({ ...stopped(), state: { kind: 'working', iteration: 0 } })).toBe(null)
    expect(runNotification({ ...stopped(), state: { kind: 'paused', pct: 100 } })).toBe(null)
    expect(runNotification(null)).toBe(null)
  })

  it('does not offer details it has no document for', () => {
    const card = runNotification(stopped({ summary: { seconds: 60, tasks: null, report: null } }))
    expect(card.actionLabel).toBeUndefined()
    expect(card.report).toBe(null)
  })

  it('never turns an unread board into a count of zero', () => {
    const card = runNotification(stopped({ summary: { seconds: 60, tasks: null, report: null } }))
    expect(card.body).not.toContain('0 closed')
    expect(card.body).toContain('could not be read')
  })

  it('survives a run from a worker that carries no summary at all', () => {
    // This front end may be older than the worker, and an ending nobody can
    // describe is still an ending. Absent rather than null on purpose: that is
    // the shape a front end predating the field sees, and the shape a fixture
    // built one field at a time has. Its sibling below carries the wire's own.
    const card = runNotification(stopped({ summary: undefined }))
    expect(card).not.toBe(null)
    expect(card.actionLabel).toBeUndefined()
    expect(card.body).toContain(REASONS.queue_empty.text)
  })

  /* The case this tells apart from the one above it, and the reason the two are
     not one branch. Pressing Stop between batches ends the run at once, while
     the account is made a moment later by the loop and arrives through
     `Run::take_summary_from` — so a run with no summary has not failed to read
     the board, nothing has looked yet. The card says only what it knows, and a
     few seconds later the same card carries the real counts.

     `summary: null` and not `undefined`, because this is the case that comes
     off the wire: `Option<RunSummary>` serialises as null, and a suite where
     neither of the two absences is written in the shape the back end actually
     sends is one refactor away from passing against a contract nobody holds. */
  it('claims nothing about the board before anything has read it', () => {
    const card = runNotification(
      stopped({ summary: null, state: { kind: 'stopped', reason: { kind: 'cancelled' } } })
    )
    expect(card.body).not.toContain('could not be read')
    expect(card.body).not.toContain('closed')
    expect(card.body).toBe(REASONS.cancelled.text)
  })
})

const readyCard = () => updateNotification({ kind: 'ready', version: '0.2.0' })

describe('the card for an update that is waiting', () => {
  it('names the version and says where the button leads', () => {
    const card = updateNotification({ kind: 'ready', version: '0.2.0' })

    expect(card.source).toBe('update')
    expect(card.id).toBe('update:0.2.0')
    expect(card.version).toBe('0.2.0')
    expect(card.title).toBe('Update ready')
    expect(card.body).toContain('Smetana 0.2.0')
    // The button opens About; the card says so rather than promising to install
    // from the panel, which is the same honesty the storage card keeps about
    // Clean up.
    expect(card.body).toContain('Install opens About in settings')
    expect(card.actionLabel).toBe('Install')
  })

  it('is the only state of the machine that says anything at all', () => {
    // Checking and downloading are not news — the app fetches quietly — and a
    // check that could not reach GitHub is not something to interrupt somebody
    // with. All three belong on About, where a person went looking.
    expect(updateNotification({ kind: 'idle' })).toBe(null)
    expect(updateNotification({ kind: 'checking' })).toBe(null)
    expect(updateNotification({ kind: 'available', version: '0.2.0' })).toBe(null)
    expect(updateNotification({ kind: 'downloading', received: 1, total: 2 })).toBe(null)
    expect(updateNotification({ kind: 'failed', message: 'no' })).toBe(null)
  })

  it('says nothing where there is nobody to ask', () => {
    expect(updateNotification(null)).toBe(null)
    expect(updateNotification({ kind: 'verifying' })).toBe(null)
  })

  it('still announces a ready update that arrived without a version', () => {
    const card = updateNotification({ kind: 'ready', version: '' })

    expect(card.id).toBe('update:ready')
    expect(card.version).toBe(null)
    expect(card.body).toContain('A new version has been downloaded')
  })

  it('draws a glyph the icon list already registers', () => {
    // An unregistered name makes Icon warn in dev and draw nothing, and no test
    // in this repository can see a component.
    expect(iconNodes[readyCard().icon]).toBeTruthy()
  })
})
