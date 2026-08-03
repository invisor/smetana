import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../../support/stores.js'
import { buffer, fileText } from '../../support/fixtures.js'

let ipc
let files
let settings
let tabs

const state = () => settings.settings.project

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  files = loaded.stores.files
  settings = loaded.stores.settings
  tabs = loaded.stores.tabs
  files.setRoot('/проект')
  ipc.on('files_read', (args) => fileText({ path: args.path, text: `текст ${args.path}` }))
})

describe('markStale', () => {
  it('поднимает полоску', () => {
    tabs.buffers.set('a.txt', buffer({ text: 'правка' }))

    tabs.markStale('a.txt')

    expect(tabs.buffers.get('a.txt').stale).toBe(true)
  })

  it('снимает прежний отказ чтения: файл вернулся, запирать поле навсегда нельзя', () => {
    tabs.buffers.set('a.txt', buffer({ text: 'правка', error: { kind: 'notFound' } }))

    tabs.markStale('a.txt')

    expect(tabs.buffers.get('a.txt').error).toBe(null)
    expect(tabs.buffers.get('a.txt').stale).toBe(true)
  })

  it('загружающийся буфер не трогает', () => {
    tabs.buffers.set('a.txt', buffer({ loading: true }))

    tabs.markStale('a.txt')

    expect(tabs.buffers.get('a.txt').stale).toBe(false)
  })
})

describe('markGone', () => {
  it('вешает отказ чтения — вместе с ним приходят и замок, и объяснение', () => {
    tabs.buffers.set('a.txt', buffer())

    tabs.markGone('a.txt')

    expect(tabs.buffers.get('a.txt').error).toEqual({ kind: 'notFound' })
    expect(tabs.buffers.get('a.txt').stale).toBe(false)
  })

  it('не перезаписывает уже стоящий отказ', () => {
    tabs.buffers.set('a.txt', buffer({ error: { kind: 'binary' } }))

    tabs.markGone('a.txt')

    expect(tabs.buffers.get('a.txt').error).toEqual({ kind: 'binary' })
  })

  it('загружающийся буфер не трогает', () => {
    tabs.buffers.set('a.txt', buffer({ loading: true }))

    tabs.markGone('a.txt')

    expect(tabs.buffers.get('a.txt').error).toBe(null)
  })
})

describe('перечитывание', () => {
  it('reloadTab отдаёт победу диску: правки уходят', async () => {
    tabs.buffers.set('a.txt', buffer({ text: 'моя правка', original: 'исходный', stale: true }))

    await tabs.reloadTab('a.txt')

    const current = tabs.buffers.get('a.txt')
    expect(current.text).toBe('текст a.txt')
    expect(current.original).toBe('текст a.txt')
    expect(current.stale).toBe(false)
    expect(tabs.isDirty('a.txt')).toBe(false)
  })

  it('reloadTab вкладки, которой нет, на диск не ходит', async () => {
    await tabs.reloadTab('нет-такой.txt')

    expect(ipc.calls('files_read')).toHaveLength(0)
  })

  /* ДЕФЕКТ, а не задуманное поведение. Комментарий в load() (tabs.js:96-106)
     обещает, что перечитывание без force не сотрёт набранное, — но сам load()
     сбрасывает буфер в пустой до await readFile, а setText не пускает правки,
     пока стоит loading. Поэтому на строке 103 всегда сравниваются две пустые
     строки, и на прямых последовательных путях (restoreTabs → load без force,
     ни с чем не гоняясь) ветка защиты не срабатывает; то же и в ветке отказа
     (строка 123). Это не значит, что ветка мертва совсем: она достижима через
     гонку с keepMine, который захватывает буфер до своего await и переписывает
     его захваченным — см. тест ниже. Тест здесь закрепляет то, что код делает
     на прямом пути, чтобы починка не прошла незамеченной. */
  it('перечитывание без force стирает набранное: на прямых путях ветка защиты не срабатывает', async () => {
    tabs.buffers.set('a.txt', buffer({ text: 'моя правка', original: 'исходный', mtime: 1 }))
    state().openTabs = ['a.txt']

    await tabs.restoreTabs()

    const current = tabs.buffers.get('a.txt')
    expect(current.text).toBe('текст a.txt')
    expect(current.original).toBe('текст a.txt')
    expect(current.mtime).toBe(10)
    expect(tabs.isDirty('a.txt')).toBe(false)
  })

  /* ДЕФЕКТ. keepMine захватывает буфер в переменную ДО своего await readFile
     и после возврата пишет обратно этот захваченный буфер (только mtime и
     stale берутся из ответа). Если между захватом и возвратом load() успел
     сбросить и перечитать буфер (диск победил), keepMine затирает свежее
     содержимое диска захваченным старым — воскрешает уже вытесненный текст.
     При смене проекта (projects.js moveTo: resetTabs() и следом restoreTabs())
     это может протащить грязный буфер одного проекта в одноимённый файл
     другого (README.md, package.json — обычное совпадение путей).

     Гонка собрана детерминированно: первый вызов files_read (из keepMine)
     держится на управляемом промисе и не отпускается, пока не завершится
     второй вызов (из reloadTab, эмулирующего load() без гонки) — порядок
     завершения задаётся явно, а не порядком разрешения промисов. */
  it('гонка keepMine с параллельным load воскрешает текст, уже вытесненный диском', async () => {
    let releaseKeepMineRead
    const keepMineRead = new Promise((resolve) => {
      releaseKeepMineRead = resolve
    })
    let calls = 0
    ipc.on('files_read', (args) => {
      calls += 1
      if (calls === 1) {
        /* Ответ на чтение, запущенное keepMine: отпускается вручную ниже,
           после того как reloadTab уже победил диском. */
        return keepMineRead.then(() => fileText({ path: args.path, text: `текст ${args.path}`, mtime: 20 }))
      }
      /* Ответ на чтение, запущенное reloadTab: отдаётся сразу. */
      return fileText({ path: args.path, text: `текст ${args.path}`, mtime: 10 })
    })
    tabs.buffers.set('a.txt', buffer({ text: 'моя правка', original: 'исходный', stale: true, mtime: 1 }))

    const keepMinePromise = tabs.keepMine('a.txt')
    await tabs.reloadTab('a.txt')
    /* Диск победил: буфер уже чист и несёт содержимое с диска. */
    expect(tabs.isDirty('a.txt')).toBe(false)

    releaseKeepMineRead()
    await keepMinePromise

    const current = tabs.buffers.get('a.txt')
    expect(current.text).toBe('моя правка')
    expect(current.original).toBe('исходный')
    expect(current.mtime).toBe(20)
    expect(tabs.isDirty('a.txt')).toBe(true)
  })
})

describe('keepMine', () => {
  it('гасит полоску и догоняет метку, иначе следующий Cmd+S снова отказал бы', async () => {
    tabs.buffers.set('a.txt', buffer({ text: 'моя правка', original: 'исходный', stale: true, mtime: 1 }))

    await tabs.keepMine('a.txt')

    const current = tabs.buffers.get('a.txt')
    expect(current.stale).toBe(false)
    expect(current.mtime).toBe(10)
    expect(current.text).toBe('моя правка')
  })

  it('отказ чтения при этом прикладывается к буферу, а текст остаётся', async () => {
    ipc.fail('files_read', { kind: 'notFound', message: 'нет файла' })
    tabs.buffers.set('a.txt', buffer({ text: 'моя правка', stale: true }))

    await tabs.keepMine('a.txt')

    expect(tabs.buffers.get('a.txt').error).toEqual({ kind: 'notFound', message: 'нет файла' })
    expect(tabs.buffers.get('a.txt').text).toBe('моя правка')
  })
})

describe('restoreTabs', () => {
  it('читает всё, что было открыто', async () => {
    state().openTabs = ['a.txt', 'b.txt']

    await tabs.restoreTabs()

    expect(tabs.buffers.get('a.txt').text).toBe('текст a.txt')
    expect(tabs.buffers.get('b.txt').text).toBe('текст b.txt')
  })

  it('исчезнувший путь молча выпадает из списка', async () => {
    ipc.on('files_read', (args) => {
      if (args.path === 'нет.txt') throw { kind: 'notFound', message: 'нет' }
      return fileText({ path: args.path })
    })
    state().openTabs = ['a.txt', 'нет.txt']
    state().activeTab = 'нет.txt'
    state().previewTab = 'нет.txt'

    await tabs.restoreTabs()

    expect(state().openTabs).toEqual(['a.txt'])
    expect(state().activeTab).toBe('kanban')
    expect(state().previewTab).toBe(null)
    expect(tabs.buffers.has('нет.txt')).toBe(false)
  })

  it('нечитаемый по другой причине путь остаётся: пропадает только исчезнувший', async () => {
    ipc.on('files_read', (args) => {
      if (args.path === 'big.log') throw { kind: 'tooLarge', message: 'велик' }
      return fileText({ path: args.path })
    })
    state().openTabs = ['big.log']

    await tabs.restoreTabs()

    expect(state().openTabs).toEqual(['big.log'])
  })
})

describe('отказ от несохранённого', () => {
  it('discardTabs трогает только перечисленное', () => {
    tabs.buffers.set('a.txt', buffer({ text: 'правка a', original: 'исходный' }))
    tabs.buffers.set('b.txt', buffer({ text: 'правка b', original: 'исходный' }))

    tabs.discardTabs(['a.txt'])

    expect(tabs.isDirty('a.txt')).toBe(false)
    expect(tabs.isDirty('b.txt')).toBe(true)
  })

  it('resetTabs чистит буферы, но список вкладок не трогает', () => {
    tabs.buffers.set('a.txt', buffer())
    state().openTabs = ['a.txt']

    tabs.resetTabs()

    expect(tabs.buffers.size).toBe(0)
    expect(state().openTabs).toEqual(['a.txt'])
  })
})

describe('confirmUnsaved', () => {
  it('без грязных вкладок разрешает не спрашивая', async () => {
    const asked = vi.fn(() => false)
    tabs.onUnsaved(asked)

    await expect(tabs.confirmUnsaved()).resolves.toBe(true)
    expect(asked).not.toHaveBeenCalled()
  })

  it('без зарегистрированного вида разрешает: запереть приложение хуже', async () => {
    state().openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'правка' }))

    await expect(tabs.confirmUnsaved()).resolves.toBe(true)
  })

  it('передаёт виду список грязных и возвращает его ответ', async () => {
    state().openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'правка' }))
    const asked = vi.fn(() => false)
    tabs.onUnsaved(asked)

    await expect(tabs.confirmUnsaved()).resolves.toBe(false)
    expect(asked).toHaveBeenCalledWith(['a.txt'])
  })

  it('спрошенный про одну вкладку трогает только её', async () => {
    state().openTabs = ['a.txt', 'b.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'правка a' }))
    tabs.buffers.set('b.txt', buffer({ text: 'правка b' }))
    const asked = vi.fn(() => true)
    tabs.onUnsaved(asked)

    await tabs.confirmUnsaved(['b.txt'])

    expect(asked).toHaveBeenCalledWith(['b.txt'])
  })
})
