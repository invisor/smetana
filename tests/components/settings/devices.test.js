import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import {
  DEFAULT_RELAY_URL,
  MAX_CODE_SECONDS,
  countdownLine,
  lastContactLine,
  pairedLine,
  platformLabel,
  qrCode,
  relayProblem,
  remainingSeconds,
  requestDetail,
  requestLine,
  validRelayUrl
} from '../../../src/components/settings/devices.js'

describe('the pairing code', () => {
  it('draws a ticket string as a square of modules', () => {
    const code = qrCode('smetana1:' + 'a'.repeat(160))
    expect(code.size).toBeGreaterThan(20)
    expect(code.path).toMatch(/^M\d+ \d+h\d+v1h-\d+z/)
  })

  it('is one segment per run of dark modules and never one past the edge', () => {
    const { size, path } = qrCode('smetana1:abcdefghij')
    for (const [, x, y, run] of path.matchAll(/M(\d+) (\d+)h(\d+)v1h-\d+z/g).map((m) => m.map(Number))) {
      expect(x + run).toBeLessThanOrEqual(size)
      expect(y).toBeLessThan(size)
    }
  })

  it('draws nothing for text that cannot be encoded', () => {
    expect(qrCode('')).toBeNull()
    expect(qrCode(null)).toBeNull()
    expect(qrCode('x'.repeat(10_000))).toBeNull()
  })
})

describe('the countdown', () => {
  it('counts whole seconds down and stops at zero', () => {
    expect(remainingSeconds(1_000 + 120, 1_000_000)).toBe(120)
    expect(remainingSeconds(1_000 + 120, 1_000_000 + 500)).toBe(120)
    expect(remainingSeconds(1_000 + 120, 1_000_000 + 119_001)).toBe(1)
    expect(remainingSeconds(1_000 + 120, 1_000_000 + 130_000)).toBe(0)
  })

  it('does not believe a code that claims to last for a day', () => {
    expect(remainingSeconds(1_000 + 86_400, 1_000_000)).toBe(MAX_CODE_SECONDS)
    expect(remainingSeconds(Number.NaN, 0)).toBe(0)
  })

  it('reads as minutes and seconds, and says when it is over', () => {
    expect(countdownLine(105)).toBe('Expires in 1:45')
    expect(countdownLine(9)).toBe('Expires in 0:09')
    expect(countdownLine(0)).toBe('This code has expired.')
  })
})

describe('devices and requests in words', () => {
  it('names the platform, and does not guess at one it does not know', () => {
    expect(platformLabel('ios')).toBe('iOS')
    expect(platformLabel('android')).toBe('Android')
    expect(platformLabel(null)).toBe('Unknown platform')
  })

  it('asks about the phone by the name it gave itself', () => {
    expect(requestLine({ deviceName: 'Pixel 8', platform: 'android' })).toBe('Pixel 8 wants to connect')
    expect(requestLine({ deviceName: '  ', platform: null })).toBe('A phone wants to connect')
    expect(requestDetail({ deviceName: 'x', platform: 'ios' })).toMatch(/^iOS\. Only accept/)
    expect(requestLine(null)).toBe('')
  })

  it('says how long ago a phone was last in touch', () => {
    const now = 1_000_000_000_000
    const at = (secondsAgo) => ({ lastContact: now / 1000 - secondsAgo })
    expect(lastContactLine({ lastContact: null }, now)).toBe('Not connected yet')
    expect(lastContactLine(at(5), now)).toBe('Last seen just now')
    expect(lastContactLine(at(4 * 60), now)).toBe('Last seen 4 min ago')
    expect(lastContactLine(at(3 * 3600), now)).toBe('Last seen 3 h ago')
    expect(lastContactLine(at(86_400), now)).toBe('Last seen 1 day ago')
    expect(lastContactLine(at(5 * 86_400), now)).toBe('Last seen 5 days ago')
  })

  it('dates the pairing, and says nothing without a date', () => {
    expect(pairedLine({ pairedAt: 1_760_011_200 })).toBe('Paired 9 Oct 2025')
    expect(pairedLine({ pairedAt: null })).toBe('')
  })
})

describe('the relay address', () => {
  it('keeps what Rust keeps', () => {
    for (const good of [
      'https://relay.example.org',
      'https://relay.example.org:8443/',
      'https://localhost',
      '  https://relay.example.org/path?x=1  '
    ]) {
      expect(validRelayUrl(good), good).toBe(true)
      expect(relayProblem(good), good).toBeNull()
    }
  })

  it('refuses what Rust would quietly replace with the default', () => {
    for (const bad of [
      '',
      '   ',
      'http://relay.example.org',
      'relay.example.org',
      'ftp://relay.example.org',
      'https://',
      'https:///path',
      'https://relay example.org',
      'https://user@relay.example.org',
      'https://203.0.113.7',
      'https://203.0.113.7:4433/',
      'https://-bad.example.org',
      `https://${'a'.repeat(3000)}.example.org`,
      null,
      7
    ]) {
      expect(validRelayUrl(bad), String(bad)).toBe(false)
    }
  })

  it('says what is wrong in a sentence', () => {
    expect(relayProblem('')).toBe('Enter the address of a relay.')
    expect(relayProblem('http://relay.example.org')).toBe('A relay address must start with https://.')
    expect(relayProblem('https://203.0.113.7')).toMatch(/host name, not an IP address/)
  })

  /* The front end holds one copy of the default and Rust holds the other; this
     is the only thing that notices when they part. */
  it('ships the relay the desktop ships', () => {
    const rust = readFileSync(resolve(process.cwd(), 'crates/core/src/remote/mod.rs'), 'utf8')
    const shipped = rust.match(/pub const DEFAULT_RELAY_URL: &str = "([^"]+)";/)
    expect(shipped, 'DEFAULT_RELAY_URL must be a string constant in crates/core/src/remote/mod.rs').not.toBeNull()
    expect(DEFAULT_RELAY_URL).toBe(shipped[1])
    expect(validRelayUrl(DEFAULT_RELAY_URL)).toBe(true)
  })
})
