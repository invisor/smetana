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

  mockIPC((command) => {
    if (command === 'tracker_snapshot' || command === 'tracker_resync') return snapshot
    if (command === 'tracker_health') return { state: 'ok' }
    if (command === 'settings_load') return settingsDefaults()
    /* Настройки — не данные трекера: в браузере им негде храниться, и это не
       обман, а отсутствие места. Ронять запись здесь значило бы сыпать
       ошибками на каждое движение панели ради того, что и так очевидно:
       в браузере состояние не переживает перезагрузку. */
    if (command === 'settings_save') return null
    // Любая команда записи (tracker_create/update/close/reopen, и всё, что
    // появится позже) должна отклониться явно, а не молча вернуть похожую на
    // правду, но чужую задачу — иначе в браузере "запись" выглядела бы
    // рабочей, ничего не делая.
    throw new Error(
      `mockBackend: "${command}" is not implemented — this is a read-only stub for browser ` +
        'dev mode; writes to the tracker require the real Tauri backend (npm run tauri dev).'
    )
  }, { shouldMockEvents: true })

  return true
}
