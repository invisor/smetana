import { describe, expect, it } from 'vitest'
import { scopeOf } from '../../../src/components/agent/batchScope.js'
import { statusColors } from '../../../src/components/status/status.js'

const ctx = {
  project: '/p',
  issueIds: ['smetana-th4', 'smetana-th4n', 'smetana-z46y'],
  appDir: '/Applications/smetana.app/Contents/Resources'
}

describe('scopeOf', () => {
  it('reads no cwd and the project root as root', () => {
    expect(scopeOf(null, ctx)).toMatchObject({ kind: 'root', label: 'root', colors: { border: 'var(--border)' } })
    expect(scopeOf('/p', ctx).kind).toBe('root')
    expect(scopeOf('/p/', ctx).kind).toBe('root')
  })

  it('names a worktree by the longest issue id that prefixes its folder', () => {
    const cwd = '/p/.worktrees/smetana-th4n-run-dialog-advanced/src'
    const scope = scopeOf(cwd, ctx)
    expect(scope).toMatchObject({ kind: 'worktree', label: 'th4n', task: 'smetana-th4n', title: cwd })
    const { fg, bg, border } = statusColors('smetana-th4n')
    expect(scope.colors).toEqual({ fg, bg, border })
  })

  it('matches a folder named exactly as an id', () => {
    expect(scopeOf('/p/.worktrees/smetana-z46y', ctx)).toMatchObject({ label: 'z46y', task: 'smetana-z46y' })
  })

  it('does not let an id match in the middle of a longer word', () => {
    expect(scopeOf('/p/.worktrees/smetana-th4nx-other', ctx)).toMatchObject({ label: 'smetana-th4nx-other', task: null })
  })

  it('keeps an unmatched worktree folder name in the muted set', () => {
    expect(scopeOf('/p/.worktrees/spike', ctx)).toMatchObject({
      kind: 'worktree',
      label: 'spike',
      task: null,
      colors: { fg: 'var(--text-muted)', border: 'var(--border-subtle)' }
    })
  })

  it('folds the temp directories to /tmp', () => {
    for (const cwd of ['/tmp/x', '/private/tmp/x', '/var/folders/ab/c', '/tmp']) {
      expect(scopeOf(cwd, ctx)).toMatchObject({ kind: 'tmp', label: '/tmp', title: cwd })
    }
  })

  it('reads a path under the app bundle as app', () => {
    expect(scopeOf(`${ctx.appDir}/smetana/skills`, ctx)).toMatchObject({ kind: 'app', label: 'app' })
  })

  it('names anything else by its last segment', () => {
    expect(scopeOf('/Users/x/other', ctx)).toMatchObject({ kind: 'other', label: 'other', title: '/Users/x/other' })
    expect(scopeOf('/p/.worktreesX/a', ctx).kind).toBe('other')
  })

  it('treats an empty app directory as matching nothing', () => {
    expect(scopeOf('/Users/x/other', { ...ctx, appDir: '' }).kind).toBe('other')
  })
})
