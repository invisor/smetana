import { describe, expect, it } from 'vitest'
import { attachmentAction } from '../../../src/components/conversation/attachmentAction.js'

describe('what a click on an attachment chip does', () => {
  it('a picture always opens in the image window, even outside the project', () => {
    expect(attachmentAction('/Users/you/Desktop/screenshot.png', '/Users/you/project')).toEqual({
      kind: 'image'
    })
  })

  it('a picture opens even with no project open at all', () => {
    expect(attachmentAction('/Users/you/Desktop/screenshot.png', '')).toEqual({ kind: 'image' })
  })

  it('a non-picture inside the project opens in an editor tab, at its project-relative path', () => {
    expect(attachmentAction('/Users/you/project/src/main.rs', '/Users/you/project')).toEqual({
      kind: 'file',
      path: 'src/main.rs'
    })
  })

  it('a non-picture outside the project has no channel to open it through', () => {
    expect(attachmentAction('/tmp/worktree.log', '/Users/you/project')).toBe(null)
  })

  it('a non-picture is not openable with no project open at all', () => {
    expect(attachmentAction('/tmp/worktree.log', '')).toBe(null)
  })
})
