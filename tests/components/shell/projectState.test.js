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

  /* The absence of the noun is what this pins, and it outlived the reason it
     was written for: the map behind these counts could not tell an agent from a
     person's own shell, and it can now (smetana-low). The words are unchanged
     until somebody decides otherwise, which is what the module's own note says,
     so a change to them is a decision rather than a repair and this is where it
     comes to be taken.

     The guard is that one word and no wider. "1 session waiting on you" would
     be perfectly true and is house vocabulary already (`AgentSettings.vue`,
     `AboutSettings.vue`), so a guard that tripped on it would be teaching the
     next person to edit this test instead of reading it. */
  it('names no agent, which is a decision nobody has revisited', () => {
    expect(stateLabel({ state: 'live', live: 1, loud: 0 })).not.toMatch(/\bagents?\b/)
    expect(stateLabel({ state: 'loud', live: 0, loud: 1 })).not.toMatch(/\bagents?\b/)
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
