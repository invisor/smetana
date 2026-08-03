/* The shapes mirror the Rust models word for word: src-tauri/src/tracker/model.rs
   and src-tauri/src/files/model.rs. An issue's fields are snake_case
   (updated_at, issue_type): the struct has no rename_all. */

export const issue = (over = {}) => ({
  id: 'bd-1',
  title: 'An issue',
  status: 'open',
  updated_at: '2026-08-03T10:00:00Z',
  created_at: '2026-08-01T09:00:00Z',
  created_by: 'flexo',
  description: null,
  priority: 2,
  issue_type: 'task',
  // bd's own name for it — the struct calls it `owner` because bd does.
  owner: null,
  started_at: null,
  closed_at: null,
  close_reason: null,
  comment_count: 0,
  dependency_count: 0,
  dependent_count: 0,
  parent: null,
  labels: [],
  dependencies: [],
  ...over
})

/* A dependency edge. bd gives only the outgoing ones: issue_id depends on
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
  text: 'original',
  mtime: 10,
  ...over
})

/* A tab's buffer in the shape tabs.js holds it. Needed where a test sets a
   buffer directly rather than going through a read from disk. */
export const buffer = (over = {}) => ({
  text: 'original',
  original: 'original',
  mtime: 10,
  error: null,
  saveError: null,
  stale: false,
  loading: false,
  ...over
})
