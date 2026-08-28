import { describe, expect, it } from 'vitest'
import {
  DIALOG_KINDS,
  dialogGround,
  dialogWidth,
  isDialogKind,
  staleDialogs,
  stalenessMessage,
  stalenessOf
} from '../../src/views/dialogRegistry.js'

describe('the dialog registry', () => {
  it('holds the eight kinds that became windows', () => {
    expect([...DIALOG_KINDS].sort()).toEqual([
      'delete-session',
      'delete-task',
      'new-branch',
      'new-task',
      'promote-column',
      'ready-task',
      'run',
      'setup-project'
    ])
  })

  it('refuses a name it has never heard of', () => {
    expect(isDialogKind('conflict')).toBe(false)
    expect(isDialogKind('new-branch')).toBe(true)
  })

  it('gives every kind a width', () => {
    for (const kind of DIALOG_KINDS) expect(dialogWidth(kind)).toBeGreaterThan(0)
  })

  it('stands every kind on the project, whatever else it stands on', () => {
    for (const kind of DIALOG_KINDS) expect(dialogGround(kind)).toContain('project')
  })
})

describe('ground that has gone', () => {
  const world = {
    project: '/repo',
    repo: '/repo/app',
    issues: new Set(['smetana-1']),
    columns: new Set(['deferred']),
    branches: new Set(['main'])
  }

  it('leaves a dialog standing on ground that is still there', () => {
    const open = [{ kind: 'delete-task', ground: { project: '/repo', issue: 'smetana-1' } }]
    expect(staleDialogs(open, world)).toEqual([])
  })

  it('closes a dialog whose issue is gone', () => {
    const open = [{ kind: 'delete-task', ground: { project: '/repo', issue: 'smetana-9' } }]
    expect(staleDialogs(open, world)).toEqual(['delete-task'])
  })

  it('closes everything when the project changes', () => {
    const open = [
      { kind: 'delete-task', ground: { project: '/other', issue: 'smetana-1' } },
      { kind: 'run', ground: { project: '/other' } }
    ]
    expect(staleDialogs(open, world).sort()).toEqual(['delete-task', 'run'])
  })

  it('closes a dialog whose column is gone', () => {
    const open = [{ kind: 'promote-column', ground: { project: '/repo', column: 'archived' } }]
    expect(staleDialogs(open, world)).toEqual(['promote-column'])
  })

  it('closes a dialog whose starting branch is gone', () => {
    const open = [
      { kind: 'new-branch', ground: { project: '/repo', repo: '/repo/app', branch: 'gone' } }
    ]
    expect(staleDialogs(open, world)).toEqual(['new-branch'])
  })

  /* The repository is equality against the selected one and not membership of
     the project's list: `main` exists in both repositories of a project, so a
     dialog left standing over the one somebody clicked away from would cut its
     branch in the other with every name check passing. */
  it('closes a dialog whose repository is no longer the selected one', () => {
    const open = [
      { kind: 'new-branch', ground: { project: '/repo', repo: '/repo/docs', branch: 'main' } }
    ]
    expect(staleDialogs(open, world)).toEqual(['new-branch'])
    expect(stalenessOf('new-branch', open[0].ground, world)).toBe('repo')
  })

  /* A kind whose ground names nothing beyond the project is not stale because
     the board happens to have no column of that name: the field is absent, not
     empty, and reading an absence as a mismatch would close a run window every
     time somebody looked at it. */
  it('ignores a sort of ground a dialog does not stand on', () => {
    const open = [{ kind: 'run', ground: { project: '/repo' } }]
    expect(staleDialogs(open, world)).toEqual([])
  })

  it('names the reason rather than only the verdict', () => {
    const standing = { project: '/repo', repo: '/repo/app', branch: 'main' }
    expect(stalenessOf('new-branch', standing, world)).toBe(null)
    expect(stalenessOf('new-branch', { ...standing, branch: 'gone' }, world)).toBe('branch')
    expect(stalenessOf('new-branch', { ...standing, project: '/other' }, world)).toBe('project')
  })

  it('says why, naming the thing that went', () => {
    expect(stalenessMessage('delete-task', 'issue')).toBe(
      'The delete dialog closed: the task it was about no longer exists.'
    )
    expect(stalenessMessage('run', 'project')).toBe('The run dialog closed: the project changed.')
    expect(stalenessMessage('new-branch', 'branch')).toBe(
      'The new branch dialog closed: the branch it started from is gone.'
    )
    expect(stalenessMessage('new-branch', 'repo')).toBe(
      'The new branch dialog closed: the Git panel moved to another repository.'
    )
  })
})
