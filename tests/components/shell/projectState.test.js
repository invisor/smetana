import { describe, expect, it } from 'vitest'
import { projectSummary, stateLabel } from '../../../src/components/shell/projectState.js'

describe('stateLabel', () => {
  it('counts what is waiting before what is running', () => {
    expect(stateLabel({ state: 'loud', live: 3, loud: 1 })).toBe('1 waiting on you')
    expect(stateLabel({ state: 'loud', live: 0, loud: 2 })).toBe('2 waiting on you')
  })

  it('counts what is running when nothing is waiting', () => {
    expect(stateLabel({ state: 'live', live: 1, loud: 0 })).toBe('1 running')
    expect(stateLabel({ state: 'live', live: 4, loud: 0 })).toBe('4 running')
  })

  /* The count is a count of sessions, and `SessionMark` does not say which of
     them are agents — a shell is marked exactly like one. Naming a noun here
     would be a claim the map cannot support, and one `liveAgentCount` answers
     differently on the same screen; see the module's own note. */
  it('names no noun, because the map behind it cannot tell an agent from a shell', () => {
    expect(stateLabel({ state: 'live', live: 1, loud: 0 })).not.toMatch(/agent|shell|session/)
    expect(stateLabel({ state: 'loud', live: 0, loud: 1 })).not.toMatch(/agent|shell|session/)
  })

  it('says idle for a project with nothing going on, and for one nobody measured', () => {
    expect(stateLabel({ state: 'idle', live: 0, loud: 0 })).toBe('idle')
    expect(stateLabel(undefined)).toBe('idle')
  })
})

describe('projectSummary', () => {
  it('joins the branch and the state with a middle dot', () => {
    expect(projectSummary('develop', { state: 'live', live: 1, loud: 0 })).toBe(
      'develop · 1 running'
    )
  })

  it('drops the branch when there is none, rather than opening with a dot', () => {
    expect(projectSummary('', { state: 'idle', live: 0, loud: 0 })).toBe('idle')
    expect(projectSummary(undefined, undefined)).toBe('idle')
  })
})
