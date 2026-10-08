import { describe, expect, it } from 'vitest'
import { needsSignIn, signInLabel, signInRowKey } from '../../../src/components/conversation/signInHint.js'

describe('which text asks for a sign-in', () => {
  it('matches the sentence Codex prints for a revoked refresh token', () => {
    expect(
      needsSignIn(
        'Your access token could not be refreshed because your refresh token was revoked. Please log out and sign in again.'
      )
    ).toBe(true)
  })

  it('matches the older Claude Code wording', () => {
    expect(needsSignIn('Invalid API key · Please run /login')).toBe(true)
  })

  it('matches what an unauthenticated Claude Code actually prints', () => {
    expect(needsSignIn('Not logged in · Please run /login')).toBe(true)
  })

  it('matches the documented expiry wording', () => {
    expect(needsSignIn('Anthropic profile login expired')).toBe(true)
  })

  it('ignores case', () => {
    expect(needsSignIn('NOT LOGGED IN')).toBe(true)
  })

  it('does not match other failures', () => {
    expect(needsSignIn('rate limited')).toBe(false)
    expect(needsSignIn('the workspace could not be sandboxed')).toBe(false)
  })

  it('does not match empty or non-text values', () => {
    expect(needsSignIn('')).toBe(false)
    expect(needsSignIn(null)).toBe(false)
    expect(needsSignIn(undefined)).toBe(false)
  })
})

describe('which row carries the offer', () => {
  const text = 'Please log out and sign in again.'

  it('is the last of two rows with the same text', () => {
    const rows = [
      { key: 1, kind: 'user', text: 'hello' },
      { key: 2, kind: 'activity', state: 'failed', text },
      { key: 3, kind: 'error', text }
    ]
    expect(signInRowKey(rows)).toBe(3)
  })

  it('skips a later row that is about something else', () => {
    const rows = [
      { key: 1, kind: 'error', text },
      { key: 2, kind: 'error', text: 'rate limited' }
    ]
    expect(signInRowKey(rows)).toBe(1)
  })

  it('ignores agent prose that happens to say the phrase', () => {
    expect(signInRowKey([{ key: 1, kind: 'agent', text }])).toBe(null)
  })

  it('ignores an activity strip that did not fail', () => {
    expect(signInRowKey([{ key: 1, kind: 'activity', state: 'done', text }])).toBe(null)
  })

  it('is null for an empty journal', () => {
    expect(signInRowKey([])).toBe(null)
  })
})

describe('what the buttons say', () => {
  it('names both ways of signing in to Codex', () => {
    expect(signInLabel('codex', 'browser', 2)).toBe('Sign in with ChatGPT')
    expect(signInLabel('codex', 'deviceCode', 2)).toBe('Sign in with a device code')
  })

  it('draws one plain button for a harness with a single variant', () => {
    expect(signInLabel('claude', 'browser', 1)).toBe('Sign in')
  })

  it('does not name ChatGPT for another harness with several variants', () => {
    expect(signInLabel('other', 'browser', 2)).toBe('Sign in with a browser')
  })
})
