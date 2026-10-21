import { describe, expect, it } from 'vitest'
import { canAttach, canFile, canStart, newTaskIntent } from '../src/newTask.js'

describe('filing a task from the phone', () => {
  it('has nothing to file without words', () => {
    expect(canFile('')).toBe(false)
    expect(canFile('  \n\t')).toBe(false)
    expect(canFile(undefined)).toBe(false)
    expect(canFile('Fix the thing')).toBe(true)
  })

  it('starts the agent with the intent the desktop\'s New task window sends', () => {
    expect(newTaskIntent({ text: '  Fix the thing  ' })).toEqual({
      kind: 'newTask',
      brainstorm: 'auto',
      spec: 'auto',
      plan: 'auto',
      draft: { text: 'Fix the thing', issue_type: null, priority: null, images: [], files: [], parent: null }
    })
  })

  it('sends Auto as absence and a chosen type and priority as bd knows them', () => {
    const { draft } = newTaskIntent({
      text: 'x',
      issueType: 'bug',
      priority: '2',
      brainstorm: 'auto',
      spec: 'auto',
      plan: 'auto',
      attachments: [{ path: '/a.png', url: 'data:image/png;base64,AA==' }]
    })
    expect(draft).toEqual({ text: 'x', issue_type: 'bug', priority: 2, images: ['/a.png'], files: [], parent: null })
    expect(newTaskIntent({ text: 'x', issueType: 'auto', priority: 'auto' }).draft).toMatchObject({
      issue_type: null,
      priority: null
    })
    expect(newTaskIntent({ text: 'x', priority: '0' }).draft.priority).toBe(0)
  })

  it('sends what the cascade shows, not what was remembered', () => {
    const intent = newTaskIntent({ text: 'x', brainstorm: 'off', spec: 'on', plan: 'on', attachments: [] })
    expect([intent.brainstorm, intent.spec, intent.plan]).toEqual(['off', 'off', 'off'])
    const open = newTaskIntent({ text: 'x', brainstorm: 'on', spec: 'on', plan: 'off' })
    expect([open.brainstorm, open.spec, open.plan]).toEqual(['on', 'on', 'off'])
  })

  it('splits what was attached into pictures and files by whether a thumbnail exists', () => {
    const { draft } = newTaskIntent({
      text: 'x',
      attachments: [
        { path: '/s/a.png', url: 'data:image/png;base64,AA==' },
        { path: '/s/b.heic', url: null },
        { path: '/s/c.jpg', url: 'data:image/jpeg;base64,AA==' }
      ]
    })
    expect(draft.images).toEqual(['/s/a.png', '/s/c.jpg'])
    expect(draft.files).toEqual(['/s/b.heic'])
  })

  it('names no parent, which a phone cannot', () => {
    expect(newTaskIntent({ text: 'x' }).draft.parent).toBeNull()
  })

  it('offers pictures only to a desktop at protocol 1.3 or later', () => {
    expect(canAttach({ state: 'online', protocol_minor: 3 })).toBe(true)
    expect(canAttach({ state: 'update-recommended', protocol_minor: 4 })).toBe(true)
    expect(canAttach({ state: 'online', protocol_minor: 2 })).toBe(false)
    expect(canAttach({ state: 'online' })).toBe(false)
    expect(canAttach({ state: 'unreachable', protocol_minor: 3 })).toBe(false)
    expect(canAttach(null)).toBe(false)
  })

  it('cannot be started while words are missing, a start is running or a picture is uploading', () => {
    expect(canStart({ text: 'x' })).toBe(true)
    expect(canStart({ text: ' ' })).toBe(false)
    expect(canStart({ text: 'x', busy: true })).toBe(false)
    expect(canStart({ text: 'x', attaching: true })).toBe(false)
  })
})
