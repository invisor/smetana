import { describe, expect, it } from 'vitest'
import { nameAgentRows, withAgentName } from '../../../src/components/agent/agentName.js'

describe('a person’s own name for an agent row', () => {
  it('writes a trimmed name under the conversation id and returns a new map', () => {
    const before = { 'conv-a': 'Old' }
    const after = withAgentName(before, 'conv-b', '  Fix the build  ')
    expect(after).toEqual({ 'conv-a': 'Old', 'conv-b': 'Fix the build' })
    expect(before).toEqual({ 'conv-a': 'Old' })
  })

  it('removes the name on an empty value rather than keeping an empty entry', () => {
    expect(withAgentName({ 'conv-a': 'Old' }, 'conv-a', '   ')).toEqual({})
    expect(withAgentName({}, 'conv-a', '')).toEqual({})
  })

  it('refuses a row with nothing to remember it by', () => {
    const names = { 'conv-a': 'Old' }
    expect(withAgentName(names, null, 'x')).toBe(names)
  })

  it('overlays the name on the label and leaves the ids and the title alone', () => {
    const rows = [
      { id: 'conversation:1', conversation: 'conv-a', label: 'Auto title', title: 'Auto title', tasks: ['x-1'] },
      { id: 'conversation:2', conversation: null, label: 'Agent', tasks: [] },
      { id: 7, conversation: 'conv-r', label: null, tasks: ['a-1', 'a-2'] }
    ]
    const named = nameAgentRows(rows, { 'conv-a': 'Mine', 'conv-r': 'The batch' })
    expect(named[0]).toEqual({ ...rows[0], label: 'Mine' })
    expect(named[1]).toBe(rows[1])
    expect(named[2]).toEqual({ ...rows[2], label: 'The batch' })
  })
})
