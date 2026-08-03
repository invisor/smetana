import { afterEach, describe, expect, it, vi } from 'vitest'
import { languageFor } from '../../../../src/components/files/editor/languages.js'

/* The language's name comes from the LanguageSupport (.language.name) or from
   the StreamLanguage itself (.name): legacy modes arrive in the second form. */
const nameOf = async (path) => {
  const support = await languageFor(path)
  return support ? (support.language ?? support).name : null
}

afterEach(() => {
  vi.doUnmock('@codemirror/lang-json')
  vi.resetModules()
})

describe('languageFor', () => {
  it('recognises the main file extensions', async () => {
    /* Checks the extension-to-language mapping from the review table: this rules
       out undetected typos such as rs→python or vue→java. */
    expect(await nameOf('main.rs')).toBe('rust')
    expect(await nameOf('index.js')).toBe('javascript')
    expect(await nameOf('data.json')).toBe('json')
    expect(await nameOf('a.py')).toBe('python')
    expect(await nameOf('app.ts')).toBe('typescript')
    expect(await nameOf('page.html')).toBe('html')
    expect(await nameOf('style.css')).toBe('css')
    expect(await nameOf('conf.yaml')).toBe('yaml')
    expect(await nameOf('main.go')).toBe('go')
    expect(await nameOf('Main.java')).toBe('java')
    expect(await nameOf('main.c')).toBe('cpp')
    expect(await nameOf('App.vue')).toBe('vue')
    expect(await nameOf('Cargo.toml')).toBe('toml')
    expect(await nameOf('run.sh')).toBe('shell')
    expect(await nameOf('app.ini')).toBe('properties')
    expect(await nameOf('Makefile')).toBe('shell')
  })

  it('recognises files whose name matters more than their extension', async () => {
    const dockerfile = await languageFor('Dockerfile')
    expect(dockerfile).not.toBe(null)
    expect(typeof dockerfile.streamParser?.token).toBe('function')
    expect(await nameOf('Makefile')).toBe('shell')
  })

  it('the case of the name does not get in the way', async () => {
    expect(await nameOf('MAIN.RS')).toBe('rust')
    const dockerfile = await languageFor('/path/DOCKERFILE')
    expect(dockerfile).not.toBe(null)
    expect(typeof dockerfile.streamParser?.token).toBe('function')
  })

  it('takes the name from the end of the path rather than the whole path', async () => {
    expect(await nameOf('src/stores/tabs.js')).toBe('javascript')
  })

  it('a file with no extension is plain text, not an error', async () => {
    expect(await languageFor('README')).toBe(null)
    expect(await languageFor('')).toBe(null)
    expect(await languageFor(null)).toBe(null)
  })

  it('a leading dot in a name does not count as an extension', async () => {
    /* dot > 0 in the source: ".gitignore" has its dot at zero, so no extension. */
    expect(await languageFor('.gitignore')).toBe(null)
  })

  it('an unknown extension is plain text', async () => {
    expect(await languageFor('file.unknown')).toBe(null)
  })

  it('Dockerfile loads its own mode, which has no name', async () => {
    const support = await languageFor('/path/Dockerfile')
    expect(support).not.toBe(null)
    expect(typeof support.streamParser?.token).toBe('function')
  })

  it('a chunk that did not arrive gives text without highlighting rather than a throw', async () => {
    vi.resetModules()
    vi.doMock('@codemirror/lang-json', () => {
      throw new Error('the chunk did not arrive')
    })
    const fresh = await import('../../../../src/components/files/editor/languages.js')

    await expect(fresh.languageFor('data.json')).resolves.toBe(null)
  })
})
