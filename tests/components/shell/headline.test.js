import { describe, expect, it } from 'vitest'
import { headline } from '../../../src/components/shell/headline.js'

const running = (over = {}) => ({ token: 1, state: { kind: 'working', iteration: 0 }, ...over })
const stopped = (over = {}) => ({ token: 2, state: { kind: 'stopped', reason: 'done' }, ...over })

describe('headline', () => {
  it('an agent waiting on the person outranks a run and every other agent', () => {
    expect(headline({ row: { state: 'loud', live: 3, loud: 1 }, runs: [running()] })).toEqual({
      text: '1 agent needs you',
      level: 'loud'
    })
  })

  it('counts the agents that are waiting, not the ones that are not', () => {
    expect(headline({ row: { state: 'loud', live: 4, loud: 2 }, runs: [] })).toEqual({
      text: '2 agents need you',
      level: 'loud'
    })
  })

  it('names a run under way ahead of the agents working inside it', () => {
    expect(headline({ row: { state: 'live', live: 2, loud: 0 }, runs: [running()] })).toEqual({
      text: 'Run under way',
      level: 'live'
    })
  })

  it('counts running agents when no run is behind them', () => {
    expect(headline({ row: { state: 'live', live: 2, loud: 0 }, runs: [] })).toEqual({
      text: '2 agents running',
      level: 'live'
    })
    expect(headline({ row: { state: 'live', live: 1, loud: 0 }, runs: [] })).toEqual({
      text: '1 agent running',
      level: 'live'
    })
  })

  it('says nothing at all when nothing is happening', () => {
    expect(headline({ row: { state: 'idle', live: 0, loud: 0 }, runs: [] })).toEqual({
      text: '',
      level: 'quiet'
    })
  })

  it('says nothing for a project nobody has a session under', () => {
    expect(headline({ row: undefined, runs: [] })).toEqual({ text: '', level: 'quiet' })
    expect(headline({})).toEqual({ text: '', level: 'quiet' })
    expect(headline()).toEqual({ text: '', level: 'quiet' })
  })

  it('a run that has stopped is not a run under way', () => {
    expect(headline({ row: { state: 'idle', live: 0, loud: 0 }, runs: [stopped()] })).toEqual({
      text: '',
      level: 'quiet'
    })
  })

  it('reads the whole list, since the newest run being over says nothing about the rest', () => {
    expect(headline({ row: undefined, runs: [running({ token: 1 }), stopped({ token: 2 })] })).toEqual({
      text: 'Run under way',
      level: 'live'
    })
  })

  it('treats a state it has never heard of as a run still going', () => {
    expect(headline({ row: undefined, runs: [running({ state: { kind: 'hibernating' } })] })).toEqual({
      text: 'Run under way',
      level: 'live'
    })
  })
})
