import { describe, expect, it } from 'vitest'
import { connectionNotice, isStale, workSentence } from '../../../src/components/shell/connectionNotice.js'

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

  it('offers to withdraw a restart that is waiting', () => {
    const outdated = { running: '0.0.9', bundled: '0.1.0', work: [], canRestart: true }
    const notice = connectionNotice({ ...base, outdated, restartScheduled: true })
    expect(notice.restart).toEqual({ canNow: false, canWhenIdle: false, canCancel: true })
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
