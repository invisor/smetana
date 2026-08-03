import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'
import { buffer, listing, snapshot } from '../support/fixtures.js'

let ipc
let settings
let projects
let tabs

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  settings = loaded.stores.settings
  projects = loaded.stores.projects
  tabs = loaded.stores.tabs

  ipc.on('settings_load', {})
  ipc.on('settings_save', null)
  ipc.on('project_root', (args) => args.path)
  ipc.on('files_list', (args) => listing({ dir: args.dir }))
  ipc.on('tracker_set_project', snapshot())
  ipc.on('tracker_probe', (args) => args.paths.map((path) => ({ path, tracked: true })))

  /* Обязательно: вотчер настроек встаёт только внутри loadSettings. Без него
     flushPending нечего дожимать, settings_save не случается никогда, и
     проверка порядка «запись до доски» сравнивала бы два -1, то есть
     проходила бы вхолостую. */
  await settings.loadSettings()
})

describe('basename', () => {
  it('режет по обоим разделителям: среди целевых вебвью есть WebView2', () => {
    expect(projects.basename('/home/кто-то/проект')).toBe('проект')
    expect(projects.basename('C:\\Users\\кто-то\\проект')).toBe('проект')
    expect(projects.basename('/проект/')).toBe('проект')
  })
})

describe('projectRows', () => {
  it('до ответа проверки строка считается отслеживаемой', () => {
    settings.settings.openProjects = ['/a']

    expect(projects.projectRows.value).toEqual([{ path: '/a', name: 'a', tracked: true }])
  })

  it('ответ проверки опускает флаг там, где трекера нет', async () => {
    settings.settings.openProjects = ['/a', '/b']
    ipc.on('tracker_probe', (args) =>
      args.paths.map((path) => ({ path, tracked: path === '/a' }))
    )

    await projects.refreshProbes()

    expect(projects.projectRows.value.map((row) => row.tracked)).toEqual([true, false])
  })
})

describe('switchTo', () => {
  it('переезжает: раскладка, дерево, вкладки, доска', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'

    await projects.switchTo('/b')

    expect(settings.settings.activeProject).toBe('/b')
    expect(ipc.calls('tracker_set_project')).toEqual([{ path: '/b' }])
    expect(ipc.calls('files_list').some((call) => call.dir === '')).toBe(true)
  })

  it('состояние уходящего проекта ложится на диск до того, как поменяется доска', async () => {
    settings.settings.activeProject = '/a'
    settings.settings.project.sideTab = 'agents'

    await projects.switchTo('/b')

    const commands = ipc.commands()
    expect(commands.indexOf('settings_save')).toBeLessThan(
      commands.indexOf('tracker_set_project')
    )
  })

  it('переезд на текущий проект ничего не делает', async () => {
    settings.settings.activeProject = '/a'

    await projects.switchTo('/a')

    expect(ipc.calls('tracker_set_project')).toHaveLength(0)
  })

  it('второй клик во время переезда игнорируется — победить должен не последний ответ', async () => {
    settings.settings.activeProject = '/a'

    const first = projects.switchTo('/b')
    const second = projects.switchTo('/c')
    await Promise.all([first, second])

    expect(ipc.calls('tracker_set_project')).toEqual([{ path: '/b' }])
    expect(settings.settings.activeProject).toBe('/b')
  })

  it('«человек передумал» отменяет переезд целиком', async () => {
    settings.settings.activeProject = '/a'
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'правка' }))
    tabs.onUnsaved(() => false)

    await projects.switchTo('/b')

    expect(settings.settings.activeProject).toBe('/a')
    expect(ipc.calls('tracker_set_project')).toHaveLength(0)
  })

  it('упавший переезд не запирает список навсегда', async () => {
    settings.settings.activeProject = '/a'
    ipc.fail('tracker_set_project', new Error('каталога нет'))
    await projects.switchTo('/b')

    ipc.on('tracker_set_project', snapshot())
    await projects.switchTo('/c')

    expect(settings.settings.activeProject).toBe('/c')
  })
})

describe('addProject', () => {
  it('добавляет выбранную папку и переезжает в неё', async () => {
    ipc.on('plugin:dialog|open', '/новый')

    await projects.addProject()

    expect(settings.settings.openProjects).toEqual(['/новый'])
    expect(settings.settings.activeProject).toBe('/новый')
  })

  it('подкаталог отслеживаемого репозитория нормализуется в корень один раз', async () => {
    ipc.on('plugin:dialog|open', '/репозиторий/src/stores')
    ipc.on('project_root', '/репозиторий')

    await projects.addProject()

    expect(settings.settings.openProjects).toEqual(['/репозиторий'])
    expect(ipc.calls('project_root')).toEqual([{ path: '/репозиторий/src/stores' }])
  })

  it('отмена диалога не трогает ничего', async () => {
    ipc.on('plugin:dialog|open', null)

    await projects.addProject()

    expect(settings.settings.openProjects).toEqual([])
    expect(ipc.calls('tracker_set_project')).toHaveLength(0)
  })

  it('упавший диалог не роняет стор', async () => {
    ipc.fail('plugin:dialog|open', new Error('диалог не открылся'))

    await expect(projects.addProject()).resolves.toBeUndefined()
    expect(settings.settings.openProjects).toEqual([])
  })

  it('выбор уже активного проекта не устраивает переезд и не спрашивает про вкладки', async () => {
    settings.settings.activeProject = '/a'
    settings.settings.openProjects = ['/a']
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'правка' }))
    const asked = vi.fn(() => true)
    tabs.onUnsaved(asked)
    ipc.on('plugin:dialog|open', '/a')

    await projects.addProject()

    expect(asked).not.toHaveBeenCalled()
    expect(ipc.calls('tracker_set_project')).toHaveLength(0)
    expect(settings.settings.openProjects).toEqual(['/a'])
  })

  it('уже открытый, но неактивный проект в списке не задваивается', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'
    ipc.on('plugin:dialog|open', '/b')

    await projects.addProject()

    expect(settings.settings.openProjects).toEqual(['/a', '/b'])
    expect(settings.settings.activeProject).toBe('/b')
  })
})

describe('removeProject', () => {
  it('активным становится следующий', async () => {
    settings.settings.openProjects = ['/a', '/b', '/c']
    settings.settings.activeProject = '/b'

    await projects.removeProject('/b')

    expect(settings.settings.openProjects).toEqual(['/a', '/c'])
    expect(settings.settings.activeProject).toBe('/c')
  })

  it('для последней строки — предыдущий', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/b'

    await projects.removeProject('/b')

    expect(settings.settings.activeProject).toBe('/a')
  })

  it('опустевший список оставляет окно без проекта — это нормальное состояние', async () => {
    settings.settings.openProjects = ['/a']
    settings.settings.activeProject = '/a'

    await projects.removeProject('/a')

    expect(settings.settings.openProjects).toEqual([])
    expect(settings.settings.activeProject).toBe(null)
  })

  it('про неактивную строку не спрашивает: «не сохранять» стёрло бы правки ни за что', async () => {
    settings.settings.openProjects = ['/a', '/b']
    settings.settings.activeProject = '/a'
    settings.settings.project.openTabs = ['a.txt']
    tabs.buffers.set('a.txt', buffer({ text: 'правка' }))
    const asked = vi.fn(() => true)
    tabs.onUnsaved(asked)

    await projects.removeProject('/b')

    expect(asked).not.toHaveBeenCalled()
    expect(settings.settings.openProjects).toEqual(['/a'])
    expect(settings.settings.activeProject).toBe('/a')
  })

  it('строки, которой нет, не касается', async () => {
    settings.settings.openProjects = ['/a']
    settings.settings.activeProject = '/a'

    await projects.removeProject('/нет')

    expect(settings.settings.openProjects).toEqual(['/a'])
    expect(settings.settings.activeProject).toBe('/a')
    expect(ipc.calls('tracker_set_project')).toHaveLength(0)
  })
})

describe('первый запуск', () => {
  it('активный проект, которого нет в списке, попадает в него', () => {
    settings.settings.activeProject = '/найденный'
    settings.settings.openProjects = []

    projects.adoptInitialProject()

    expect(settings.settings.openProjects).toEqual(['/найденный'])
  })

  it('пустой активный список пустым и оставляет', () => {
    settings.settings.activeProject = null

    projects.adoptInitialProject()

    expect(settings.settings.openProjects).toEqual([])
  })
})

describe('bd init', () => {
  it('успех обновляет доску и проверку каталогов', async () => {
    ipc.on('tracker_init', snapshot({ generation: 3 }))
    settings.settings.openProjects = ['/a']

    await projects.initActive()

    expect(ipc.calls('tracker_init')).toHaveLength(1)
    expect(ipc.calls('tracker_probe').length).toBeGreaterThan(0)
  })

  it('отказ проглочен: сообщение человеку уже показал тост', async () => {
    ipc.fail('tracker_init', new Error('bd init не отработал'))

    await expect(projects.initActive()).resolves.toBeUndefined()
  })
})
