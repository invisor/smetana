import { afterEach, describe, expect, it, vi } from 'vitest'
import { languageFor } from '../../../../src/components/files/editor/languages.js'

afterEach(() => {
  vi.doUnmock('@codemirror/lang-json')
  vi.resetModules()
})

describe('languageFor', () => {
  it('узнаёт язык по расширению', async () => {
    expect(await languageFor('main.rs')).not.toBe(null)
    expect(await languageFor('index.js')).not.toBe(null)
    expect(await languageFor('data.json')).not.toBe(null)
  })

  it('узнаёт файлы, у которых имя важнее расширения', async () => {
    expect(await languageFor('Dockerfile')).not.toBe(null)
    expect(await languageFor('Makefile')).not.toBe(null)
  })

  it('регистр в имени не мешает', async () => {
    expect(await languageFor('MAIN.RS')).not.toBe(null)
    expect(await languageFor('/путь/DOCKERFILE')).not.toBe(null)
  })

  it('берёт имя из конца пути, а не весь путь', async () => {
    expect(await languageFor('src/stores/tabs.js')).not.toBe(null)
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

  it('непривезённый чанк даёт текст без подсветки, а не бросок', async () => {
    vi.resetModules()
    vi.doMock('@codemirror/lang-json', () => {
      throw new Error('чанк не приехал')
    })
    const fresh = await import('../../../../src/components/files/editor/languages.js')

    await expect(fresh.languageFor('data.json')).resolves.toBe(null)
  })
})
