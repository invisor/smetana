import { describe, expect, it } from 'vitest'
import { repoLabel } from '../../../src/components/git/repoLabel.js'

describe('repoLabel', () => {
  /* The whole point of the module: the project root reaches the front end
     named `"."`, and for a project of a single repository that dot is the
     entire row. */
  it('draws the project root as the name of its folder', () => {
    expect(repoLabel({ name: '.', path: '/Users/you/dev/smetana' })).toBe('smetana')
  })

  it('leaves a name from the one-level walk alone', () => {
    expect(repoLabel({ name: 'admin', path: '/Users/you/dev/smetana/admin' })).toBe('admin')
  })

  /* A name somebody wrote in `[project].repos` is theirs, path-shaped or not —
     replacing it with the last segment would erase what they called it. */
  it('leaves a name from [project].repos alone, path-shaped or not', () => {
    expect(repoLabel({ name: '../shared', path: '/Users/you/dev/shared' })).toBe('../shared')
  })

  /* `basename` answers a root path with the path itself, which is what keeps
     this row from going blank for a project opened at `/`. */
  it('never draws an empty row for a project at a root path', () => {
    expect(repoLabel({ name: '.', path: '/' })).toBe('/')
  })

  /* A trailing separator is the shape a path arrives in often enough to be
     worth pinning: it must not cost the row its name. */
  it('ignores a trailing separator', () => {
    expect(repoLabel({ name: '.', path: '/Users/you/dev/smetana/' })).toBe('smetana')
  })

  /* WebView2 is among the target webviews, so the Windows form of a path has
     to name the same folder. */
  it('names a Windows path by its last segment', () => {
    expect(repoLabel({ name: '.', path: 'C:\\Users\\you\\dev\\smetana' })).toBe('smetana')
  })
})
