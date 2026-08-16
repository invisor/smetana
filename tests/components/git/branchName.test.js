import { describe, expect, it } from 'vitest'
import { branchNameError, canCreate } from '../../../src/components/git/branchName.js'

const BRANCHES = [{ name: 'develop', current: true }, { name: 'feat/login', current: false }]

describe('branchNameError', () => {
  it('takes an ordinary name', () => {
    expect(branchNameError('feat/worktree-rename', BRANCHES)).toBe(null)
  })

  /* Nothing typed yet is not a mistake. The button is held by `canCreate`. */
  it('says nothing about an empty field', () => {
    expect(branchNameError('', BRANCHES)).toBe(null)
    expect(branchNameError('   ', BRANCHES)).toBe(null)
    expect(branchNameError(null, BRANCHES)).toBe(null)
  })

  it('names the character that is wrong rather than that one is', () => {
    expect(branchNameError('feat/log in')).toBe('A branch name cannot contain spaces.')
    expect(branchNameError('feat/log~in')).toBe(
      'A branch name cannot contain ~ ^ : ? * [ or a backslash.'
    )
    expect(branchNameError('feat/../login')).toBe('A branch name cannot contain two dots in a row.')
    expect(branchNameError('feat@{1}')).toBe('A branch name cannot contain @{.')
  })

  it('refuses the shapes git refuses at the edges', () => {
    expect(branchNameError('-feat')).toBe('A branch name cannot start with a dash.')
    expect(branchNameError('/feat')).toBe('A branch name cannot start or end with a slash.')
    expect(branchNameError('feat/')).toBe('A branch name cannot start or end with a slash.')
    expect(branchNameError('feat//login')).toBe('A branch name cannot hold two slashes in a row.')
    expect(branchNameError('@')).toBe('A branch cannot be called @ on its own.')
  })

  it('refuses git’s dot rules, including inside a path component', () => {
    expect(branchNameError('.hidden')).toBe('No part of a branch name can start with a dot.')
    expect(branchNameError('feat/.wip')).toBe('No part of a branch name can start with a dot.')
    expect(branchNameError('feat.')).toBe('A branch name cannot end with a dot.')
    expect(branchNameError('feat.lock')).toBe('A branch name cannot end with .lock.')
  })

  it('refuses a name the repository already holds', () => {
    expect(branchNameError('develop', BRANCHES)).toBe('A branch with this name already exists.')
  })

  /* Exactly, and deliberately: git compares exactly, and refusing a name git
     would take is the direction this rule may not be wrong in. */
  it('compares that name exactly', () => {
    expect(branchNameError('Develop', BRANCHES)).toBe(null)
  })

  it('judges the trimmed name, since that is what would be created', () => {
    expect(branchNameError('  develop  ', BRANCHES)).toBe('A branch with this name already exists.')
  })
})

describe('canCreate', () => {
  const ready = { name: 'feat/login-2', branches: BRANCHES, allowed: true, busy: false }

  it('takes a name nothing is wrong with', () => {
    expect(canCreate(ready)).toBe(true)
  })

  it('holds the button over an empty field', () => {
    expect(canCreate({ ...ready, name: '  ' })).toBe(false)
  })

  it('holds it over a name git would refuse', () => {
    expect(canCreate({ ...ready, name: 'feat/log in' })).toBe(false)
  })

  /* The dialog can be open when a run starts underneath it, so the verdict is
     read here and not only where the menu was opened. */
  it('holds it while a run holds the repository', () => {
    expect(canCreate({ ...ready, allowed: false })).toBe(false)
  })

  it('holds it while git is already working', () => {
    expect(canCreate({ ...ready, busy: true })).toBe(false)
  })
})
