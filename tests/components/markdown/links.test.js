import { describe, it, expect } from 'vitest'
import { classifyLink, localHref, splitPath } from '../../../src/components/markdown/links.js'

describe('classifyLink: external', () => {
  it('reads an http or https target as external', () => {
    expect(classifyLink('https://example.com/x')).toEqual({
      kind: 'external',
      href: 'https://example.com/x'
    })
    expect(classifyLink('http://localhost:5173')).toEqual({
      kind: 'external',
      href: 'http://localhost:5173'
    })
  })

  /* A scheme is case-insensitive by RFC 3986, but `opener:allow-open-url`'s
     scope is spelled `https://*` and `http://*` — lowercasing only the scheme
     is what makes the two agree by construction, and the rest of the URL is
     left exactly as typed since case is meaningful in a path. */
  it('lowercases only the scheme, leaving the rest of the URL untouched', () => {
    expect(classifyLink('HTTPS://Example.com/X')).toEqual({
      kind: 'external',
      href: 'https://Example.com/X'
    })
  })
})

describe('classifyLink: every other scheme stays unrecognised', () => {
  it('declines mailto, javascript and a pasted file: URI, whatever the case', () => {
    expect(classifyLink('mailto:person@example.com')).toBeNull()
    expect(classifyLink('JAVASCRIPT:alert(1)')).toBeNull()
    expect(classifyLink('file:///Users/x/notes.txt')).toBeNull()
    expect(classifyLink('data:text/plain,hi')).toBeNull()
  })

  it('declines a bare fragment, which names a spot this parser gives no id to', () => {
    expect(classifyLink('#section')).toBeNull()
  })

  it('declines the empty string and anything that is not a string', () => {
    expect(classifyLink('')).toBeNull()
    expect(classifyLink(undefined)).toBeNull()
    expect(classifyLink(null)).toBeNull()
  })
})

/* `OTHER_SCHEME` keeps RFC 3986's dot, which an earlier version of this
   module had dropped to stop `tauri.conf.json:41` reading as a scheme — and
   dropping it instead stopped every *dotted* scheme being recognised at
   all, `com.example.app://…` (an ordinary desktop deep link) included. The
   fix is a negative lookahead on what follows the colon rather than a
   narrower grammar, so a dotted scheme is still a scheme unless a digit
   comes right after it. */
describe('classifyLink: a dotted scheme is still a scheme', () => {
  it('declines a reverse-DNS deep link, dots and all', () => {
    expect(classifyLink('com.example.app://x')).toBeNull()
  })

  /* The lookahead is not the whole answer, and this is the shape it leaves
     open: a dotted scheme whose colon happens to be followed by a digit and
     nothing else distinguishes it from `tauri.conf.json:41` — both are a
     dot, a short alphanumeric run and a line-shaped suffix, and neither
     regex has a dictionary of real extensions or real scheme names to tell
     them apart by. It reads as local, the same as a genuine file with a line
     number must. `OTHER_SCHEME`'s own header names this gap next to the
     wider one `LOOKS_LIKE_PATH`'s header names (`foo:1/bar`, which needs a
     `/` as well) — it is not chased for the same reason: nothing reaches
     `openExternal` from here, and Rust's own `resolve_within` holds the
     filesystem boundary regardless of what this module classifies. */
  it('cannot tell a dotted scheme with no slash from a real file and a line, and does not try', () => {
    expect(classifyLink('com.example.app:41')).toEqual({
      kind: 'local',
      targetKind: 'file',
      path: 'com.example.app',
      display: 'com.example.app:41'
    })
  })
})

describe('classifyLink: local', () => {
  it('reads a bare relative path as a local file, with no line suffix', () => {
    expect(classifyLink('src-tauri/tauri.conf.json')).toEqual({
      kind: 'local',
      targetKind: 'file',
      path: 'src-tauri/tauri.conf.json',
      display: 'src-tauri/tauri.conf.json'
    })
  })

  /* The line number rides on `display`, which is what the visible text is cut
     from, and is stripped from `path`, which is what the file is opened by —
     no filesystem holds a file whose name ends in a colon and a number. */
  it('strips a trailing line number from the path but keeps it in the display text', () => {
    expect(classifyLink('src-tauri/tauri.conf.json:41')).toEqual({
      kind: 'local',
      targetKind: 'file',
      path: 'src-tauri/tauri.conf.json',
      display: 'src-tauri/tauri.conf.json:41'
    })
  })

  it('reads a line and a column the same way', () => {
    expect(classifyLink('src/paths.js:12:3')).toEqual({
      kind: 'local',
      targetKind: 'file',
      path: 'src/paths.js',
      display: 'src/paths.js:12:3'
    })
  })

  /* A trailing slash is the one signal for "this is a folder", and it is
     spent rather than kept — `sm-prose.css` draws the folder glyph itself, so
     a slash left standing in `path` or `display` would draw two of them. */
  it('reads a trailing slash as a directory and drops it from both fields', () => {
    expect(classifyLink('docs/design/')).toEqual({
      kind: 'local',
      targetKind: 'dir',
      path: 'docs/design',
      display: 'docs/design'
    })
  })

  it('never reads a line suffix off a directory target', () => {
    expect(classifyLink('releases/v2/')).toEqual({
      kind: 'local',
      targetKind: 'dir',
      path: 'releases/v2',
      display: 'releases/v2'
    })
  })

  it('reads a bare file name with no folder above it', () => {
    expect(classifyLink('README.md')).toEqual({
      kind: 'local',
      targetKind: 'file',
      path: 'README.md',
      display: 'README.md'
    })
  })

  it('reads an absolute path as local too', () => {
    expect(classifyLink('/etc/hosts')).toEqual({
      kind: 'local',
      targetKind: 'file',
      path: '/etc/hosts',
      display: '/etc/hosts'
    })
  })

  it('declines a lone slash, which names no path at all', () => {
    expect(classifyLink('/')).toBeNull()
  })
})

/* "No scheme" used to be the whole rule, and it was too wide: a football
   score, an aspect ratio and a time of day carry no scheme either, and none
   of them is a path. Local is a positive shape now — a `/` somewhere, or an
   extension optionally followed by a line reference — tested against the
   whole target before anything is stripped off it. */
describe('classifyLink: local is a positive shape, not "everything else"', () => {
  it('declines a bare number pair that merely contains a colon, a score', () => {
    expect(classifyLink('2:1')).toBeNull()
  })

  it('declines an aspect ratio the same way', () => {
    expect(classifyLink('16:9')).toBeNull()
  })

  it('declines a time of day, which is the same shape again', () => {
    expect(classifyLink('12:30')).toBeNull()
  })

  it('declines a bare word with no slash and no extension', () => {
    expect(classifyLink('docs')).toBeNull()
    expect(classifyLink('notes')).toBeNull()
  })

  /* The shape is tested against the target as written, before the line
     suffix is stripped — stripping first and then judging what is left
     would read `'2'` off `'2:1'` as an extension-less one-character name,
     which is exactly the shape this rule exists to keep out. */
  it('still reads a bare file name with a line number as local, extension and all', () => {
    expect(classifyLink('tauri.conf.json:41')).toEqual({
      kind: 'local',
      targetKind: 'file',
      path: 'tauri.conf.json',
      display: 'tauri.conf.json:41'
    })
  })

  it('still reads a path with a slash as local, whatever else it lacks', () => {
    expect(classifyLink('src/paths.js')).toEqual({
      kind: 'local',
      targetKind: 'file',
      path: 'src/paths.js',
      display: 'src/paths.js'
    })
  })

  it('still reads a directory target as local off its trailing slash alone', () => {
    expect(classifyLink('docs/')).toEqual({
      kind: 'local',
      targetKind: 'dir',
      path: 'docs',
      display: 'docs'
    })
  })
})

describe('splitPath', () => {
  it('cuts at the last slash, keeping it on the head', () => {
    expect(splitPath('src-tauri/tauri.conf.json:41')).toEqual({
      head: 'src-tauri/',
      tail: 'tauri.conf.json:41'
    })
  })

  it('answers an empty head for a target with no slash in it', () => {
    expect(splitPath('README.md')).toEqual({ head: '', tail: 'README.md' })
  })

  it('takes the last slash of several, not the first', () => {
    expect(splitPath('src/components/markdown/links.js')).toEqual({
      head: 'src/components/markdown/',
      tail: 'links.js'
    })
  })
})

describe('localHref', () => {
  it('builds a three-slash file URI from a unix root and a relative path', () => {
    expect(localHref('/Users/flexo/project', 'src-tauri/tauri.conf.json')).toBe(
      'file:///Users/flexo/project/src-tauri/tauri.conf.json'
    )
  })

  it('answers the empty string with no root at all, rather than a broken URI', () => {
    expect(localHref('', 'src-tauri/tauri.conf.json')).toBe('')
    expect(localHref(null, 'src-tauri/tauri.conf.json')).toBe('')
  })

  it('escapes a character a URI cannot carry raw, such as a space', () => {
    expect(localHref('/Users/flexo/my project', 'a b.txt')).toBe(
      'file:///Users/flexo/my%20project/a%20b.txt'
    )
  })
})
