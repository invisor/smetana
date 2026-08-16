import { describe, expect, it } from 'vitest'
import bodies from '../src/catppuccinBodies.json'
import associations from '../src/catppuccinAssociations.json'
import { fileIconUrl, folderIconUrl } from '../src/catppuccinIcon.js'

/* The resolver hands back a `data:` URL, which says nothing about which icon it
   is. Rebuilding the expected URL from the artifact is what makes an assertion
   here readable — and it is why these tests are about routing and palette rather
   than about pixels. */
const urlOf = (name, theme = 'dark') => {
  const body = bodies.icons[name].replace(
    /\$(\w+)\$/g,
    (whole, colour) => bodies.colours[theme][colour] ?? whole
  )
  return `data:image/svg+xml;utf8,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">${body}</svg>`
  )}`
}

describe('the vendored artifacts', () => {
  it('are there at all, which the fetch script also asserts', () => {
    expect(Object.keys(associations.fileExtensions).length).toBeGreaterThan(500)
    expect(Object.keys(associations.folderNames).length).toBeGreaterThan(300)
    expect(Object.keys(bodies.icons).length).toBeGreaterThan(600)
    expect(associations.license).toBe('MIT')
  })

  it('name only icons the artifact actually carries', () => {
    // A table entry pointing at a missing icon is silent: the row draws the
    // default page and looks like a file nobody wrote a rule for.
    const missing = new Set()
    for (const table of ['fileNames', 'fileExtensions', 'languageIds']) {
      for (const icon of Object.values(associations[table])) {
        if (!bodies.icons[icon]) missing.add(icon)
      }
    }
    for (const icon of Object.values(associations.folderNames)) {
      if (!bodies.icons[`folder_${icon}`]) missing.add(icon)
      if (!bodies.icons[`folder_${icon}_open`]) missing.add(`${icon} (open)`)
    }
    expect([...missing]).toEqual([])
  })

  it('carries both palettes, over the same colour names', () => {
    expect(Object.keys(bodies.colours).sort()).toEqual(['dark', 'light'])
    expect(Object.keys(bodies.colours.light)).toEqual(Object.keys(bodies.colours.dark))
    // Latte's text against Macchiato's: the light theme was the whole reason
    // for the second palette, and a build that read one flavour twice would
    // pass every other test here.
    expect(bodies.colours.light.text).not.toBe(bodies.colours.dark.text)
  })

  it('leaves no placeholder unsubstituted in either theme', () => {
    // A colour name the palette does not carry would survive into the SVG as
    // `var(--vscode-ctp-…)`, which resolves to nothing inside a data: URL and
    // draws an invisible icon.
    for (const theme of ['light', 'dark']) {
      expect(fileIconUrl('main.rs', theme)).not.toMatch(/%24\w+%24/)
      expect(folderIconUrl('src', true, theme)).not.toMatch(/%24\w+%24/)
    }
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
    // The whole reason the walk starts at the first dot: a declaration file has
    // its own icon in this set, and asking `ts` first would never reach it.
    expect(fileIconUrl('component.d.ts')).toBe(urlOf('typescript-def'))
    expect(fileIconUrl('component.d.ts')).not.toBe(fileIconUrl('component.ts'))
  })

  it('an unknown name is the plain page rather than an empty src', () => {
    expect(fileIconUrl('data.parquet-unknown')).toBe(urlOf('_file'))
    expect(fileIconUrl('')).toBe(urlOf('_file'))
    expect(fileIconUrl(undefined)).toBe(urlOf('_file'))
  })

  it('takes a path as readily as a name', () => {
    expect(fileIconUrl('src/components/files/FileTree.vue')).toBe(urlOf('vue'))
    expect(fileIconUrl('C:\\project\\src\\main.rs')).toBe(urlOf('rust'))
  })

  it('a name that is also a prototype key is still a file', () => {
    // The tables are parsed from JSON, so they arrive carrying
    // Object.prototype the way an object literal does.
    expect(fileIconUrl('constructor')).toBe(urlOf('_file'))
    expect(fileIconUrl('toString')).toBe(urlOf('_file'))
  })
})

describe('what a folder resolves to', () => {
  it('takes the folder_ prefix and its open twin', () => {
    expect(folderIconUrl('src', false)).toBe(urlOf('folder_src'))
    expect(folderIconUrl('src', true)).toBe(urlOf('folder_src_open'))
    expect(folderIconUrl('node_modules', false)).toBe(urlOf('folder_node'))
  })

  it('never draws a file icon for a folder', () => {
    // Many folder icon names are also file icon names, so a resolver that skips
    // the prefix draws a `gradle` folder as a gradle file.
    for (const name of ['gradle', 'src', 'test', 'dist', 'docs', 'admin']) {
      const drawn = folderIconUrl(name, false)
      expect(drawn).not.toBe(urlOf('_file'))
      const icon = Object.keys(bodies.icons).find((key) => urlOf(key) === drawn)
      expect(icon?.startsWith('folder'), `${name} resolved to ${icon}`).toBe(true)
    }
  })

  it('an unmapped folder is the plain folder, open or closed', () => {
    expect(folderIconUrl('smetana-something', false)).toBe(urlOf('_folder'))
    expect(folderIconUrl('smetana-something', true)).toBe(urlOf('_folder_open'))
  })
})

describe('the two palettes', () => {
  it('draw the same icon differently', () => {
    expect(fileIconUrl('main.rs', 'light')).not.toBe(fileIconUrl('main.rs', 'dark'))
  })

  it('put Latte on the light theme and Macchiato on the dark one', () => {
    // The measured failure this branch was rebuilt for: Macchiato's `text` is
    // #cad3f5, which sits at 1.38:1 on --surface in the light theme. Latte's
    // is a dark grey.
    expect(fileIconUrl('unknown-thing', 'light')).toContain(
      encodeURIComponent(bodies.colours.light.text)
    )
    expect(fileIconUrl('unknown-thing', 'dark')).toContain(
      encodeURIComponent(bodies.colours.dark.text)
    )
  })

  it('an unknown theme name draws rather than drawing nothing', () => {
    // The three callers pass a document attribute, and an attribute is not a
    // closed vocabulary.
    expect(fileIconUrl('main.rs', 'sepia')).toBe(fileIconUrl('main.rs', 'dark'))
  })
})
