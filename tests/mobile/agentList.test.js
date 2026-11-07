import { describe, expect, it } from 'vitest'
import { conversationPresent } from '../../mobile/src/agentList.js'

describe('phone conversation presence', () => {
  it('keeps a Crew root open while its tree row exists and closes when it disappears', () => {
    const crew = [{ id: 'crew:41:41', state: 'needs-you' }, { id: 'crew:41:42', state: 'running' }]
    expect(conversationPresent('crew:41:41', true, [], crew)).toBe(true)
    expect(conversationPresent('crew:41:42', true, [], crew)).toBe(true)
    expect(conversationPresent('crew:41:41', true, [], [])).toBe(false)
  })

  it('still reads ordinary sessions by numeric id', () => {
    expect(conversationPresent(7, true, [{ id: 7 }], [])).toBe(true)
    expect(conversationPresent(7, true, [], [])).toBe(false)
    expect(conversationPresent(7, false, [{ id: 7 }], [])).toBe(false)
  })
})
