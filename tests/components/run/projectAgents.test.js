import { describe, expect, it } from 'vitest'
import {
  canSaveProject,
  OWN_AGENTS_DESCRIPTION,
  OWN_AGENTS_LABEL,
  sameAgentTable,
  seedFromGlobal
} from '../../../src/components/run/projectAgents.js'

const emptyRoles = () => ({
  tasks: { agent: '', model: '' },
  code: { agent: '', model: '' },
  runLead: { agent: '', model: '' },
  reviewBranch: { agent: '', model: '' }
})

describe('the switch', () => {
  it('has words', () => {
    expect(OWN_AGENTS_LABEL).toBeTruthy()
    expect(OWN_AGENTS_DESCRIPTION).toBeTruthy()
  })
})

describe('seedFromGlobal', () => {
  it('copies the global table with all four roles written out', () => {
    const global = {
      agent: 'claude',
      model: 'opus',
      agentRoles: { tasks: { agent: 'codex', model: 'gpt-5.6-sol' } }
    }
    expect(seedFromGlobal(global)).toEqual({
      agent: 'claude',
      model: 'opus',
      agentRoles: {
        tasks: { agent: 'codex', model: 'gpt-5.6-sol' },
        code: { agent: '', model: '' },
        runLead: { agent: '', model: '' },
        reviewBranch: { agent: '', model: '' }
      }
    })
  })

  /* A copy, so a form writing through into the table it was seeded from would
     make every later dialog open on the last one's numbers. */
  it('is a copy, not a reference to the table it was seeded from', () => {
    const global = { agent: 'claude', model: '', agentRoles: emptyRoles() }
    const seeded = seedFromGlobal(global)
    seeded.agent = 'codex'
    seeded.agentRoles.tasks.agent = 'codex'
    expect(global.agent).toBe('claude')
    expect(global.agentRoles.tasks.agent).toBe('')
  })

  it('reads a table with nothing on it as every role inheriting', () => {
    expect(seedFromGlobal(undefined)).toEqual({ agent: '', model: '', agentRoles: emptyRoles() })
  })
})

describe('sameAgentTable', () => {
  it('two nulls are the same table', () => {
    expect(sameAgentTable(null, null)).toBe(true)
  })

  it('null and a table are never the same table', () => {
    const table = seedFromGlobal({ agent: 'claude', model: '' })
    expect(sameAgentTable(null, table)).toBe(false)
    expect(sameAgentTable(table, null)).toBe(false)
  })

  it('compares by value, not by reference', () => {
    const a = seedFromGlobal({ agent: 'codex', model: 'gpt-5.6-sol' })
    const b = seedFromGlobal({ agent: 'codex', model: 'gpt-5.6-sol' })
    expect(a).not.toBe(b)
    expect(sameAgentTable(a, b)).toBe(true)
  })

  it('a changed role is a different table', () => {
    const a = seedFromGlobal({ agent: 'claude', model: '' })
    const b = seedFromGlobal({
      agent: 'claude',
      model: '',
      agentRoles: { tasks: { agent: 'codex', model: '' } }
    })
    expect(sameAgentTable(a, b)).toBe(false)
  })
})

describe('canSaveProject', () => {
  it('nothing changed → false', () => {
    expect(
      canSaveProject({
        busy: false,
        fields: true,
        defaultsDirty: false,
        defaultsValid: true,
        agentsDirty: false
      })
    ).toBe(false)
  })

  it('the block changed with no file to offer defaults from → true', () => {
    expect(
      canSaveProject({
        busy: false,
        fields: false,
        defaultsDirty: false,
        defaultsValid: false,
        agentsDirty: true
      })
    ).toBe(true)
  })

  it('defaults changed, over a parsed and valid file → true', () => {
    expect(
      canSaveProject({
        busy: false,
        fields: true,
        defaultsDirty: true,
        defaultsValid: true,
        agentsDirty: false
      })
    ).toBe(true)
  })

  it('invalid defaults hold everything, agents changed or not', () => {
    expect(
      canSaveProject({
        busy: false,
        fields: true,
        defaultsDirty: true,
        defaultsValid: false,
        agentsDirty: false
      })
    ).toBe(false)
    expect(
      canSaveProject({
        busy: false,
        fields: true,
        defaultsDirty: false,
        defaultsValid: false,
        agentsDirty: true
      })
    ).toBe(false)
  })

  it('busy holds everything, whatever else changed', () => {
    expect(
      canSaveProject({
        busy: true,
        fields: true,
        defaultsDirty: true,
        defaultsValid: true,
        agentsDirty: true
      })
    ).toBe(false)
  })
})
