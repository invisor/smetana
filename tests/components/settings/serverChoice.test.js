import { describe, expect, it } from 'vitest'
import {
  askSentence,
  localStopLine,
  nextStep,
  relayHost,
  shortNode,
  stateLine,
  targetLine,
  ticketProblem
} from '../../../src/components/settings/serverChoice.js'

const code = `smetana2:${'a'.repeat(60)}`

describe('the pasted code', () => {
  it('has nothing to say about an empty field, and accepts one that looks like a desktop code', () => {
    expect(ticketProblem('')).toBe(null)
    expect(ticketProblem('   ')).toBe(null)
    expect(ticketProblem(code)).toBe(null)
    expect(ticketProblem(`  ${code}\n`)).toBe(null)
  })

  it('tells a phone\'s code from a desktop\'s before anything is attempted', () => {
    expect(ticketProblem(`smetana1:${'a'.repeat(60)}`)).toMatch(/smetanad pair --kind desktop/)
  })

  it('refuses what is not a code, a broken one and one cut short', () => {
    expect(ticketProblem('hello')).toMatch(/not a Smetana pairing code/)
    expect(ticketProblem(`smetana2:${'a'.repeat(30)}\n${'a'.repeat(30)}`)).toMatch(/line break/)
    expect(ticketProblem('smetana2:abc')).toMatch(/cut short/)
  })
})

describe('what pressing the button does first', () => {
  it('stops at a bad or missing code', () => {
    expect(nextStep({ ticket: '', usingLocal: true, work: [] }).kind).toBe('problem')
    expect(nextStep({ ticket: 'nope', usingLocal: true, work: [] }).kind).toBe('problem')
  })

  it('goes straight ahead when the server being left has nothing running, and leaves it running', () => {
    expect(nextStep({ ticket: code, usingLocal: true, work: [] })).toEqual({ kind: 'switch', stopLocal: false })
    expect(nextStep({ ticket: code, usingLocal: true, work: null })).toEqual({ kind: 'switch', stopLocal: false })
  })

  it('asks first when it has work in it, and says what', () => {
    const work = [{ kind: 'run', label: 'run' }, { kind: 'shell', label: 'shell' }]
    const step = nextStep({ ticket: code, usingLocal: true, work })
    expect(step).toEqual({ kind: 'ask', work })
    expect(askSentence(work)).toMatch(/Switching does not end it/)
    expect(askSentence(work)).toMatch(/Run: run/)
  })

  it('does not ask about the local server when this device was not using it', () => {
    expect(nextStep({ ticket: code, usingLocal: false, work: [{ kind: 'run' }] }).kind).toBe('switch')
  })
})

describe('how the choice is put into words', () => {
  it('names the local server and a remote one, with the relay and not the whole key', () => {
    expect(targetLine({ kind: 'local' })).toMatch(/on this machine/)
    expect(targetLine(undefined)).toMatch(/on this machine/)
    const line = targetLine({ kind: 'remote', node: 'a'.repeat(64), relay: 'https://relay.example.com/' })
    expect(line).toMatch(/another machine/)
    expect(line).toMatch(/relay\.example\.com/)
    expect(line).not.toContain('a'.repeat(64))
    expect(shortNode('abc')).toBe('abc')
    expect(relayHost('not a url')).toBe('not a url')
  })

  it('says how the connection is going', () => {
    expect(stateLine({ state: 'connected', server: { version: '0.2.0' } })).toBe('Connected, server 0.2.0')
    expect(stateLine({ state: 'connected' })).toBe('Connected')
    expect(stateLine({ state: 'unpaired' })).toBe('Not paired with this server')
    expect(stateLine({ state: 'failed' })).toBe('')
  })

  it('reports what happened to the server that was left', () => {
    expect(localStopLine({ localStop: 'scheduled' })).toMatch(/once its work is finished/)
    expect(localStopLine({ localStop: 'stopping' })).toMatch(/is stopping/)
    expect(localStopLine({ localStop: { failed: 'x' } })).toMatch(/still running/)
    expect(localStopLine({ localStop: null })).toBe('')
    expect(localStopLine(null)).toBe('')
  })
})
