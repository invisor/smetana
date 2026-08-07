/* In a browser there is no back end, and components still have to be checked
   (npm run dev, ?view=gallery). We install the official mockIPC so components
   know only invoke and listen and branch nowhere.

   This is a stub for browser mode, not a second back end: it answers read
   commands (snapshot/resync/health, settings) and stores nothing between calls.
   Writes to the tracker (tracker_update/close/reopen and anything else
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
/* One project per run-configuration state, because there are three of them and
   a browser is the only place any of them can be looked at. The third is the
   one that costs nothing to leave out and is worth the most: `broken` is the
   state with no board behind it and no gear on its row, so an omission there
   does not read as an omission — it reads as a project that is simply quiet. */
const MOCK_PROJECTS = ['/Users/you/dev/smetana', '/Users/you/dev/notes', '/Users/you/dev/holiday-curb']
/* Tracked is about `.beads/`, not about the run configuration: the damaged one
   is a fully tracked project whose board draws, which is exactly the case where
   nothing else on screen would say what is wrong. */
const UNTRACKED = '/Users/you/dev/notes'
const BROKEN_CONFIG_PROJECT = '/Users/you/dev/holiday-curb'

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

/* The task inspector draws only the fields an issue actually has, so a fixture
   that fills every one of them would hide the case it is meant to catch — a
   panel that reads as a form with empty rows. The index decides: every third
   issue carries a description, every other one an owner, and only closed ones
   carry a close reason and a closing time. That way ?view=gallery and the dev
   server show both a full inspector and a sparse one without anyone editing
   this file to see the second. */
function fixtureIssues() {
  const flat = fixtureColumns.flatMap((column) => column.tasks)
  return flat.map((task, i) => {
    const status = BD_STATUS[task.status] ?? task.status
    const closed = status === 'closed'
    return {
      id: task.id,
      title: task.title,
      status,
      updated_at: '2026-07-31T00:00:00Z',
      created_at: '2026-07-28T09:15:00Z',
      created_by: 'flexo',
      description:
        i % 3 === 0
          ? 'The watcher reports the failure and the sweep picks the work up on the next tick, so the board is stale rather than wrong. What is missing is a way to say so on screen.'
          : null,
      priority: (i % 4) + 1,
      // All six of bd's types plus a custom one, so the board in `npm run dev`
      // shows both halves of the type palette without anyone editing this file.
      issue_type: ['task', 'bug', 'feature', 'chore', 'epic', 'decision', 'tech-debt'][i % 7],
      owner: i % 2 === 0 ? 'merazent@gmail.com' : null,
      started_at: closed || status === 'in_progress' ? '2026-07-30T11:02:00Z' : null,
      closed_at: closed ? '2026-07-31T00:00:00Z' : null,
      close_reason: closed ? 'Delivered and merged into main' : null,
      comment_count: i % 5,
      dependency_count: (DEPENDENCY_EDGES[task.id] ?? []).length,
      dependent_count: 0,
      parent: task.spawnedFrom ?? null,
      labels: i % 3 === 1 ? ['tracker', 'ui'] : [],
      dependencies: (DEPENDENCY_EDGES[task.id] ?? []).map((dependsOnId) => ({
        issue_id: task.id,
        depends_on_id: dependsOnId,
        type: 'blocks'
      }))
    }
  })
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
      return MOCK_PROJECTS.map((path) => ({ path, tracked: path !== UNTRACKED }))
    }
    /* One project per state — set up, not set up, and damaged: without one of
       each there is nowhere to see any of them under npm run dev. The `ok` branch
       is the whole struct Rust serializes, defaults included, not just the
       fields something reads today — src-tauri/src/runs/config.rs's
       Defaults::default() is where target_branch/min_priority/
       max_parallel_tasks/review_passes come from, and repo/preflight/merge/
       live_check are its own empty-map and None. A narrower shape here would
       still work for every component that exists now and throw for the first
       one that reads config.defaults.target_branch, in the browser only. */
    if (command === 'project_config') {
      /* The parser's own message, caret line and all, because that is what the
         run dialog quotes verbatim — a tidied one-liner here would leave the
         only view of the real thing untested. */
      if (payload?.project === BROKEN_CONFIG_PROJECT) {
        return {
          state: 'broken',
          message:
            'TOML parse error at line 14, column 1\n' +
            '   |\n' +
            '14 | gate = ["npm test", "npm run build"]\n' +
            '   | ^^^^\n' +
            'unknown field `gate`, expected one of `setup`, `gates`, `env_files`\n'
        }
      }
      return payload?.project === MOCK_PROJECTS[0]
        ? {
            state: 'ok',
            config: {
              project: { repos: ['.'] },
              defaults: {
                target_branch: null,
                min_priority: 2,
                max_parallel_tasks: 3,
                review_passes: 5
              },
              repo: {},
              preflight: null,
              merge: null,
              live_check: null
            }
          }
        : { state: 'missing' }
    }
    /* No worker in a browser, so nothing is ever running. Answering null
       rather than rejecting: "is a run going here" is a read, and a read that
       throws would leave the panel unable to draw its ordinary empty state. */
    if (command === 'run_state') return null
    /* Enough branches for the dialog's field to be worth looking at. */
    if (command === 'git_branches') return ['main', 'staging', 'feature/runs-project-config']
    /* `attachment_import` and `attachment_write` are deliberately absent too,
       and their absence costs the browser nothing it could have had: there is
       no app data directory to copy an image into, so a thumbnail answered
       here would be a picture of a file that does not exist. The dialog shows
       the refusal in its own error line, which is the one place it would be
       looked for. */
    /* `run_start` and `run_stop` are deliberately absent: they fall through to
       the refusal at the bottom, like every other write. A run that looked like
       it had started would be worse than none — there is no worker, no session
       and no board behind it. */
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
          exitCode: null,
          /* Without this the row would be captioned "Agent", which is the
             honest answer for a session whose work is unknown and a useless
             one for the only session a browser has. An edit is the case worth
             showing: it is the one caption with both halves in it, prose and
             an issue id, set in different families. */
          work: { kind: 'editTask', id: 'smetana-42' }
        }
      ]
    }
    if (command === 'terminal_attach') {
      return { data: toBase64(MOCK_SESSION_OUTPUT), seq: 0 }
    }
    /* Detach and resize change nothing on disk and have nothing to lie
       about. */
    if (command === 'terminal_detach' || command === 'terminal_resize') return null
    // Any write command (tracker_update/close/reopen, files_write, and
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
