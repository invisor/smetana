import { describe, expect, it } from 'vitest'
import { canFile, newTaskIntent } from '../src/newTask.js'

describe('filing a task from the phone', () => {
  it('has nothing to file without words', () => {
    expect(canFile('')).toBe(false)
    expect(canFile('  \n\t')).toBe(false)
    expect(canFile(undefined)).toBe(false)
    expect(canFile('Fix the thing')).toBe(true)
  })

  it('starts the agent with the intent the desktop\'s New task window sends', () => {
    expect(newTaskIntent('  Fix the thing  ')).toEqual({
      kind: 'newTask',
      brainstorm: 'auto',
      spec: 'auto',
      plan: 'auto',
      draft: { text: 'Fix the thing', issue_type: null, priority: null, images: [], parent: null }
    })
  })

  it('names no images and no parent, which a phone cannot', () => {
    const { draft } = newTaskIntent('x')
    expect(draft.images).toEqual([])
    expect(draft.parent).toBeNull()
  })
})
