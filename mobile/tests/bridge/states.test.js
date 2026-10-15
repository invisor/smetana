import { describe, expect, it } from 'vitest'
import { isOnline, lastSeenText, screenFor } from '../../src/bridge/states.js'

describe('which screen the shell draws', () => {
  it('waits for the core before choosing', () => {
    expect(screenFor(null, { state: 'connecting' })).toBe('loading')
  })
  it('an unpaired phone has one thing to do', () => {
    expect(screenFor(false, { state: 'online' })).toBe('pair')
  })
  it('a too-old app gets the update screen whatever else is true', () => {
    expect(screenFor(true, { state: 'update-required', store_url: 'x' })).toBe('update')
  })
  it('maps each state of the link', () => {
    expect(screenFor(true, { state: 'connecting' })).toBe('connecting')
    expect(screenFor(true, { state: 'online' })).toBe('home')
    expect(screenFor(true, { state: 'update-recommended' })).toBe('home')
    expect(screenFor(true, { state: 'unreachable', last_seen: 1 })).toBe('unreachable')
    expect(screenFor(true, { state: 'revoked' })).toBe('revoked')
  })
  it('online means usable, recommended included', () => {
    expect(isOnline({ state: 'online' })).toBe(true)
    expect(isOnline({ state: 'update-recommended' })).toBe(true)
    expect(isOnline({ state: 'update-required' })).toBe(false)
    expect(isOnline(undefined)).toBe(false)
  })
})

describe('last seen', () => {
  const now = new Date(2026, 9, 15, 18, 0).getTime()
  it('says so when the phone never reached the desktop', () => {
    expect(lastSeenText(null, now)).toMatch(/not reached/)
  })
  it('gives the time for today and the day for earlier', () => {
    expect(lastSeenText(new Date(2026, 9, 15, 9, 5).getTime(), now, 'en-US')).toBe('Last seen at 9:05 AM.')
    expect(lastSeenText(new Date(2026, 9, 13, 9, 5).getTime(), now, 'en-US')).toBe('Last seen Oct 13 at 9:05 AM.')
  })
})
