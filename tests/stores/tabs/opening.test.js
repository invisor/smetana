import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../../support/stores.js'
import { buffer, fileText } from '../../support/fixtures.js'

let ipc
let files
let settings
let tabs

/* Состояние проекта — это settings.project: именно его читает и пишет tabs.js. */
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

const opened = async (path, options) => {
  tabs.openFile(path, options)
  await vi.waitFor(() => expect(tabs.buffers.get(path).loading).toBe(false))
}

describe('одиночный клик', () => {
  it('открывает файл временной вкладкой и делает её активной', async () => {
    await opened('a.txt')

    expect(state().openTabs).toEqual(['a.txt'])
    expect(state().previewTab).toBe('a.txt')
    expect(state().activeTab).toBe('a.txt')
    expect(tabs.buffers.get('a.txt').text).toBe('текст a.txt')
  })

  it('следующий клик подставляется на место временной, а не растит ряд', async () => {
    await opened('a.txt')
    await opened('b.txt')

    expect(state().openTabs).toEqual(['b.txt'])
    expect(state().previewTab).toBe('b.txt')
    expect(tabs.buffers.has('a.txt')).toBe(false)
  })

  it('замена происходит на том же месте ряда', async () => {
    await opened('a.txt', { permanent: true })
    await opened('b.txt')
    await opened('c.txt')

    expect(state().openTabs).toEqual(['a.txt', 'c.txt'])
  })
})

describe('двойной клик', () => {
  it('открывает постоянную вкладку рядом и не выселяет чужое превью', async () => {
    await opened('a.txt')
    await opened('b.txt', { permanent: true })

    expect(state().openTabs).toEqual(['a.txt', 'b.txt'])
    expect(state().previewTab).toBe('a.txt')
  })

  it('закрепляет вкладку, уже открытую временной', async () => {
    await opened('a.txt')
    tabs.openFile('a.txt', { permanent: true })

    expect(state().previewTab).toBe(null)
    expect(state().openTabs).toEqual(['a.txt'])
  })

  it('promote снимает временность и делает активной', async () => {
    await opened('a.txt')
    tabs.promote('a.txt')

    expect(state().previewTab).toBe(null)
    expect(state().activeTab).toBe('a.txt')
  })
})

describe('повторное открытие уже открытого', () => {
  it('только переключает активную и не читает файл заново', async () => {
    await opened('a.txt', { permanent: true })
    await opened('b.txt', { permanent: true })
    const before = ipc.calls('files_read').length

    tabs.openFile('a.txt')

    expect(state().activeTab).toBe('a.txt')
    expect(ipc.calls('files_read')).toHaveLength(before)
  })
})

describe('первая правка закрепляет вкладку', () => {
  it('setText снимает временность — так «временная никогда не грязная» становится правдой', async () => {
    await opened('a.txt')

    tabs.setText('a.txt', 'правленый')

    expect(state().previewTab).toBe(null)
    expect(tabs.isDirty('a.txt')).toBe(true)
  })

  it('правка в буфер, чьё первое чтение не вернулось, не проходит', async () => {
    tabs.openFile('a.txt')
    expect(tabs.buffers.get('a.txt').loading).toBe(true)

    tabs.setText('a.txt', 'преждевременно')

    expect(tabs.buffers.get('a.txt').text).toBe('')
    await vi.waitFor(() => expect(tabs.buffers.get('a.txt').loading).toBe(false))
    expect(tabs.buffers.get('a.txt').text).toBe('текст a.txt')
  })

  it('правка в буфер с отказом чтения не проходит', async () => {
    ipc.fail('files_read', { kind: 'binary', message: 'двоичный' })
    await opened('a.png')

    tabs.setText('a.png', 'что-то')

    expect(tabs.buffers.get('a.png').text).toBe('')
  })
})

describe('закрытие', () => {
  const openThree = async () => {
    await opened('a.txt', { permanent: true })
    await opened('b.txt', { permanent: true })
    await opened('c.txt', { permanent: true })
  }

  it('активной становится соседняя справа', async () => {
    await openThree()
    state().activeTab = 'b.txt'

    tabs.closeTab('b.txt')

    expect(state().openTabs).toEqual(['a.txt', 'c.txt'])
    expect(state().activeTab).toBe('c.txt')
  })

  it('для последней — соседняя слева', async () => {
    await openThree()
    state().activeTab = 'c.txt'

    tabs.closeTab('c.txt')

    expect(state().activeTab).toBe('b.txt')
  })

  it('опустевший ряд возвращает на доску', async () => {
    await opened('a.txt', { permanent: true })

    tabs.closeTab('a.txt')

    expect(state().openTabs).toEqual([])
    expect(state().activeTab).toBe('kanban')
    expect(tabs.buffers.has('a.txt')).toBe(false)
  })

  it('закрытие неактивной вкладки активную не двигает', async () => {
    await openThree()
    state().activeTab = 'a.txt'

    tabs.closeTab('c.txt')

    expect(state().activeTab).toBe('a.txt')
  })

  it('закрытие того, чего нет, ничего не делает', async () => {
    await opened('a.txt', { permanent: true })

    tabs.closeTab('нет-такого.txt')

    expect(state().openTabs).toEqual(['a.txt'])
  })
})

describe('tabList', () => {
  it('закреплённые идут первыми и не закрываются', async () => {
    await opened('a.txt')

    const list = tabs.tabList.value

    expect(list.slice(0, 2).map((tab) => tab.id)).toEqual(['chat', 'kanban'])
    expect(list[0].kind).toBe('pinned')
  })

  it('временная вкладка помечена своим видом', async () => {
    await opened('a.txt')

    expect(tabs.tabList.value[2]).toMatchObject({ id: 'a.txt', kind: 'preview', label: 'a.txt' })
  })

  it('вкладка с отказом чтения несёт замок и его причину', async () => {
    ipc.fail('files_read', { kind: 'tooLarge', message: 'слишком велик' })
    await opened('big.log')

    expect(tabs.tabList.value[2]).toMatchObject({
      readOnly: true,
      readOnlyHint: 'File is too large to open here.'
    })
  })

  it('первое чтение замка не получает: замок, мигающий на каждом открытии, врал бы', async () => {
    tabs.openFile('a.txt')

    expect(tabs.tabList.value[2].readOnly).toBe(false)
  })

  it('имя вкладки — только последний сегмент пути', async () => {
    await opened('src/stores/tabs.js')

    expect(tabs.tabList.value[2].label).toBe('tabs.js')
  })
})

describe('грязнота', () => {
  it('буфер с отказом чтения не считается грязным сам по себе', async () => {
    ipc.fail('files_read', { kind: 'notFound', message: 'нет' })
    await opened('a.txt')

    expect(tabs.isDirty('a.txt')).toBe(false)
  })

  it('текст, набранный до отказа чтения, обязан считаться несохранённым', async () => {
    tabs.buffers.set('a.txt', buffer({ text: 'набрано', original: '', error: { kind: 'io' } }))

    expect(tabs.isDirty('a.txt')).toBe(true)
  })

  it('dirtyPaths перечисляет только грязные из открытых', async () => {
    await opened('a.txt', { permanent: true })
    await opened('b.txt', { permanent: true })
    tabs.setText('b.txt', 'правка')

    expect(tabs.dirtyPaths.value).toEqual(['b.txt'])
  })
})
