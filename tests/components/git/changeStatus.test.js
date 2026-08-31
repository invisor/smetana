import { describe, expect, it } from 'vitest'
import { changeStatus } from '../../../src/components/git/changeStatus.js'

describe('what a changed file is captioned with', () => {
  it('gives each kind its own mark and its own word', () => {
    expect(changeStatus('modified').letter).toBe('M')
    expect(changeStatus('untracked').letter).toBe('U')
    expect(changeStatus('deleted').letter).toBe('D')
    expect(changeStatus('renamed').letter).toBe('R')
  })

  /* The conflict is the one row marked with something other than a letter, and
     it is the tree's own `!` (`files/FileTreeRow.vue`). The word stays: a bare
     `!` reads as nothing at all to a screen reader. */
  it('marks a conflict the way the file tree does, and still names it', () => {
    expect(changeStatus('conflicted').letter).toBe('!')
    expect(changeStatus('conflicted').label).toBe('Conflicted')
  })

  it('captions in sentence case, the way the rest of the app does', () => {
    expect(changeStatus('typeChanged').label).toBe('Type changed')
  })

  /* A kind this file has never heard of is an ordinary outcome, not an error:
     git may grow one, and a row with no letter beats a panel that throws. */
  it('a kind nobody has heard of still draws a row', () => {
    const unknown = changeStatus('something-new')
    expect(unknown.letter).toBe('?')
    expect(unknown.label).toBe('Changed')
  })

  /* The file tree already draws a modified file's mark in `--git-modified`
     (`files/FileTreeRow.vue`), and one file has to look the same in both
     places. A token name rather than a colour, so the browser repaints both on
     a theme change with nothing here to keep in step. */
  it('names a token and never a colour', () => {
    expect(changeStatus('modified').token).toBe('--git-modified')
    expect(changeStatus('conflicted').token).toBe('--git-conflict')
    for (const kind of ['modified', 'added', 'deleted', 'renamed', 'untracked', 'conflicted', 'nonsense']) {
      expect(changeStatus(kind).token).toMatch(/^--[a-z-]+$/)
    }
  })
})
