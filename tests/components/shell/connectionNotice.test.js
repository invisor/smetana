import { describe, expect, it } from 'vitest'
import {
  IDLE_NOTE,
  SCHEDULED_IDLE_NOTE,
  connectionNotice,
  isStale,
  serverIndicator,
  workSentence
} from '../../../src/components/shell/connectionNotice.js'

const base = { mode: 'client', state: 'connected', reason: null, outdated: null }

describe('what the strip says about the connection', () => {
  it('says nothing without a server of its own, and nothing while connected and current', () => {
    expect(connectionNotice({ mode: 'browser', state: 'connected' })).toBe(null)
    expect(connectionNotice(base)).toBe(null)
    expect(connectionNotice(null)).toBe(null)
  })

  it('says what happens to the data and to writes when the connection is lost', () => {
    const notice = connectionNotice({ ...base, state: 'reconnecting', reason: 'the server stopped' })
    expect(notice.tone).toBe('wait')
    expect(notice.detail).toContain('marked stale')
    expect(notice.detail).toContain('not kept for later')
    expect(notice.detail).toContain('(the server stopped)')
    expect(notice.canRetry).toBe(true)
  })

  it('tells a state that needs a person from one that mends itself', () => {
    expect(connectionNotice({ ...base, state: 'connecting' }).tone).toBe('wait')
    expect(connectionNotice({ ...base, state: 'incompatible' }).tone).toBe('blocked')
    expect(connectionNotice({ ...base, state: 'unpaired' }).tone).toBe('blocked')
    expect(connectionNotice({ ...base, state: 'failed', reason: 'no data directory' }).canRetry).toBe(false)
  })

  it('offers a restart of an older server only when it can be asked for', () => {
    const outdated = { running: '0.0.9', bundled: '0.1.0', work: [{ kind: 'run', label: 'smetana' }], canRestart: true }
    const notice = connectionNotice({ ...base, outdated })
    expect(notice.restart).toEqual({ canNow: true, canWhenIdle: true, canCancel: false })
    expect(notice.detail).toContain('Run: smetana')
    expect(connectionNotice({ ...base, outdated: { ...outdated, canRestart: false } }).restart).toBe(null)
  })

  it('leaves a restart that is waiting to the indicator, and offers to withdraw it there', () => {
    const outdated = { running: '0.0.9', bundled: '0.1.0', work: [], canRestart: true }
    const connection = { ...base, outdated, restartScheduled: true }
    expect(connectionNotice(connection)).toBe(null)
    expect(serverIndicator(connection).restart).toEqual({ canNow: false, canWhenIdle: false, canCancel: true })
  })

  it('counts the work a restart would end', () => {
    expect(workSentence([])).toBe('Nothing is running.')
    expect(workSentence([{ kind: 'shell', label: 'zsh' }])).toContain('1 thing is running')
    const many = ['a', 'b', 'c', 'd', 'e'].map((label) => ({ kind: 'agent', label }))
    expect(workSentence(many)).toContain('and 2 more')
  })

  it('calls the data stale in every state but connected, and only in the desktop app', () => {
    expect(isStale({ ...base, state: 'reconnecting' })).toBe(true)
    expect(isStale(base)).toBe(false)
    expect(isStale({ mode: 'browser', state: 'connecting' })).toBe(false)
  })
})

describe('the notice for a server on another machine', () => {
  const remote = (state, over = {}) => ({ mode: 'client', state, local: false, reason: null, ...over })

  it('does not say the app starts a server it does not own', () => {
    expect(connectionNotice(remote('connecting')).detail).toBe('Reaching the server you chose in Settings.')
    expect(connectionNotice({ ...remote('connecting'), local: true }).detail).toBe('Starting it if it is not running.')
  })

  it('sends a device the server forgot to the place a new code is typed', () => {
    expect(connectionNotice(remote('unpaired')).detail).toMatch(/Settings, Server/)
    expect(connectionNotice({ ...remote('unpaired'), local: true }).detail).not.toMatch(/Settings, Server/)
  })
})

describe('the scope bar indicator for an older server', () => {
  const outdated = { running: '0.1.0', bundled: '0.3.3', work: [{ kind: 'run', label: 'smetana' }], canRestart: true }

  it('says the server needs a restart and offers both choices, in the banner\'s words', () => {
    const indicator = serverIndicator({ ...base, outdated })
    expect(indicator.label).toBe('Server needs a restart')
    expect(indicator.title).toBe('The server is older than this app')
    expect(indicator.restart).toEqual({ canNow: true, canWhenIdle: true, canCancel: false })
    for (const text of [indicator.detail, connectionNotice({ ...base, outdated }).detail]) {
      expect(text).toContain('The server is 0.1.0, this app ships 0.3.3.')
      expect(text).toContain('Run: smetana')
      expect(text).toContain(IDLE_NOTE)
      expect(text).not.toContain('Nothing is ended')
    }
  })

  it('offers no restart when the server cannot be restarted from here', () => {
    expect(serverIndicator({ ...base, outdated: { ...outdated, canRestart: false } }).restart).toBe(null)
  })

  it('says nothing when current, while reconnecting, or without a server of its own', () => {
    expect(serverIndicator(base)).toBe(null)
    expect(serverIndicator(null)).toBe(null)
    expect(serverIndicator({ ...base, outdated, state: 'reconnecting' })).toBe(null)
    expect(serverIndicator({ ...base, outdated, mode: 'browser' })).toBe(null)
  })

  it('carries a waiting restart that the banner no longer draws', () => {
    const connection = { ...base, outdated, restartScheduled: true }
    expect(connectionNotice(connection)).toBe(null)
    const indicator = serverIndicator(connection)
    expect(indicator.title).toBe('The server will restart when its work is done')
    expect(indicator.detail).toContain(SCHEDULED_IDLE_NOTE)
    expect(indicator.restart).toEqual({ canNow: false, canWhenIdle: false, canCancel: true })
  })
})
