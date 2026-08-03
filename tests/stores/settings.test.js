import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'
import { buffer } from '../support/fixtures.js'

let ipc
let emit
let nextTick
let settings
let tabs

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  emit = loaded.emit
  nextTick = loaded.nextTick
  settings = loaded.stores.settings
  tabs = loaded.stores.tabs
})

describe('загрузка', () => {
  it('пустой ответ оставляет умолчания', async () => {
    ipc.on('settings_load', {})

    await settings.loadSettings()

    expect(settings.settings.appearance).toEqual({ theme: 'dark', density: 'comfortable' })
    expect(settings.settings.openProjects).toEqual([])
    expect(settings.settings.project.activeTab).toBe('kanban')
  })

  it('сохранённое накрывает умолчания по полям, а не по секциям', async () => {
    ipc.on('settings_load', { appearance: { theme: 'light' } })

    await settings.loadSettings()

    expect(settings.settings.appearance.theme).toBe('light')
    expect(settings.settings.appearance.density).toBe('comfortable')
  })

  it('отказ чтения оставляет умолчания и не роняет запуск', async () => {
    ipc.fail('settings_load', new Error('файл не читается'))

    await expect(settings.loadSettings()).resolves.toBeTruthy()
    expect(settings.settings.appearance.theme).toBe('dark')
  })
})

describe('раскладка проекта', () => {
  it('проект, которого нет в карте, начинается с чистого, а не донашивает чужое', async () => {
    ipc.on('settings_load', {
      project: { sideTab: 'agents', openTabs: ['a.txt'], expanded: ['src'] }
    })
    await settings.loadSettings()
    expect(settings.settings.project.openTabs).toEqual(['a.txt'])

    ipc.on('settings_load', { project: { sideTab: 'files' } })
    await settings.loadProjectLayout('/новый')

    expect(settings.settings.project.sideTab).toBe('files')
    expect(settings.settings.project.openTabs).toEqual([])
    expect(settings.settings.project.expanded).toEqual([])
  })

  it('без проекта ставит умолчания и на диск не ходит', async () => {
    ipc.on('settings_load', { project: { sideTab: 'agents' } })
    await settings.loadSettings()
    const before = ipc.calls('settings_load').length

    await settings.loadProjectLayout(null)

    expect(settings.settings.project.sideTab).toBe('files')
    expect(ipc.calls('settings_load')).toHaveLength(before)
  })

  it('секция сливается на месте: ссылка на объект остаётся прежней', async () => {
    ipc.on('settings_load', {})
    await settings.loadSettings()
    const held = settings.settings.project

    ipc.on('settings_load', { project: { sideTab: 'agents' } })
    await settings.loadProjectLayout('/новый')

    expect(settings.settings.project).toBe(held)
    expect(held.sideTab).toBe('agents')
  })
})

describe('запись', () => {
  beforeEach(async () => {
    ipc.on('settings_load', {})
    ipc.on('settings_save', null)
    await settings.loadSettings()
  })

  it('поток правок схлопывается в одну запись', async () => {
    vi.useFakeTimers()

    settings.settings.layout.leftCollapsed = true
    settings.settings.layout.rightCollapsed = true
    settings.settings.project.sideTab = 'agents'
    await nextTick()
    vi.advanceTimersByTime(400)
    vi.useRealTimers()
    await Promise.resolve()

    expect(ipc.calls('settings_save')).toHaveLength(1)
    expect(ipc.calls('settings_save')[0].settings.layout.leftCollapsed).toBe(true)
  })

  it('flushPending видит таймер, заведённый в том же синхронном блоке', async () => {
    settings.settings.layout.leftCollapsed = true
    await settings.flushPending()

    expect(ipc.calls('settings_save')).toHaveLength(1)
  })

  it('две записи не летят внахлёст', async () => {
    const order = []
    ipc.on('settings_save', async (args) => {
      const mark = args.settings.layout.leftCollapsed
      order.push(`начало:${mark}`)
      await new Promise((resolve) => setTimeout(resolve, 10))
      order.push(`конец:${mark}`)
      return null
    })

    settings.settings.layout.leftCollapsed = true
    const first = settings.flushPending()
    settings.settings.layout.leftCollapsed = false
    const second = settings.flushPending()
    await Promise.all([first, second])

    expect(order).toEqual(['начало:true', 'конец:true', 'начало:false', 'конец:false'])
  })

  it('на диск уходит простой объект, а не реактивный прокси', async () => {
    settings.settings.layout.leftCollapsed = true
    await settings.flushPending()

    const sent = ipc.calls('settings_save')[0].settings
    expect(sent).toEqual(JSON.parse(JSON.stringify(sent)))
  })

  it('отказ записи не роняет ожидающего', async () => {
    ipc.fail('settings_save', new Error('диск полон'))

    settings.settings.layout.leftCollapsed = true
    await expect(settings.flushPending()).resolves.toBeUndefined()
  })
})

describe('закрытие окна', () => {
  beforeEach(async () => {
    ipc.on('settings_load', {})
    ipc.on('settings_save', null)
    await settings.loadSettings()
  })

  it('дожимает запись и только потом разрушает окно', async () => {
    ipc.on('plugin:window|destroy', null)
    settings.settings.layout.leftCollapsed = true
    await nextTick()

    await emit('tauri://close-requested', {})
    await vi.waitFor(() => expect(ipc.commands()).toContain('plugin:window|destroy'))

    const commands = ipc.commands()
    expect(commands.indexOf('settings_save')).toBeLessThan(
      commands.indexOf('plugin:window|destroy')
    )
  })

  it('«человек передумал» не закрывает окно', async () => {
    ipc.on('plugin:window|destroy', null)
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'правка' }))
    tabs.onUnsaved(() => false)

    await emit('tauri://close-requested', {})
    await new Promise((resolve) => setTimeout(resolve, 30))

    expect(ipc.commands()).not.toContain('plugin:window|destroy')
  })

  it('окно закрывается и тогда, когда запись не отвечает', async () => {
    /* Обещано, что окно закроется. Не обещано, что правка успеет на диск. */
    ipc.on('plugin:window|destroy', null)
    ipc.on('settings_save', () => new Promise(() => {}))
    settings.settings.layout.leftCollapsed = true
    await nextTick()

    await emit('tauri://close-requested', {})
    await vi.waitFor(
      () => expect(ipc.commands()).toContain('plugin:window|destroy'),
      { timeout: 4000 }
    )
  })

  it('вопрос о несохранённом задаётся до записи настроек, а не внутри её потолка', async () => {
    ipc.on('plugin:window|destroy', null)
    const asked = []
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'правка' }))
    tabs.onUnsaved(() => {
      asked.push(ipc.commands().includes('settings_save'))
      return true
    })
    settings.settings.layout.leftCollapsed = true
    await nextTick()

    await emit('tauri://close-requested', {})
    await vi.waitFor(() => expect(ipc.commands()).toContain('plugin:window|destroy'))

    expect(asked).toEqual([false])
  })
})
