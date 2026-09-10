import { describe, expect, it } from 'vitest'
import { absolutePath, ancestors, basename, dirname, isUnder, relativeTo } from '../src/paths.js'

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

describe('every folder a path sits under', () => {
  it('is the folders above it, from the root down', () => {
    // Root first, which is the order the caller walks them in. It is not a
    // safety property — the reads it issues are not awaited one at a time — and
    // the order is pinned here because the answer is a list and a list has one.
    expect(ancestors('a/b/c.txt')).toEqual(['a', 'a/b'])
    expect(ancestors('docs/reviews-pr/review-NXC-239.html')).toEqual([
      'docs',
      'docs/reviews-pr'
    ])
  })

  it('a name with no folder above it has none, and so does the root', () => {
    // An empty answer is the ordinary case for a file at the top of a project,
    // not a failure: the root is expanded by construction.
    expect(ancestors('README.md')).toEqual([])
    expect(ancestors('')).toEqual([])
    expect(ancestors(null)).toEqual([])
    expect(ancestors(undefined)).toEqual([])
  })

  it('a trailing separator gives no empty ancestor', () => {
    // `''` is the root in this path space, and a row nothing draws: pushed into
    // `project.expanded` it would be an entry in settings.json naming the
    // project itself.
    expect(ancestors('a/b/')).toEqual(['a'])
    expect(ancestors('a/b//')).toEqual(['a'])
    expect(ancestors('a/')).toEqual([])
  })

  it('a double separator inside the path is not a folder either', () => {
    expect(ancestors('a//b/c.txt')).toEqual(['a', 'a/b'])
  })

  it('one separator only, deliberately, as in isUnder', () => {
    // Every path here is the tree's own: relative to the project and written
    // with `/` whatever the platform. A backslash is an ordinary character in a
    // name on macOS and Linux and must not cut one into folders.
    expect(ancestors('a\\b.txt')).toEqual([])
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

/* One line lifted out of four callers: the delete and the move in
   DesktopApp.vue, the paste refusal in fileClipboard.js and the focus sweep's
   fold-away in stores/files.js. Two of the four are in a .vue file nothing here
   can reach, which is the whole reason the rule is pinned at this end. */
describe('isUnder', () => {
  it('a folder holds itself and everything below it', () => {
    expect(isUnder('src', 'src')).toBe(true)
    expect(isUnder('src', 'src/stores')).toBe(true)
    expect(isUnder('src', 'src/stores/files.js')).toBe(true)
  })

  it('the separator is the whole of the rule: a sibling with a longer name is not inside', () => {
    // Without the trailing slash a delete of `src` would close the tabs of
    // `src-tauri`, a folder nobody touched.
    expect(isUnder('src', 'src-tauri')).toBe(false)
    expect(isUnder('src', 'src-tauri/src/main.rs')).toBe(false)
  })

  it('neither a parent nor an unrelated path is inside', () => {
    expect(isUnder('src/stores', 'src')).toBe(false)
    expect(isUnder('src', 'docs/readme.md')).toBe(false)
    expect(isUnder('src', '')).toBe(false)
  })

  it('the folder comes first, and the two orders are not the same question', () => {
    expect(isUnder('src', 'src/stores')).toBe(true)
    expect(isUnder('src/stores', 'src')).toBe(false)
  })

  it('one separator only, deliberately', () => {
    // Every caller compares paths in the tree's own space, where files.js
    // writes `/` on every platform. A backslash is an ordinary character in a
    // name on macOS and Linux, and it must not divide one.
    expect(isUnder('a', 'a\\b')).toBe(false)
  })

  it('the same paths spelled absolutely answer alike', () => {
    // The paste refusal compares a system clipboard path against a folder,
    // and both are absolute there.
    expect(isUnder('/p/src', '/p/src/a.js')).toBe(true)
    expect(isUnder('/p/src', '/p/src-tauri')).toBe(false)
  })
})
