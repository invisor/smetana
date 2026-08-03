/* В браузере бэкенда нет, а проверять компоненты нужно (npm run dev,
   ?view=gallery). Ставим официальный mockIPC, чтобы компоненты знали
   только invoke и listen и нигде не ветвились.

   Это заглушка для браузерного режима, а не второй бэкенд: она отвечает на
   команды чтения (snapshot/resync/health, настройки) и ничего не хранит между
   вызовами. Команды записи в трекер (tracker_create/update/close/reopen и
   любая другая, которой здесь нет) должны с треском проваливаться, а не
   отвечать правдоподобной, но вымышленной задачей — иначе в браузере "запись"
   выглядела бы рабочей, молча не делая ничего. */
import { mockIPC } from '@tauri-apps/api/mocks'
import { columns as fixtureColumns } from '../views/desktopAppData.js'
import { defaults as settingsDefaults } from './settings.js'
import { MOCK_SESSION_OUTPUT } from './terminalFixture.js'

/* Обратный перевод: фикстуры написаны в терминах дизайн-системы,
   а бэкенд отдаёт статусы bd. */
const BD_STATUS = { ready: 'open', running: 'in_progress', done: 'closed' }

const COLUMN_CATEGORY = {
  open: 'active',
  in_progress: 'wip',
  blocked: 'wip',
  'needs-you': 'wip',
  'awaiting-review': 'wip',
  closed: 'done'
}

/* Фикстура в desktopAppData.js задаёт blockedBy/blocks как независимые числа
   на карточку — так их и рисовал React-прототип, но это не согласованный
   граф: сумма всех "blocks" (5+1+2=8) не равна сумме всех "blockedBy" (2),
   а в реальном графе зависимостей они обязаны совпадать (это одни и те же
   рёбра, посчитанные с двух concов). Здесь единственная пара, которую можно
   выразить рёбрами между существующими карточками, не заводя фиктивную
   задачу и не приписывая чужой карточке лишний blockedBy, — то, что bd-77e1
   заблокирована bd-a1b2 и bd-7f31 (это же и её spawnedFrom-родитель).
   Остальные "blocks" со стороны bd-a1b2/bd-3c9d/bd-7f31 в моке недостижимы:
   см. task-8-report.md. */
const DEPENDENCY_EDGES = {
  'bd-77e1': ['bd-a1b2', 'bd-7f31']
}

/* В браузере проектов два, чтобы список в панели было на чём смотреть.
   Первый — «настоящий», второй — без трекера: без него пометку
   «здесь нет bd» негде увидеть в npm run dev. */
const MOCK_PROJECTS = ['/Users/you/dev/smetana', '/Users/you/dev/notes']

/* Дерево, которое раньше лежало в views/desktopAppData.js. Настоящее дерево
   приходит с диска, но в браузере диска нет, а Gallery нужно на чём-то
   показывать FileTree. Форма — ответы files_list: путь каталога → его записи. */
export const MOCK_TREE = {
  '': [
    { name: 'src', path: 'src', kind: 'dir' },
    { name: 'Cargo.toml', path: 'Cargo.toml', kind: 'file' }
  ],
  src: [
    { name: 'agent.rs', path: 'src/agent.rs', kind: 'file' },
    { name: 'scratch.rs', path: 'src/scratch.rs', kind: 'file' },
    { name: 'tabs.rs', path: 'src/tabs.rs', kind: 'file' },
    { name: 'worktree.rs', path: 'src/worktree.rs', kind: 'file' }
  ]
}

const MOCK_FILE = `fn main() {\n    println!("hello from the mock backend");\n}\n`
const MOCK_MTIME = 1754006400000

/* PTY output is arbitrary bytes; the fixture's box-drawing characters sit
   outside Latin-1, so plain btoa() would throw. Route through TextEncoder
   first, to get from this fixture's JS string to the UTF-8 bytes a PTY would
   have produced. The Rust side has no equivalent step and needs none — it
   holds those bytes already and base64-encodes them directly; the encoding
   only exists here because the fixture starts life as text. */
const toBase64 = (text) => btoa(String.fromCharCode(...new TextEncoder().encode(text)))

function fixtureIssues() {
  return fixtureColumns.flatMap((column) =>
    column.tasks.map((task) => ({
      id: task.id,
      title: task.title,
      status: BD_STATUS[task.status] ?? task.status,
      updated_at: '2026-07-31T00:00:00Z',
      priority: 2,
      issue_type: 'task',
      assignee: null,
      parent: task.spawnedFrom ?? null,
      labels: [],
      dependencies: (DEPENDENCY_EDGES[task.id] ?? []).map((dependsOnId) => ({
        issue_id: task.id,
        depends_on_id: dependsOnId,
        type: 'blocks'
      }))
    }))
  )
}

export function installMockBackend() {
  if (window.__TAURI_INTERNALS__) return false

  const issues = fixtureIssues()
  const columns = fixtureColumns.map((c) => {
    const name = BD_STATUS[c.status] ?? c.status
    return { name, category: COLUMN_CATEGORY[name] ?? 'wip' }
  })
  const snapshot = { generation: 1, columns, issues }

  mockIPC((command, payload) => {
    if (command === 'tracker_snapshot' || command === 'tracker_resync') return snapshot
    if (command === 'tracker_health') return { state: 'ok' }
    if (command === 'settings_load') {
      /* project — «прочитай состояние вот этого проекта»: настоящий бэкенд
         отвечает на него, и заглушка обязана тоже, иначе переключение
         в браузере нельзя было бы посмотреть — подсветка активной строки
         откатывалась бы обратно после каждого клика. */
      return {
        ...settingsDefaults(),
        openProjects: MOCK_PROJECTS,
        activeProject: payload?.project ?? MOCK_PROJECTS[0]
      }
    }
    /* Настройки — не данные трекера: в браузере им негде храниться, и это не
       обман, а отсутствие места. Ронять запись здесь значило бы сыпать
       ошибками на каждое движение панели ради того, что и так очевидно:
       в браузере состояние не переживает перезагрузку. */
    if (command === 'settings_save') return null
    if (command === 'tracker_set_project') return snapshot
    if (command === 'tracker_probe') {
      return MOCK_PROJECTS.map((path) => ({ path, tracked: path === MOCK_PROJECTS[0] }))
    }
    /* Про файловую систему заглушка не знает ничего и выдумывать не станет:
       путь возвращается как есть. Настоящий бэкенд поднялся бы отсюда до
       корня отслеживаемого репозитория, и это единственное, чем ответ в
       браузере отличается от ответа в приложении. */
    if (command === 'project_root') return payload?.path ?? null
    /* Выбрать папку в браузере нечем. Отвечаем как отменённый диалог: это
       отказ, а не выдуманный путь — тем же правилом, по которому здесь
       отклоняются записи в трекер. */
    if (command === 'plugin:dialog|open') {
      console.info('[mockBackend] выбор папки в браузере недоступен — диалог считается отменённым')
      return null
    }
    if (command === 'files_list') {
      const dir = payload?.dir ?? ''
      /* Каталога, которого нет в фикстуре, в браузере не бывает: отвечаем
         пустым списком, а не отказом — так дерево остаётся кликабельным. */
      return { dir, entries: MOCK_TREE[dir] ?? [], truncated: 0 }
    }
    if (command === 'files_read') {
      return { path: payload?.path ?? '', text: MOCK_FILE, mtime: MOCK_MTIME }
    }
    /* Ничего не менялось: в браузере файлам меняться неоткуда. */
    if (command === 'files_stat') {
      return (payload?.paths ?? []).map((path) => ({ path, mtime: MOCK_MTIME }))
    }
    if (command === 'terminal_list') {
      return [
        {
          id: 1,
          agent: 'claude',
          cwd: MOCK_PROJECTS[0],
          project: payload?.project ?? MOCK_PROJECTS[0],
          state: 'needs-you',
          question: {
            text: 'Do you want to make this edit to tabs.js?',
            options: [
              { label: 'Yes', send: '1\r' },
              { label: "Yes, and don't ask again this session", send: '2\r' },
              { label: 'No, and tell Claude what to do differently', send: '3\r' }
            ],
            selected: 0
          },
          startedAt: new Date(Date.now() - 134 * 60000).toISOString(),
          exitCode: null
        }
      ]
    }
    if (command === 'terminal_attach') {
      return { data: toBase64(MOCK_SESSION_OUTPUT), seq: 0 }
    }
    /* Detach and resize change nothing on disk and have nothing to lie
       about. */
    if (command === 'terminal_detach' || command === 'terminal_resize') return null
    // Любая команда записи (tracker_create/update/close/reopen, files_write,
    // и всё, что появится позже) должна отклониться явно, а не молча вернуть
    // похожую на правду, но чужую задачу — иначе в браузере "запись"
    // выглядела бы рабочей, ничего не делая.
    throw new Error(
      `mockBackend: "${command}" is not implemented — this is a read-only stub for browser ` +
        'dev mode; writes to the tracker require the real Tauri backend (npm run tauri dev).'
    )
  }, { shouldMockEvents: true })

  return true
}
