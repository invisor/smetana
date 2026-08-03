/* Формы повторяют модели Rust дословно: src-tauri/src/tracker/model.rs и
   src-tauri/src/files/model.rs. Поля задачи — snake_case (updated_at,
   issue_type): rename_all на структуре нет. */

export const issue = (over = {}) => ({
  id: 'bd-1',
  title: 'Задача',
  status: 'open',
  updated_at: '2026-08-03T10:00:00Z',
  priority: 2,
  issue_type: 'task',
  assignee: null,
  parent: null,
  labels: [],
  dependencies: [],
  ...over
})

/* Ребро зависимости. bd отдаёт только исходящие: issue_id зависит от
   depends_on_id. */
export const edge = (over = {}) => ({
  issue_id: 'bd-2',
  depends_on_id: 'bd-1',
  type: 'blocks',
  ...over
})

export const snapshot = (over = {}) => ({
  generation: 1,
  columns: [
    { name: 'open', category: 'active' },
    { name: 'in_progress', category: 'wip' },
    { name: 'closed', category: 'done' }
  ],
  issues: [],
  ...over
})

export const delta = (over = {}) => ({
  generation: 2,
  upserted: [],
  removed: [],
  ...over
})

export const entry = (over = {}) => ({
  name: 'a.txt',
  path: 'a.txt',
  kind: 'file',
  ...over
})

export const listing = (over = {}) => ({
  dir: '',
  entries: [],
  truncated: 0,
  ...over
})

export const fileText = (over = {}) => ({
  path: 'a.txt',
  text: 'исходный',
  mtime: 10,
  ...over
})

/* Буфер вкладки в том виде, в каком его держит tabs.js. Нужен там, где тест
   ставит буфер напрямую, не проходя через чтение с диска. */
export const buffer = (over = {}) => ({
  text: 'исходный',
  original: 'исходный',
  mtime: 10,
  error: null,
  saveError: null,
  stale: false,
  loading: false,
  ...over
})
