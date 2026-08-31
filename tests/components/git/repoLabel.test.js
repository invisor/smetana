import { describe, expect, it } from 'vitest'
import { repoLabel, repoPath } from '../../../src/components/git/repoLabel.js'

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

describe('repoPath', () => {
  const ROOT = '/Users/you/dev/smetana'
  const HOME = '/Users/you'

  /* The case the review window has today: a project of one repository, whose
     name reaches the front end as `.` and whose path is the project itself. */
  it('draws the project root as ./', () => {
    expect(repoPath(ROOT, ROOT)).toBe('./')
  })

  it('draws a repository inside the project under ./', () => {
    expect(repoPath(ROOT, `${ROOT}/services/backend`)).toBe('./services/backend')
  })

  /* `[project].repos` takes entries like `../shared`, so a repository outside
     the project is an ordinary arrangement rather than a broken one. */
  it('draws a repository under the home folder with a tilde', () => {
    expect(repoPath(ROOT, '/Users/you/work/smetana-infra', HOME)).toBe('~/work/smetana-infra')
  })

  it('draws the home folder itself as a bare tilde', () => {
    expect(repoPath(ROOT, HOME, HOME)).toBe('~')
  })

  it('draws a repository outside both as its absolute path', () => {
    expect(repoPath(ROOT, '/opt/vendor/shared', HOME)).toBe('/opt/vendor/shared')
  })

  /* The home folder is an argument because nothing in this app knows one yet,
     so the rule has to be useful without it. */
  it('falls back to the absolute path when no home folder is given', () => {
    expect(repoPath(ROOT, '/Users/you/work/smetana-infra')).toBe('/Users/you/work/smetana-infra')
  })

  /* The project wins over the home folder for anything inside it, which is the
     usual arrangement: a project normally sits under the home folder, and
     `~/dev/smetana/services/backend` would say less than `./services/backend`. */
  it('prefers the project over the home folder for a repository inside both', () => {
    expect(repoPath(ROOT, `${ROOT}/admin`, HOME)).toBe('./admin')
  })

  /* A path is not inside a folder because its string starts with it: a sibling
     whose name merely begins the same way is somewhere else entirely. */
  it('does not read a sibling folder as being inside the project', () => {
    expect(repoPath(ROOT, '/Users/you/dev/smetana-infra', HOME)).toBe('~/dev/smetana-infra')
  })

  /* A trailing separator is the shape a path arrives in often enough to be
     worth pinning on both sides of the comparison. */
  it('ignores a trailing separator on either path', () => {
    expect(repoPath(`${ROOT}/`, ROOT)).toBe('./')
    expect(repoPath(ROOT, `${ROOT}/services/backend/`)).toBe('./services/backend')
  })

  /* WebView2 is among the target webviews, so the Windows form of a path has to
     answer the same question — and the path drawn under `./` is written with
     the separator a reader of this column expects. */
  it('answers a Windows path, and draws the relative half with slashes', () => {
    expect(repoPath('C:\\Users\\you\\dev\\smetana', 'C:\\Users\\you\\dev\\smetana')).toBe('./')
    expect(
      repoPath('C:\\Users\\you\\dev\\smetana', 'C:\\Users\\you\\dev\\smetana\\services\\backend')
    ).toBe('./services/backend')
  })

  it('keeps the platform form of a path it did not compose', () => {
    expect(repoPath('C:\\Users\\you\\dev\\smetana', 'D:\\vendor\\shared')).toBe('D:\\vendor\\shared')
  })

  /* No project is open, or a repository arrived without a path: neither is an
     error worth throwing over, and neither may produce a row reading
     `./undefined`. */
  it('survives an empty root and an empty path', () => {
    expect(repoPath('', '/Users/you/dev/smetana')).toBe('/Users/you/dev/smetana')
    expect(repoPath('', '/Users/you/dev/smetana', HOME)).toBe('~/dev/smetana')
    expect(repoPath(ROOT, '')).toBe('')
    expect(repoPath('', '')).toBe('')
    expect(repoPath(undefined, undefined)).toBe('')
  })

  /* A project opened at a root path: the root is still the root, and a
     repository under it is still inside it. */
  it('answers for a project at a root path', () => {
    expect(repoPath('/', '/')).toBe('./')
    expect(repoPath('/', '/srv/app')).toBe('./srv/app')
  })
})
