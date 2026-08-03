/* In a browser there is no back end, and components still have to be checked
   (npm run dev, ?view=gallery). We install the official mockIPC so components
   know only invoke and listen and branch nowhere.

   This is a stub for browser mode, not a second back end: it answers read
   commands (snapshot/resync/health, settings) and stores nothing between calls.
   Writes to the tracker (tracker_create/update/close/reopen and anything else
   not listed here) have to fail loudly rather than answer with a plausible but
   invented issue — otherwise a "write" in the browser would look like it worked
   while silently doing nothing. */
import { mockIPC } from '@tauri-apps/api/mocks'
import { columns as fixtureColumns } from '../views/desktopAppData.js'
import { defaults as settingsDefaults } from './settings.js'
import { MOCK_SESSION_OUTPUT } from './terminalFixture.js'

/* The reverse translation: the fixtures are written in the design system's
   terms, while the back end returns bd's statuses. */
const BD_STATUS = { ready: 'open', running: 'in_progress', done: 'closed' }

const COLUMN_CATEGORY = {
  open: 'active',
  in_progress: 'wip',
  blocked: 'wip',
  'needs-you': 'wip',
  'awaiting-review': 'wip',
  closed: 'done'
}

/* The fixture in desktopAppData.js sets blockedBy/blocks as independent
   numbers per card — that is how the React prototype drew them, but it is not a
   consistent graph: the sum of all "blocks" (5+1+2=8) does not equal the sum of
   all "blockedBy" (2), while in a real dependency graph they have to match
   (they are the same edges counted from both ends). The only pair expressible
   as edges between existing cards here, without inventing a fictional issue or
   attributing an extra blockedBy to somebody else's card, is that bd-77e1 is
   blocked by bd-a1b2 and bd-7f31 (the latter being its spawnedFrom parent too).
   The remaining "blocks" on the bd-a1b2/bd-3c9d/bd-7f31 side are unreachable in
   the mock: see task-8-report.md. */
const DEPENDENCY_EDGES = {
  'bd-77e1': ['bd-a1b2', 'bd-7f31']
}

/* There are two projects in the browser so that the list in the panel has
   something to show. The first is the "real" one, the second has no tracker:
   without it there is nowhere to see the "no bd here" mark under
   npm run dev. */
const MOCK_PROJECTS = ['/Users/you/dev/smetana', '/Users/you/dev/notes']

/* The tree that used to live in views/desktopAppData.js. The real tree comes
   from disk, but a browser has no disk and Gallery needs something to show
   FileTree with. The shape is files_list's answers: a directory's path → its
   entries. */
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
      /* project means "read this project's state": the real back end answers
         it, and the stub has to as well, otherwise switching could not be seen
         in the browser — the active row's highlight would roll back after every
         click. */
      return {
        ...settingsDefaults(),
        openProjects: MOCK_PROJECTS,
        activeProject: payload?.project ?? MOCK_PROJECTS[0]
      }
    }
    /* Settings are not tracker data: there is nowhere for them to live in a
       browser, and that is an absence of somewhere to put them, not a
       deception. Failing the write here would mean spraying errors on every
       panel movement over something already obvious: state does not survive a
       reload in a browser. */
    if (command === 'settings_save') return null
    if (command === 'tracker_set_project') return snapshot
    if (command === 'tracker_probe') {
      return MOCK_PROJECTS.map((path) => ({ path, tracked: path === MOCK_PROJECTS[0] }))
    }
    /* The stub knows nothing about the filesystem and will not invent
       anything: the path comes back as is. The real back end would climb from
       here to the tracked repository's root, and that is the only way the
       browser's answer differs from the app's. */
    if (command === 'project_root') return payload?.path ?? null
    /* There is nothing to pick a folder with in a browser. We answer as a
       cancelled dialog would: a refusal, not an invented path — by the same
       rule that rejects writes to the tracker here. */
    if (command === 'plugin:dialog|open') {
      console.info('[mockBackend] picking a folder is unavailable in a browser — the dialog counts as cancelled')
      return null
    }
    if (command === 'files_list') {
      const dir = payload?.dir ?? ''
      /* A directory absent from the fixture does not exist in the browser: we
         answer with an empty list rather than a refusal — that keeps the tree
         clickable. */
      return { dir, entries: MOCK_TREE[dir] ?? [], truncated: 0 }
    }
    if (command === 'files_read') {
      return { path: payload?.path ?? '', text: MOCK_FILE, mtime: MOCK_MTIME }
    }
    /* Nothing changed: there is nowhere for files to change in a browser. */
    if (command === 'files_stat') {
      return (payload?.paths ?? []).map((path) => ({ path, mtime: MOCK_MTIME }))
    }
    /* The branch is a read, and in a browser there is nowhere for it to come
       from but a fixture. The answer's shape is the real command's: a branch or
       a detached HEAD. */
    if (command === 'git_head') {
      return { branch: 'feat/worktree-rename', detached: null }
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
    // Any write command (tracker_create/update/close/reopen, files_write, and
    // whatever appears later) has to reject explicitly rather than silently
    // return a plausible but foreign issue — otherwise a "write" in the browser
    // would look like it worked while doing nothing.
    throw new Error(
      `mockBackend: "${command}" is not implemented — this is a read-only stub for browser ` +
        'dev mode; writes to the tracker require the real Tauri backend (npm run tauri dev).'
    )
  }, { shouldMockEvents: true })

  return true
}
