import { describe, expect, it } from 'vitest'
import { agentsLabel, dirtyLabel } from '../../../src/components/shell/statusCounters.js'

describe('dirtyLabel', () => {
  it('says file in the singular', () => {
    expect(dirtyLabel(1)).toBe('1 uncommitted file')
  })

  it('says files for anything else', () => {
    expect(dirtyLabel(2)).toBe('2 uncommitted files')
    expect(dirtyLabel(12)).toBe('12 uncommitted files')
  })

  /* The strip never draws the counter at zero, so this is what the rule
     answers rather than what anybody sees — and a plural noun is still the
     right English for it. */
  it('says files for a count of zero', () => {
    expect(dirtyLabel(0)).toBe('0 uncommitted files')
  })
})

describe('agentsLabel', () => {
  it('says agent in the singular', () => {
    expect(agentsLabel(1)).toBe('1 agent running')
  })

  it('says agents for anything else', () => {
    expect(agentsLabel(2)).toBe('2 agents running')
    expect(agentsLabel(3)).toBe('3 agents running')
  })

  it('says agents for a count of zero', () => {
    expect(agentsLabel(0)).toBe('0 agents running')
  })
})
