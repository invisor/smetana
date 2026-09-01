import { describe, expect, it } from 'vitest'
import {
  branchNameError,
  canCreate,
  canRename,
  renameError
} from '../../../src/components/git/branchName.js'

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

describe('renameError', () => {
  /* The whole reason this exists beside `branchNameError`: the branch being
     renamed is in the list it would be checked against, so the plain rule opens
     the window refusing the name it just filled the field with — a dialog
     calling a name taken by the person it is asking. */
  it('does not call the branch’s own name taken', () => {
    expect(renameError('develop', 'develop', BRANCHES)).toBe(null)
  })

  it('still refuses a name another branch holds', () => {
    expect(renameError('feat/login', 'develop', BRANCHES)).toBe(
      'A branch with this name already exists.'
    )
  })

  it('keeps every shape rule git documents', () => {
    expect(renameError('feat/log in', 'develop', BRANCHES)).toBe(
      'A branch name cannot contain spaces.'
    )
    expect(renameError('.hidden', 'develop', BRANCHES)).toBe(
      'No part of a branch name can start with a dot.'
    )
  })

  /* An emptied field is not a mistake, exactly as it is not one when a branch
     is being cut: `canRename` is what holds the button. */
  it('says nothing about an empty field', () => {
    expect(renameError('', 'develop', BRANCHES)).toBe(null)
    expect(renameError('   ', 'develop', BRANCHES)).toBe(null)
  })

  /* Trimmed, since that is the name that would be written — and trimmed before
     the comparison, so that the branch's own name padded with spaces is still
     its own name and draws no red line. */
  it('judges the trimmed name', () => {
    expect(renameError('  develop  ', 'develop', BRANCHES)).toBe(null)
    expect(renameError('  feat/login  ', 'develop', BRANCHES)).toBe(
      'A branch with this name already exists.'
    )
  })
})

describe('canRename', () => {
  const ready = { name: 'feat/login-2', from: 'develop', branches: BRANCHES }

  it('takes a name nothing is wrong with', () => {
    expect(canRename(ready)).toBe(true)
  })

  /* The unchanged name holds the button and says nothing under the field: there
     is nothing wrong with it and nothing to ask git for. */
  it('holds the button over the name the branch already has', () => {
    expect(canRename({ ...ready, name: 'develop' })).toBe(false)
    expect(canRename({ ...ready, name: '  develop  ' })).toBe(false)
    expect(renameError('develop', 'develop', BRANCHES)).toBe(null)
  })

  it('holds it over an empty field', () => {
    expect(canRename({ ...ready, name: '  ' })).toBe(false)
  })

  it('holds it over a name git would refuse', () => {
    expect(canRename({ ...ready, name: 'feat/log in' })).toBe(false)
  })

  it('holds it over a name another branch already holds', () => {
    expect(canRename({ ...ready, name: 'feat/login' })).toBe(false)
  })

  /* The window can be open when a run starts underneath it, so the verdict is
     read here and not only where the menu was opened. */
  it('holds it while a run holds the repository', () => {
    expect(canRename({ ...ready, allowed: false })).toBe(false)
  })

  it('holds it while git is already working', () => {
    expect(canRename({ ...ready, busy: true })).toBe(false)
  })

  /* A caller that went round the menu: with nothing to rename there is nothing
     to press, and the list would refuse the new name as taken by whichever
     branch happens to carry it. */
  it('holds it when there is no branch to rename', () => {
    expect(canRename({ ...ready, from: null })).toBe(false)
  })
})
