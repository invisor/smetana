import { describe, expect, it } from 'vitest'
import { absolutePath, basename, dirname, relativeTo } from '../src/paths.js'

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

describe('what folder a path sits in', () => {
  it('is everything above the last segment', () => {
    expect(dirname('/Users/someone/Projects/smetana')).toBe('/Users/someone/Projects')
    expect(dirname('/Users/someone/Projects/smetana/src/App.vue')).toBe(
      '/Users/someone/Projects/smetana/src'
    )
  })

  it('a trailing separator changes nothing', () => {
    // The OS file dialog hands back one form and settings another.
    expect(dirname('/Users/someone/Projects/smetana/')).toBe('/Users/someone/Projects')
    expect(dirname('/Users/someone/Projects/smetana///')).toBe('/Users/someone/Projects')
  })

  it('splits on the Windows separator too, since WebView2 is a target webview', () => {
    expect(dirname('C:\\Users\\someone\\smetana')).toBe('C:\\Users\\someone')
    expect(dirname('C:\\Users\\someone\\smetana\\')).toBe('C:\\Users\\someone')
  })

  it('a folder directly under a root keeps the root, which is not the empty string', () => {
    // '' as a defaultPath is not the root: the option would be there and name
    // nowhere, where the folder above /smetana is /.
    expect(dirname('/smetana')).toBe('/')
    expect(dirname('C:\\smetana')).toBe('C:\\')
  })

  it('a root and a bare name have no folder above them, and say so', () => {
    // `null` rather than a guess: the caller opens its dialog with no
    // `defaultPath` at all, which is what the panel did before this existed.
    expect(dirname('/')).toBe(null)
    expect(dirname('///')).toBe(null)
    expect(dirname('smetana')).toBe(null)
  })

  it('nothing at all is null rather than a throw', () => {
    // `settings.activeProject` is null with no project open, and the folder
    // picker asks this before it asks anything else.
    expect(dirname('')).toBe(null)
    expect(dirname(null)).toBe(null)
    expect(dirname(undefined)).toBe(null)
  })
})

/* The other direction. It lived in `components/files/fileMenu.js` while the
   tree's menu was the only caller; `stores/files.js` wants it too now, for the
   system clipboard, so the rule moved up here and the menu re-exports it under
   the name it always had. */
describe('absolutePath', () => {
  it('joins the project root and the tree path', () => {
    expect(absolutePath('/Users/you/dev/app', 'src/main.rs')).toBe('/Users/you/dev/app/src/main.rs')
  })

  it('is the root itself for the root', () => {
    expect(absolutePath('/Users/you/dev/app', '')).toBe('/Users/you/dev/app')
  })

  it('does not double a separator the root already ends in', () => {
    expect(absolutePath('/Users/you/dev/app/', 'src')).toBe('/Users/you/dev/app/src')
  })

  it('writes a Windows path in one separator rather than two', () => {
    // Everything relative in stores/files.js is written with "/" whatever the
    // platform, and the root arrives from Rust in the platform's own form.
    expect(absolutePath('C:\\Users\\you\\app', 'src/main.rs')).toBe('C:\\Users\\you\\app\\src\\main.rs')
  })

  it('keeps a forward slash for a root that has one, whatever else it holds', () => {
    expect(absolutePath('/Users/you/a\\b', 'src')).toBe('/Users/you/a\\b/src')
  })

  it('is the path alone when there is no project to hang it off', () => {
    expect(absolutePath(null, 'src/main.rs')).toBe('src/main.rs')
  })
})
