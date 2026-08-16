import { describe, expect, it } from 'vitest'
import catppuccinIcons from '@iconify-json/catppuccin/icons.json'
import associations from '../src/catppuccinAssociations.json'
import { fileIconUrl, folderIconUrl } from '../src/catppuccinIcon.js'

/* The resolver hands back a `data:` URL, which says nothing about which icon it
   is. Comparing against the URL the set's own body produces is what makes an
   assertion here readable — and it is why these tests are about routing rather
   than about pixels. */
const urlOf = (name) => {
  const body = catppuccinIcons.icons[name].body
  return `data:image/svg+xml;utf8,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">${body}</svg>`
  )}`
}

describe('the vendored association tables', () => {
  it('are there at all, which the fetch script also asserts', () => {
    expect(Object.keys(associations.fileExtensions).length).toBeGreaterThan(500)
    expect(Object.keys(associations.folderNames).length).toBeGreaterThan(300)
    expect(associations.license).toBe('MIT')
  })

  it('name only icons the set actually ships', () => {
    // A table entry pointing at a missing icon is silent: the row draws the
    // default page and looks like a file nobody wrote a rule for.
    const missing = new Set()
    for (const icon of Object.values(associations.fileNames)) {
      if (!catppuccinIcons.icons[icon.replace(/_/g, '-')]) missing.add(icon)
    }
    for (const icon of Object.values(associations.folderNames)) {
      if (!catppuccinIcons.icons[`folder-${icon.replace(/_/g, '-')}`]) missing.add(icon)
    }
    expect([...missing]).toEqual([])
  })
})

describe('what a file resolves to', () => {
  it('is drawn by its own name first', () => {
    expect(fileIconUrl('package.json')).toBe(urlOf('package-json'))
    expect(fileIconUrl('Cargo.toml')).toBe(urlOf('cargo'))
    expect(fileIconUrl('Dockerfile')).toBe(urlOf('docker'))
  })

  it('falls back to the extension, and reads a language id where one is spelled alike', () => {
    expect(fileIconUrl('main.rs')).toBe(urlOf('rust'))
    expect(fileIconUrl('App.vue')).toBe(urlOf('vue'))
    expect(fileIconUrl('tabs.js')).toBe(urlOf('javascript'))
  })

  it('walks a compound extension from the longest suffix down', () => {
    // The whole reason the walk starts at the first dot: a spec file has its
    // own icon in this set, and asking `ts` first would never reach it.
    expect(fileIconUrl('tabs.test.js')).not.toBe(fileIconUrl('tabs.js'))
    expect(fileIconUrl('component.d.ts')).toBe(urlOf('typescript-def'))
  })

  it('an unknown name is the plain page rather than an empty src', () => {
    expect(fileIconUrl('data.parquet-unknown')).toBe(urlOf('file'))
    expect(fileIconUrl('')).toBe(urlOf('file'))
    expect(fileIconUrl(undefined)).toBe(urlOf('file'))
  })

  it('takes a path as readily as a name', () => {
    expect(fileIconUrl('src/components/files/FileTree.vue')).toBe(urlOf('vue'))
    expect(fileIconUrl('C:\\project\\src\\main.rs')).toBe(urlOf('rust'))
  })

  it('a name that is also a prototype key is still a file', () => {
    // The tables are parsed from JSON, so they carry Object.prototype with
    // them the way an object literal does.
    expect(fileIconUrl('constructor')).toBe(urlOf('file'))
    expect(fileIconUrl('toString')).toBe(urlOf('file'))
  })
})

describe('what a folder resolves to', () => {
  it('takes the folder- prefix and its open twin', () => {
    expect(folderIconUrl('src', false)).toBe(urlOf('folder-src'))
    expect(folderIconUrl('src', true)).toBe(urlOf('folder-src-open'))
    expect(folderIconUrl('node_modules', false)).toBe(urlOf('folder-node'))
  })

  it('never draws a file icon for a folder', () => {
    // 50 of the 113 folder icon names are also file icon names, so a resolver
    // that skips the prefix draws a `gradle` folder as a gradle file. Every
    // folder answer here has to be a folder icon.
    for (const name of ['gradle', 'src', 'test', 'dist', 'docs', 'admin']) {
      const closed = folderIconUrl(name, false)
      expect(closed).not.toBe(urlOf('file'))
      const isFolder = Object.entries(catppuccinIcons.icons).some(
        ([key, icon]) => key.startsWith('folder') && urlOf(key) === closed && icon
      )
      expect(isFolder, `${name} did not resolve to a folder icon`).toBe(true)
    }
  })

  it('an unmapped folder is the plain folder, open or closed', () => {
    expect(folderIconUrl('smetana-something', false)).toBe(urlOf('folder'))
    expect(folderIconUrl('smetana-something', true)).toBe(urlOf('folder-open'))
  })
})
