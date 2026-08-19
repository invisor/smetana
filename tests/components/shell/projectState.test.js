import { describe, expect, it } from 'vitest'
import { projectSummary, stateLabel } from '../../../src/components/shell/projectState.js'

describe('stateLabel', () => {
  it('counts what is waiting before what is running', () => {
    expect(stateLabel({ state: 'loud', live: 3, loud: 1 })).toBe('1 agent waiting on you')
    expect(stateLabel({ state: 'loud', live: 0, loud: 2 })).toBe('2 agents waiting on you')
  })

  it('counts what is running when nothing is waiting', () => {
    expect(stateLabel({ state: 'live', live: 1, loud: 0 })).toBe('1 agent running')
    expect(stateLabel({ state: 'live', live: 4, loud: 0 })).toBe('4 agents running')
  })

  it('says idle for a project with nothing going on, and for one nobody measured', () => {
    expect(stateLabel({ state: 'idle', live: 0, loud: 0 })).toBe('idle')
    expect(stateLabel(undefined)).toBe('idle')
  })
})

describe('projectSummary', () => {
  it('joins the branch and the state with a middle dot', () => {
    expect(projectSummary('develop', { state: 'live', live: 1, loud: 0 })).toBe(
      'develop · 1 agent running'
    )
  })

  it('drops the branch when there is none, rather than opening with a dot', () => {
    expect(projectSummary('', { state: 'idle', live: 0, loud: 0 })).toBe('idle')
    expect(projectSummary(undefined, undefined)).toBe('idle')
  })
})
