import { describe, expect, it } from 'vitest'
import {
  isAbsoluteLinkPath,
  resolveLocalLinkPath
} from '../../../src/components/conversation/localLinkTarget.js'

describe('isAbsoluteLinkPath', () => {
  it('is true for a leading /, the only shape classifyLink can hand this', () => {
    expect(isAbsoluteLinkPath('/etc/hosts')).toBe(true)
    expect(isAbsoluteLinkPath('/Users/flexo/Downloads/x.log')).toBe(true)
  })

  it('is false for a project-relative path, which classifyLink hands over unchanged', () => {
    expect(isAbsoluteLinkPath('src/App.vue')).toBe(false)
    expect(isAbsoluteLinkPath('README.md')).toBe(false)
  })

  it('is false for anything that is not a string, rather than throwing', () => {
    expect(isAbsoluteLinkPath(undefined)).toBe(false)
    expect(isAbsoluteLinkPath(null)).toBe(false)
    expect(isAbsoluteLinkPath('')).toBe(false)
  })
})

describe('resolveLocalLinkPath', () => {
  it('leaves a relative path exactly as classifyLink built it', () => {
    expect(resolveLocalLinkPath('/project', 'src/App.vue')).toBe('src/App.vue')
    // Even with no project open at all — a relative target never asks the root anything.
    expect(resolveLocalLinkPath(null, 'src/App.vue')).toBe('src/App.vue')
  })

  it('translates an absolute path inside the project to the same relative one', () => {
    expect(resolveLocalLinkPath('/project', '/project/src/App.vue')).toBe('src/App.vue')
  })

  it('translates an absolute path naming the project root itself to the empty string', () => {
    expect(resolveLocalLinkPath('/project', '/project')).toBe('')
  })

  it('refuses an absolute path outside the project rather than guessing', () => {
    expect(resolveLocalLinkPath('/project', '/etc/hosts')).toBe(null)
    expect(resolveLocalLinkPath('/project', '/Users/flexo/Downloads/x.log')).toBe(null)
    // A neighbour whose name merely starts with the root's is not inside it.
    expect(resolveLocalLinkPath('/project', '/project-two/src/App.vue')).toBe(null)
  })

  it('refuses an absolute path when there is no project open to resolve it against', () => {
    expect(resolveLocalLinkPath(null, '/etc/hosts')).toBe(null)
  })
})
