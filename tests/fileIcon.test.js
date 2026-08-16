import { describe, expect, it } from 'vitest'
import { fileIcon, folderIcon } from '../src/fileIcon.js'

describe('what a file is drawn as', () => {
  it('groups by meaning rather than by language', () => {
    // lucide ships no language logos, and at 13px an invented glyph per
    // language reads as noise. Everything a person edits as source is one
    // silhouette.
    expect(fileIcon('App.vue')).toBe('file-code')
    expect(fileIcon('main.rs')).toBe('file-code')
    expect(fileIcon('tabs.js')).toBe('file-code')
    expect(fileIcon('styles.css')).toBe('file-code')
    expect(fileIcon('index.html')).toBe('file-code')
  })

  it('tells prose from configuration', () => {
    expect(fileIcon('CLAUDE.md')).toBe('file-text')
    expect(fileIcon('notes.txt')).toBe('file-text')
    expect(fileIcon('settings.json')).toBe('braces')
    expect(fileIcon('tauri.conf.json')).toBe('braces')
    expect(fileIcon('vitest.config.yml')).toBe('braces')
  })

  it('names a manifest and its lock as one thing', () => {
    // A manifest is not read the way the rest of the configuration is: it is
    // what the project is made of, and it is the file a person looks for by
    // shape in a directory listing full of dotfiles.
    expect(fileIcon('package.json')).toBe('package')
    expect(fileIcon('Cargo.toml')).toBe('package')
    expect(fileIcon('Cargo.lock')).toBe('package')
    expect(fileIcon('pnpm-lock.yaml')).toBe('package')
  })

  it('a plain toml or json beside them is still configuration', () => {
    // The name wins over the extension, and only for the names listed: the
    // rest of the tree's toml is somebody's own settings file.
    expect(fileIcon('BD_VERSION.toml')).toBe('braces')
  })

  it('draws pictures and archives', () => {
    expect(fileIcon('app-icon.png')).toBe('image')
    expect(fileIcon('logo.svg')).toBe('image')
    expect(fileIcon('bd-aarch64.tar.gz')).toBe('file-archive')
    expect(fileIcon('release.zip')).toBe('file-archive')
  })

  it('reads the extension whatever case it is written in', () => {
    expect(fileIcon('README.MD')).toBe('file-text')
    expect(fileIcon('SCREENSHOT.PNG')).toBe('image')
  })

  it('knows the names that carry no extension at all', () => {
    expect(fileIcon('Dockerfile')).toBe('file-code')
    expect(fileIcon('Makefile')).toBe('file-code')
    expect(fileIcon('LICENSE')).toBe('file-text')
    expect(fileIcon('CHANGELOG')).toBe('file-text')
  })

  it('a leading dot is configuration by itself, so the list does not have to grow', () => {
    // .gitignore, .npmrc, .prettierrc, .zshrc: a dotfile in a project is
    // almost always something configured, and one rule beats a list nobody
    // remembers to extend. `.env.local` reaches it through an extension
    // nothing claims.
    expect(fileIcon('.gitignore')).toBe('braces')
    expect(fileIcon('.editorconfig')).toBe('braces')
    expect(fileIcon('.env')).toBe('braces')
    expect(fileIcon('.env.local')).toBe('braces')
  })

  it('an unknown file is an ordinary outcome and keeps the plain page', () => {
    // The same stance languages.js takes: not knowing what something is is
    // not an error, and the row still has to draw.
    expect(fileIcon('bd')).toBe('file')
    expect(fileIcon('data.parquet')).toBe('file')
    expect(fileIcon('')).toBe('file')
    expect(fileIcon(undefined)).toBe('file')
  })

  it('a name that is also a prototype key is still a file', () => {
    // Both maps are prototype-free. A plain object literal answers
    // `constructor` with the `Object` function, which reaches `Icon` as a
    // non-string glyph name — and a file called `constructor` is unremarkable
    // in a JavaScript tree.
    expect(fileIcon('constructor')).toBe('file')
    expect(fileIcon('toString')).toBe('file')
    expect(fileIcon('thing.constructor')).toBe('file')
  })

  it('takes a whole path as readily as a name, since two of its callers hold paths', () => {
    // stores/tabs.js keys by a project-relative path and the Git panel's
    // changes arrive the same way.
    expect(fileIcon('src/components/files/FileTree.vue')).toBe('file-code')
    expect(fileIcon('src-tauri/Cargo.toml')).toBe('package')
    expect(fileIcon('C:\\project\\src\\main.rs')).toBe('file-code')
  })
})

describe('what a folder is drawn as', () => {
  it('opens and closes', () => {
    expect(folderIcon('src', false)).toBe('folder')
    expect(folderIcon('src', true)).toBe('folder-open')
  })

  it('no name is special, including the one that looks like it should be', () => {
    // `.git` is the only thing the tree does not draw at all
    // (files::model::skip_in_tree), and the change list never sees it either,
    // so a glyph for it would be a rule nothing could reach.
    expect(folderIcon('.git', false)).toBe('folder')
    expect(folderIcon('node_modules', false)).toBe('folder')
    expect(folderIcon('.beads', true)).toBe('folder-open')
  })

  it('a trailing separator is drawn as a folder, which is what the change list needs', () => {
    // `--untracked-files=normal` reports an untracked directory as one record
    // with a trailing slash.
    expect(folderIcon('src-tauri/target/', false)).toBe('folder')
  })
})
