import { describe, expect, it } from 'vitest'
import {
  UNAVAILABLE,
  installRefusal,
  readyVersion,
  updateAction,
  updateKind,
  updateLine,
  updatesKnown
} from '../../../src/components/settings/update.js'

const MIB = 1024 * 1024

/* The six tags `UpdateState` puts on the wire, one example of each, in the
   order a person meets them. Written out here rather than derived, so a state
   renamed in Rust and not here shows up as a test naming a state nothing
   answers for. */
const IDLE = { kind: 'idle' }
const CHECKING = { kind: 'checking' }
const AVAILABLE = { kind: 'available', version: '0.2.0', notes: null, date: '2026-08-20' }
const DOWNLOADING = { kind: 'downloading', received: 12 * MIB, total: 48 * MIB }
const READY = { kind: 'ready', version: '0.2.0' }
const FAILED = { kind: 'failed', message: 'Could not check for updates: the feed timed out.' }

describe('which of the states this is', () => {
  it('reads each of the six tags Rust sends', () => {
    expect(updateKind(IDLE)).toBe('idle')
    expect(updateKind(CHECKING)).toBe('checking')
    expect(updateKind(AVAILABLE)).toBe('available')
    expect(updateKind(DOWNLOADING)).toBe('downloading')
    expect(updateKind(READY)).toBe('ready')
    expect(updateKind(FAILED)).toBe('failed')
  })

  it('reads no answer at all as unavailable rather than as idle', () => {
    // A browser: `stores/updates.js` holds null, and "you are up to date" said
    // by a window that never asked anybody would be a claim about a machine
    // nobody consulted.
    expect(updateKind(null)).toBe(UNAVAILABLE)
    expect(updateKind(undefined)).toBe(UNAVAILABLE)
    expect(updateKind({})).toBe(UNAVAILABLE)
  })

  it('reads a state this build has never heard of as unavailable', () => {
    // The whole reason the state travels as a tag: an unknown one matches
    // nothing, where a missing boolean would be indistinguishable from false.
    expect(updateKind({ kind: 'verifying' })).toBe(UNAVAILABLE)
  })

  it('draws the row for the six and for nothing else', () => {
    for (const state of [IDLE, CHECKING, AVAILABLE, DOWNLOADING, READY, FAILED]) {
      expect(updatesKnown(state)).toBe(true)
    }
    expect(updatesKnown(null)).toBe(false)
    expect(updatesKnown({ kind: 'verifying' })).toBe(false)
  })
})

describe('what is waiting to be installed', () => {
  it('names the version only while one is ready', () => {
    expect(readyVersion(READY)).toBe('0.2.0')
    expect(readyVersion(AVAILABLE)).toBe(null)
    expect(readyVersion(DOWNLOADING)).toBe(null)
    expect(readyVersion(IDLE)).toBe(null)
    expect(readyVersion(null)).toBe(null)
  })

  it('treats an empty version as none, since a sentence naming "" is worse', () => {
    // `Machine::ready` falls back to an empty string if it somehow has no
    // version to carry over from the check.
    expect(readyVersion({ kind: 'ready', version: '' })).toBe(null)
  })
})

describe('the sentence under the label', () => {
  it('says nothing is waiting, and how the app looks by itself', () => {
    expect(updateLine(IDLE)).toContain('No update is waiting')
    expect(updateLine(IDLE)).toContain('once a day')
  })

  it('says a check is going', () => {
    expect(updateLine(CHECKING)).toBe('Looking for a new version…')
  })

  it('names the release found and says the download is starting', () => {
    expect(updateLine(AVAILABLE)).toBe('Smetana 0.2.0 was found; the download is starting.')
  })

  it('says a version was found even when it arrived without a number', () => {
    expect(updateLine({ kind: 'available' })).toBe(
      'A new version was found; the download is starting.'
    )
  })

  it('counts a download against its total, in binary units and whole percent', () => {
    expect(updateLine(DOWNLOADING)).toBe('Downloading — 12.0 MiB of 48.0 MiB (25%).')
  })

  it('rounds the percent down, so it never reads 100% with bytes still coming', () => {
    expect(updateLine({ kind: 'downloading', received: 48 * MIB - 1, total: 48 * MIB })).toContain(
      '(99%)'
    )
  })

  it('says how much has arrived when the server never said how much there is', () => {
    // `total` is None until the response headers are read, and some servers
    // never say. A size with no end is a truer drawing than one invented.
    expect(updateLine({ kind: 'downloading', received: 3 * MIB, total: null })).toBe(
      'Downloading — 3.0 MiB so far.'
    )
  })

  it('starts a download at nothing received rather than at no answer', () => {
    expect(updateLine({ kind: 'downloading' })).toBe('Downloading — 0 bytes so far.')
  })

  it('names what is ready and warns that installing restarts the app', () => {
    expect(updateLine(READY)).toBe(
      'Smetana 0.2.0 is downloaded and ready. Installing restarts the app.'
    )
    expect(updateLine({ kind: 'ready', version: '' })).toBe(
      'A new version is downloaded and ready. Installing restarts the app.'
    )
  })

  it('shows the failure in Rust’s own words', () => {
    expect(updateLine(FAILED)).toBe('Could not check for updates: the feed timed out.')
  })

  it('still says something for a failure that arrived with no message', () => {
    expect(updateLine({ kind: 'failed', message: '' })).toBe('The last check did not finish.')
    expect(updateLine({ kind: 'failed' })).toBe('The last check did not finish.')
  })

  it('says nothing at all where there is nobody to ask', () => {
    expect(updateLine(null)).toBe('')
    expect(updateLine({ kind: 'verifying' })).toBe('')
  })
})

describe('the control the row offers', () => {
  it('offers a check from idle and from a failure, since a later check can still succeed', () => {
    expect(updateAction(IDLE)).toEqual({
      verb: 'check',
      label: 'Check for updates',
      disabled: false
    })
    expect(updateAction(FAILED)).toEqual({
      verb: 'check',
      label: 'Check for updates',
      disabled: false
    })
  })

  it('keeps the button while a check is going, disabled and saying so', () => {
    // Drawn disabled rather than taken away: a control that vanished for the
    // length of a round trip and grew back is a moving target.
    expect(updateAction(CHECKING)).toEqual({ verb: 'check', label: 'Checking…', disabled: true })
  })

  it('offers nothing for the two states that finish by themselves', () => {
    expect(updateAction(AVAILABLE)).toBe(null)
    expect(updateAction(DOWNLOADING)).toBe(null)
  })

  it('offers the install, and never draws it dead on a guess', () => {
    // The run gate is Rust's to answer: this window cannot see a run in a
    // project nobody is looking at, so a control disabled here would be wrong
    // as often as it was right, and silent either way.
    expect(updateAction(READY)).toEqual({
      verb: 'install',
      label: 'Install and restart',
      disabled: false
    })
  })

  it('offers nothing where there is nobody to ask', () => {
    expect(updateAction(null)).toBe(null)
    expect(updateAction({ kind: 'verifying' })).toBe(null)
  })
})

describe('why an install did not happen', () => {
  it('names the projects the run gate refused for', () => {
    expect(installRefusal({ kind: 'run_live', detail: { projects: 'smetana, notes' } })).toBe(
      'A run is going in smetana, notes. Installing restarts the app, which would end it.'
    )
  })

  it('still explains the gate when the refusal named no project', () => {
    expect(installRefusal({ kind: 'run_live', detail: {} })).toBe(
      'A run is going. Installing restarts the app, which would end it.'
    )
  })

  it('says there is nothing to install when the state moved under the press', () => {
    expect(installRefusal({ kind: 'nothing_ready' })).toBe(
      'There is no downloaded update to install.'
    )
  })

  it('says a development build does not replace itself', () => {
    expect(installRefusal({ kind: 'development_build' })).toBe(
      'A development build does not replace itself.'
    )
  })

  it('passes through the two refusals that carry somebody else’s words', () => {
    // The run worker could not be reached, and the install itself would not go
    // through: both messages are the only part that says what actually
    // happened, so they are shown rather than replaced.
    expect(installRefusal({ kind: 'runs', detail: 'the run worker did not answer' })).toBe(
      'the run worker did not answer'
    )
    expect(installRefusal({ kind: 'install', detail: 'permission denied' })).toBe(
      'permission denied'
    )
  })

  it('has something to say about a refusal in a shape this build does not know', () => {
    expect(installRefusal({ kind: 'quarantined' })).toBe('The update could not be installed.')
    expect(installRefusal(new Error('the channel broke'))).toBe('the channel broke')
  })

  it('shows a broken channel in whatever words it arrived with', () => {
    // Not every failure on the way to Rust is one of the five refusals; a
    // string is what a channel that broke hands over.
    expect(installRefusal('the command never reached the app')).toBe(
      'the command never reached the app'
    )
  })

  it('says nothing when nothing was refused', () => {
    expect(installRefusal(null)).toBe(null)
    expect(installRefusal(undefined)).toBe(null)
  })
})
