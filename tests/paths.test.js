import { describe, expect, it } from 'vitest'
import { basename, relativeTo } from '../src/paths.js'

describe('what a path is called', () => {
  it('is the last segment', () => {
    expect(basename('/Users/someone/Projects/smetana')).toBe('smetana')
    expect(basename('/Users/someone/Projects/smetana/src/App.vue')).toBe('App.vue')
  })

  it('a trailing separator changes nothing', () => {
    // Paths reach this from settings, from the OS dialog and from the run
    // worker, and only some of those normalise.
    expect(basename('/Users/someone/Projects/smetana/')).toBe('smetana')
    expect(basename('/Users/someone/Projects/smetana///')).toBe('smetana')
  })

  it('splits on the Windows separator too, since WebView2 is a target webview', () => {
    // Without this the whole path became the project's name on Windows.
    expect(basename('C:\\Users\\someone\\smetana')).toBe('smetana')
    expect(basename('C:\\Users\\someone\\smetana\\')).toBe('smetana')
  })

  it('a path with nothing left after the separators keeps its own name', () => {
    // A name is more use than an empty gap in a sentence: the run dialog's
    // tooltip interpolates this into "The run in X is driving the browser",
    // and a fourth copy of this rule answering '' here is what put an empty
    // gap on screen.
    expect(basename('/')).toBe('/')
    expect(basename('smetana')).toBe('smetana')
  })
})

describe('what a path is called from inside a folder', () => {
  it('is what is left after the root', () => {
    expect(relativeTo('/project', '/project/admin/src/main.rs')).toBe('admin/src/main.rs')
  })

  it('the root itself is the empty string, which is what files_list calls it', () => {
    expect(relativeTo('/project', '/project')).toBe('')
    expect(relativeTo('/project/', '/project')).toBe('')
  })

  it('a folder outside answers null rather than guessing', () => {
    // `[project].repos` may name anything at all, including `../shared`, and
    // `files_read` refuses everything outside the project root — so this is an
    // ordinary answer here and not a failure to paper over.
    expect(relativeTo('/project', '/elsewhere/admin/src/main.rs')).toBe(null)
    // A neighbour whose name merely starts with the root's is not inside it.
    expect(relativeTo('/project', '/project-two/src/main.rs')).toBe(null)
  })

  it('normalises the Windows separator, since the two halves arrive in different forms', () => {
    // Rust writes the platform's separator; everything relative in files.js is
    // written with `/`.
    expect(relativeTo('C:\\project', 'C:\\project\\admin/src/main.rs')).toBe('admin/src/main.rs')
  })

  it('nothing to compare against is null, not an accidental match', () => {
    expect(relativeTo(null, '/project/a.txt')).toBe(null)
    expect(relativeTo('/project', '')).toBe(null)
  })
})
