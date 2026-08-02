import { afterEach, describe, expect, it, vi } from 'vitest'
import { languageFor } from '../../../../src/components/files/editor/languages.js'

/* Имя языка достаётся из LanguageSupport (.language.name) или из самого
   StreamLanguage (.name): legacy-режимы приходят вторым видом. */
const nameOf = async (path) => {
  const support = await languageFor(path)
  return support ? (support.language ?? support).name : null
}

afterEach(() => {
  vi.doUnmock('@codemirror/lang-json')
  vi.resetModules()
})

describe('languageFor', () => {
  it('узнаёт язык по расширению', async () => {
    expect(await nameOf('main.rs')).toBe('rust')
    expect(await nameOf('index.js')).toBe('javascript')
    expect(await nameOf('data.json')).toBe('json')
  })

  it('узнаёт файлы, у которых имя важнее расширения', async () => {
    const dockerfile = await languageFor('Dockerfile')
    expect(dockerfile).not.toBe(null)
    expect(typeof dockerfile.streamParser?.token).toBe('function')
    expect(await nameOf('Makefile')).toBe('shell')
  })

  it('регистр в имени не мешает', async () => {
    expect(await nameOf('MAIN.RS')).toBe('rust')
    const dockerfile = await languageFor('/путь/DOCKERFILE')
    expect(dockerfile).not.toBe(null)
    expect(typeof dockerfile.streamParser?.token).toBe('function')
  })

  it('берёт имя из конца пути, а не весь путь', async () => {
    expect(await nameOf('src/stores/tabs.js')).toBe('javascript')
  })

  it('файл без расширения — обычный текст, а не ошибка', async () => {
    expect(await languageFor('README')).toBe(null)
    expect(await languageFor('')).toBe(null)
    expect(await languageFor(null)).toBe(null)
  })

  it('точка в начале имени не считается расширением', async () => {
    /* dot > 0 в исходнике: у ".gitignore" точка нулевая, расширения нет. */
    expect(await languageFor('.gitignore')).toBe(null)
  })

  it('незнакомое расширение — обычный текст', async () => {
    expect(await languageFor('файл.неизвестное')).toBe(null)
  })

  it('Dockerfile загружает свой режим, у которого нет имени', async () => {
    const support = await languageFor('/путь/Dockerfile')
    expect(support).not.toBe(null)
    expect(typeof support.streamParser?.token).toBe('function')
  })

  it('непривезённый чанк даёт текст без подсветки, а не бросок', async () => {
    vi.resetModules()
    vi.doMock('@codemirror/lang-json', () => {
      throw new Error('чанк не приехал')
    })
    const fresh = await import('../../../../src/components/files/editor/languages.js')

    await expect(fresh.languageFor('data.json')).resolves.toBe(null)
  })
})
