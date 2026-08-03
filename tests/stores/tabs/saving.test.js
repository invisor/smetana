import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../../support/stores.js'
import { buffer, fileText } from '../../support/fixtures.js'

let ipc
let files
let tabs
let mtime

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  files = loaded.stores.files
  tabs = loaded.stores.tabs
  files.setRoot('/проект')
  mtime = 10
  ipc.on('files_read', (args) => fileText({ path: args.path, mtime }))
  ipc.on('files_write', () => {
    mtime += 1
    return mtime
  })
})

const opened = async (path = 'a.txt') => {
  tabs.openFile(path, { permanent: true })
  await vi.waitFor(() => expect(tabs.buffers.get(path).loading).toBe(false))
}

describe('удачное сохранение', () => {
  it('несёт метку времени, полученную при чтении, и снимает грязноту', async () => {
    await opened()
    tabs.setText('a.txt', 'правленый')

    await tabs.saveTab('a.txt')

    expect(ipc.calls('files_write')).toEqual([
      { root: '/проект', path: 'a.txt', text: 'правленый', expectedMtime: 10 }
    ])
    expect(tabs.isDirty('a.txt')).toBe(false)
    expect(tabs.buffers.get('a.txt').mtime).toBe(11)
  })

  it('чистая вкладка на диск не ходит', async () => {
    await opened()

    await tabs.saveTab('a.txt')

    expect(ipc.calls('files_write')).toHaveLength(0)
  })

  it('вторая запись подряд берёт метку, сдвинутую первой, а не захваченную при постановке', async () => {
    await opened()

    tabs.setText('a.txt', 'первая')
    const first = tabs.saveTab('a.txt')
    tabs.setText('a.txt', 'вторая')
    const second = tabs.saveTab('a.txt')
    await Promise.all([first, second])

    const writes = ipc.calls('files_write')
    expect(writes).toHaveLength(2)
    expect(writes[0].expectedMtime).toBe(10)
    expect(writes[1].expectedMtime).toBe(11)
    expect(tabs.isDirty('a.txt')).toBe(false)
  })

  it('набранное во время полёта остаётся грязным', async () => {
    await opened()
    tabs.setText('a.txt', 'первая правка')

    const pending = tabs.saveTab('a.txt')
    tabs.setText('a.txt', 'первая правка и ещё')
    await pending

    expect(tabs.buffers.get('a.txt').original).toBe('первая правка')
    expect(tabs.buffers.get('a.txt').text).toBe('первая правка и ещё')
    expect(tabs.isDirty('a.txt')).toBe(true)
  })

  it('удачная запись гасит прежний отказ записи', async () => {
    await opened()
    tabs.buffers.set('a.txt', buffer({ text: 'правка', saveError: { kind: 'denied' } }))

    await tabs.saveTab('a.txt')

    expect(tabs.buffers.get('a.txt').saveError).toBe(null)
  })
})

describe('отказы', () => {
  it('stale поднимает полоску, ничего не теряя', async () => {
    await opened()
    tabs.setText('a.txt', 'моя правка')
    ipc.fail('files_write', { kind: 'stale', message: 'файл уехал' })

    await tabs.saveTab('a.txt')

    const current = tabs.buffers.get('a.txt')
    expect(current.stale).toBe(true)
    expect(current.text).toBe('моя правка')
    expect(current.error).toBe(null)
    expect(current.saveError).toBe(null)
  })

  it('отказ записи не запирает поле: править и сохранять заново можно', async () => {
    await opened()
    tabs.setText('a.txt', 'моя правка')
    ipc.fail('files_write', { kind: 'denied', message: 'нет прав' })

    await tabs.saveTab('a.txt')

    const current = tabs.buffers.get('a.txt')
    expect(current.saveError).toEqual({ kind: 'denied', message: 'нет прав' })
    expect(current.error).toBe(null)

    tabs.setText('a.txt', 'ещё правка')
    expect(tabs.buffers.get('a.txt').text).toBe('ещё правка')
  })

  it('отказ не останавливает очередь: следующая запись проходит', async () => {
    await opened()
    tabs.setText('a.txt', 'первая')
    ipc.fail('files_write', { kind: 'denied', message: 'нет прав' })
    await tabs.saveTab('a.txt')

    ipc.on('files_write', () => 12)
    tabs.setText('a.txt', 'вторая')
    await tabs.saveTab('a.txt')

    expect(tabs.buffers.get('a.txt').mtime).toBe(12)
    expect(tabs.isDirty('a.txt')).toBe(false)
  })
})

describe('что не пишется никогда', () => {
  it('буфер, чьё первое чтение не вернулось', async () => {
    tabs.openFile('a.txt', { permanent: true })

    await tabs.saveTab('a.txt')

    expect(ipc.calls('files_write')).toHaveLength(0)
  })

  /* Буфер из openFile ещё не грязный: text === original === '', и saveTab
     выходит по !isDirty, а не по buffer.loading — проверка замка вообще не
     срабатывает. Чтобы застать именно её, нужен буфер одновременно
     загружающийся и грязный. Через публичный API такого состояния не
     получить (setText не пускает правки, пока стоит loading) — оно
     синтетическое, приложение его не порождает. Тест стережёт сам замок в
     saveTab, а не достижимое состояние. */
  it('буфер, одновременно загружающийся и грязный, не пишется — стережём саму проверку loading', async () => {
    tabs.buffers.set(
      'a.txt',
      buffer({ loading: true, text: 'набрано', original: 'исходное' })
    )

    await tabs.saveTab('a.txt')

    expect(ipc.calls('files_write')).toHaveLength(0)
  })

  it('буфер с отказом чтения', async () => {
    tabs.buffers.set('a.txt', buffer({ text: 'что-то', original: '', error: { kind: 'binary' } }))

    await tabs.saveTab('a.txt')

    expect(ipc.calls('files_write')).toHaveLength(0)
  })

  it('вкладка, которой нет', async () => {
    await tabs.saveTab('нет-такой.txt')

    expect(ipc.calls('files_write')).toHaveLength(0)
  })
})

describe('saveTabs', () => {
  it('пишет только грязные из перечисленных', async () => {
    await opened('a.txt')
    tabs.openFile('b.txt', { permanent: true })
    await vi.waitFor(() => expect(tabs.buffers.get('b.txt').loading).toBe(false))
    tabs.setText('b.txt', 'правка')

    await tabs.saveTabs(['a.txt', 'b.txt'])

    expect(ipc.calls('files_write').map((call) => call.path)).toEqual(['b.txt'])
  })
})
