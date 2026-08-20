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
   entries.

   The names are deliberately of several kinds rather than four `.rs` files:
   `src/catppuccinIcon.js` draws a row by its name, and a fixture of one kind would
   show one glyph — so the tree that used to prove FileTree renders now also
   shows whether the whole vocabulary does. */
export const MOCK_TREE = {
  '': [
    { name: 'src', path: 'src', kind: 'dir' },
    { name: '.gitignore', path: '.gitignore', kind: 'file' },
    { name: 'Cargo.toml', path: 'Cargo.toml', kind: 'file' },
    { name: 'LICENSE', path: 'LICENSE', kind: 'file' },
    { name: 'README.md', path: 'README.md', kind: 'file' },
    { name: 'tauri.conf.json', path: 'tauri.conf.json', kind: 'file' }
  ],
  src: [
    { name: 'agent.rs', path: 'src/agent.rs', kind: 'file' },
    { name: 'app-icon.png', path: 'src/app-icon.png', kind: 'file' },
    { name: 'bd-aarch64.tar.gz', path: 'src/bd-aarch64.tar.gz', kind: 'file' },
    { name: 'scratch.rs', path: 'src/scratch.rs', kind: 'file' },
    { name: 'tabs.rs', path: 'src/tabs.rs', kind: 'file' },
    { name: 'unknown-binary', path: 'src/unknown-binary', kind: 'file' },
    { name: 'worktree.rs', path: 'src/worktree.rs', kind: 'file' }
  ]
}

/* The entries of the tree above that a real `files_read` would refuse. Named
   rather than sniffed: a fixture has no bytes to look at. */
const MOCK_BINARY = new Set(['src/app-icon.png', 'src/bd-aarch64.tar.gz', 'src/unknown-binary'])

const MOCK_FILE = `fn main() {\n    println!("hello from the mock backend");\n}\n`
const MOCK_MTIME = 1754006400000

/* The same file one commit ago, for the Git panel's diff. It differs from
   `MOCK_FILE` in one line rather than wholesale, since what a diff is for is
   showing the difference and a fixture with nothing in common between its two
   sides would draw two solid blocks of colour. `notes/todo.txt` is untracked in
   the status fixture below, so it answers `null` — the added-file case, which
   is the one the empty left column exists for. */
const MOCK_FILE_AT_HEAD = `fn main() {\n    println!("hello");\n}\n`
const MOCK_UNTRACKED = 'notes/todo.txt'

/* PTY output is arbitrary bytes; the fixture's box-drawing characters sit
   outside Latin-1, so plain btoa() would throw. Route through TextEncoder
   first, to get from this fixture's JS string to the UTF-8 bytes a PTY would
   have produced. The Rust side has no equivalent step and needs none — it
   holds those bytes already and base64-encodes them directly; the encoding
   only exists here because the fixture starts life as text. */
const toBase64 = (text) => btoa(String.fromCharCode(...new TextEncoder().encode(text)))

/* A distinct closing time per closed fixture, and deliberately out of step with
   the order the fixtures are written in: the done column is ordered on this
   field (`components/kanban/cardOrder.js`), so the one date every closed issue
   used to share left the rule with nothing to do and nothing to see. Computed
   rather than tabulated, because a table would have to be extended by whoever
   adds a task to `views/desktopAppData.js` and would not be; 5 and 12 are
   coprime with the hours, 17 with the minutes, so no two indices land on the
   same stamp anywhere near this many fixtures. It stays inside the afternoon
   and evening of the 30th, between the `started_at` and the `updated_at` every
   fixture carries, so the inspector never shows one closed before it was picked
   up. */
const fixtureClosedAt = (i) => {
  const hour = String(12 + ((i * 5) % 12)).padStart(2, '0')
  const minute = String((i * 17) % 60).padStart(2, '0')
  return `2026-07-30T${hour}:${minute}:00Z`
}

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
      /* The prose fields ride with the description so the same issues show a
         full inspector and the rest a sparse one. The note is two lines on
         purpose — every `bd note` appends, and the panel owes the whole log. */
      acceptance_criteria:
        i % 3 === 0
          ? 'The failure is visible on screen while it lasts and gone when the sweep catches up.'
          : null,
      design:
        i % 3 === 0
          ? 'A quiet notice over the board rather than in place of it: the cards stay readable while the app says the data may be stale.'
          : null,
      /* A parked issue carries its questions whatever its index says: the card
         menu, the greyed play and the Ready warning all hang off this status,
         and the dialog quotes these very lines. Leaving it to `i % 3` would
         make the one fixture the feature exists for depend on where its column
         happened to land in the list. */
      notes:
        status === 'parked'
          ? 'parked: needs a decision on where the strip sits\nparked: still waiting on the design call'
          : i % 3 === 0
            ? 'parked: needs a decision on the storage format\nresolved: sqlite, decided on 2026-08-01'
            : null,
      priority: (i % 4) + 1,
      // All six of bd's types plus a custom one, so the board in `npm run dev`
      // shows both halves of the type palette without anyone editing this file.
      issue_type: ['task', 'bug', 'feature', 'chore', 'epic', 'decision', 'tech-debt'][i % 7],
      owner: i % 2 === 0 ? 'merazent@gmail.com' : null,
      /* Only what a run has claimed carries one, which is what bd does: a
         `--claim` writes the actor into `assignee` and leaves `owner` alone
         (smetana-a5b). So an in_progress fixture shows the inspector's Assignee
         row holding a run actor beside an owner who is a person, and everything
         else shows the row absent. */
      assignee: status === 'in_progress' ? 'smetana-run-7' : null,
      started_at: closed || status === 'in_progress' ? '2026-07-30T11:02:00Z' : null,
      closed_at: closed ? fixtureClosedAt(i) : null,
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

/* Whether the fixtures are what is answering. `window.__TAURI_INTERNALS__` is
   **not** the way to ask that question from anywhere else in the app: `mockIPC`
   sets that very property itself (`mocks.js` calls `mockInternals`), so it is
   true in a browser too, and code that read it as "there is a back end" got the
   dev server exactly backwards. This flag is the decision made below, published
   so nobody has to guess at it. */
let mocked = false
export const usingMockBackend = () => mocked

export function installMockBackend() {
  if (window.__TAURI_INTERNALS__) return false
  mocked = true

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
    /* Making a window is Tauri's, not the front end's, so a browser has nothing
       to do here and the gear is a no-op. Answered rather than refused, and that
       is the difference from a write: nothing was promised and nothing was lost
       — the settings UI is reachable in a browser at `?view=settings`, which is
       how it is checked by eye. */
    if (command === 'settings_window_open') {
      console.info('[mockBackend] a second window needs Tauri — open ?view=settings instead')
      return null
    }
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
              /* Browser, not null, and this is the line that makes the
                 fixture below reachable. `liveCheckBlock` is scoped to
                 mode = "browser", so while this was null the blocked toggle
                 could not be produced under `npm run dev` at all — the machine
                 answer was read, found nothing, and was then thrown away by a
                 mode that never matched. Since `?view=gallery` passes the
                 blocked string to RunModal as a literal, that left the
                 DesktopApp computed, the live_check.mode accessor and the prop
                 hand-off covered by nothing anywhere: a typo in the accessor
                 would have shipped in silence. */
              live_check: { mode: 'browser', command: null, notes: null }
            }
          }
        : { state: 'missing' }
    }
    /* One run, working, holding the session whose `work.kind` is `run` in
       `terminal_list`. A read rather than a rejection, the way it always was —
       "which runs are going here" is a question, and one that threw would leave
       the panel unable to draw at all — but the answer is no longer the empty
       set, and that is smetana-a5b's doing.

       The whole visible form of that bug is an agent row captioned "Agent" where
       the ids the run claimed belong, and `claimedBy` in terminals.js
       reconstructs those ids from two halves: a run naming the session that is
       working, and the tracker's in_progress issues naming their `assignee`.
       With no run on this side the reconstruction had nothing to start from, so
       the one case the bug was about was unreachable in a browser and could only
       ever be looked at in the real app.

       The session id here, the id in `terminal_list` and the `assignee` on the
       in_progress fixture issues are one fact written in three places
       (`smetana-run-7`, the shape `run_actor` mints). Drift between any two of
       them costs the caption — which is exactly the symptom — so they are worth
       checking together.

       Only in the one project `project_config` above calls set up, and the other
       projects keep the empty answer they always had. A run bar drawn over a
       project the very same stub reports as unconfigured would put two
       contradictory things on screen at once — `needsSetup` offering to set the
       project up, under a bar saying a batch is already merging into a branch.

       A stop reaches `run_stop`, which is absent and therefore refused like every
       other write here. That is the same bargain the tracker's writes take. */
    if (command === 'run_state') {
      if (payload?.project && payload.project !== MOCK_PROJECTS[0]) return []
      return [
        {
          token: 1,
          project: payload?.project ?? MOCK_PROJECTS[0],
          settings: {
            scope: { kind: 'queue' },
            mode: 'supervised',
            target_branch: 'develop',
            create_target: false,
            min_priority: 2,
            max_parallel_tasks: 2,
            live_check: true,
            file_findings: true
          },
          state: { kind: 'working', iteration: 0 },
          session: 7,
          /* The same id, because a run that is working is working in it. The
             two only part company at the ending, where `session` is cleared and
             this is what `reportDelivery.js` reads. */
          last_session: 7,
          batches: 1,
          stopping: false,
          reduced: null
        }
      ]
    }
    /* A machine with neither tool, deliberately, and it is the one choice here
       that is not simply "what the developer's laptop has". Every machine that
       runs `npm run dev` on this project already has Playwright and the
       extension, so answering what the real command would answer would put the
       blocked toggle out of reach in a browser — and a control that only appears
       on somebody else's laptop is a control nobody checks. The absence is also
       honest for the browser itself: there is no Rust here to drive anything
       with.

       What this achieves only holds together with the `live_check` fixture in
       `project_config` above: the two are one fixture in two halves, and either
       on its own leaves the blocked toggle unreachable. Opening "Run the queue"
       in the dev server is what exercises the DesktopApp computed, the
       live_check.mode accessor and the prop hand-off — none of which any gate
       reaches.

       Busy-ness stays null, and would be inert even if it were not: the busy
       branch only fires where Playwright is the tool that would be used, and
       neither tool is here. */
    if (command === 'browser_tools') {
      return {
        playwright_mcp: false,
        playwright_browsers: false,
        extension: false,
        busy_project: null
      }
    }
    /* Enough branches for the dialog's field to be worth looking at, and one of
       them short of a repository — the browser is the only place the lower
       group and its notes can be seen at all, since there is no Rust worker
       here to walk anybody's repositories. */
    if (command === 'target_branches') {
      return [
        { name: 'main', missing_in: [] },
        { name: 'staging', missing_in: [] },
        { name: 'feature/runs-project-config', missing_in: [] },
        { name: 'release/7', missing_in: ['admin', 'extension'] }
      ]
    }
    /* The Storage tab's numbers. A read, so it answers — otherwise the section
       could not be looked at under `npm run dev` at all, and the one place in
       the app that deletes anything would be the one screen nobody could see.
       The shape is `attachments::Survey` in full: a store bigger than the
       active project's share of it, some of that share still in use and some of
       it not, which is the state the button is drawn for.

       The same numbers are what the bell weighs, and the project's share of the
       store — `kept` and `removable` together, 15.25 MiB — is deliberately over
       the first threshold: that is what makes the notification panel visible in
       `npm run dev` at all, with no Rust worker to grow a folder behind it. It
       announces once per page load and no more, because a browser has nowhere
       to keep the threshold it just announced (`settings_save` is accepted and
       dropped here) — so the card comes back on every reload, which is the one
       way this fixture's behaviour differs from the app's.

       `attachments_clean` is deliberately absent and falls through to the loud
       refusal at the bottom, like every other write. There is no store to
       delete from in a browser, and a deletion that looked like it had happened
       would be the worst of them: the person would believe their pictures were
       gone. The refusal in the section's own error line is the honest answer,
       and it is a thing worth seeing by eye. */
    if (command === 'attachments_survey') {
      return {
        store: { files: 14, bytes: 22 * 1024 * 1024 + 512 * 1024 },
        project: MOCK_PROJECTS[0],
        /* `tracker_health` above answers `ok`, and this has to agree: the two
           are one fact in Rust, where the board and its health leave the worker
           in the same message. A fixture claiming a healthy board here and a
           broken one there would show a state the app cannot produce. */
        board: 'ok',
        kept: { files: 5, bytes: 6 * 1024 * 1024 },
        removable: { files: 6, bytes: 9 * 1024 * 1024 + 256 * 1024 }
      }
    }
    /* `attachment_import` and `attachment_write` are deliberately absent too,
       and their absence costs the browser nothing it could have had: there is
       no app data directory to copy an image into, so a thumbnail answered
       here would be a picture of a file that does not exist. The dialog shows
       the refusal in its own error line, which is the one place it would be
       looked for. */
    /* `run_start` and `run_stop` are deliberately absent: they fall through to
       the refusal at the bottom, like every other write. A run that looked like
       it had started would be worse than none — there is no worker, no session
       and no board behind it. Reading what a run is *doing* is a different
       question, and `run_state` above answers it. */
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
      const path = payload?.path ?? ''
      /* The three fixture files that are not text refuse the way the real
         backend refuses them, and the refusal is by name because there are no
         bytes here to sniff. Without it, clicking the png in the dev tree opens
         a tab of Rust source: the tree would be drawing a picture's glyph over
         a file the app claims to have read as text, which is the one thing this
         fixture was extended to make visible. */
      if (MOCK_BINARY.has(path)) {
        throw { kind: 'binary', message: `mockBackend: ${path} is not text` }
      }
      return { path, text: MOCK_FILE, mtime: MOCK_MTIME }
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
    /* The Git panel's reads. A browser has no git to run, so all of them answer
       from a fixture — two repositories rather than one, since a project made
       of several is the case the repository list exists for and the case a
       single-repository machine can never show. */
    if (command === 'vcs_repos') {
      const project = payload?.project ?? MOCK_PROJECTS[0]
      return [
        { name: '.', path: project, branch: 'feat/worktree-rename', detached: null },
        { name: 'admin', path: `${project}/admin`, branch: null, detached: 'a1b2c3d' }
      ]
    }
    /* The left-hand side of a diff. A browser has no git, so one fixture stands
       for every tracked file and the untracked one answers `null` — the two
       answers this command has, and the second is not a failure. */
    if (command === 'vcs_file_at_head') {
      return payload?.path === MOCK_UNTRACKED ? null : MOCK_FILE_AT_HEAD
    }
    /* One of each kind the panel draws, including a rename with its original
       path and a conflict — the loud row, which is the one worth being able to
       look at with no worker behind it. */
    if (command === 'vcs_status') {
      return {
        branch: 'feat/worktree-rename',
        detached: null,
        changes: [
          { path: 'src/stores/vcs.js', origPath: null, kind: 'modified', staged: false, unstaged: true },
          { path: 'src/components/git/GitPanel.vue', origPath: null, kind: 'added', staged: true, unstaged: false },
          { path: 'src/views/desktopAppData.js', origPath: null, kind: 'deleted', staged: true, unstaged: false },
          {
            path: 'src/components/git/RepoList.vue',
            origPath: 'src/components/shell/RepoList.vue',
            kind: 'renamed',
            staged: true,
            unstaged: false
          },
          { path: MOCK_UNTRACKED, origPath: null, kind: 'untracked', staged: false, unstaged: true },
          { path: 'src/stores/tabs.js', origPath: null, kind: 'conflicted', staged: false, unstaged: true }
        ]
      }
    }
    /* The commit-message button. A read like the three above — it runs `git
       diff` and a model, and changes nothing — so it answers here rather than
       falling through to the refusal, which is what lets the field, the
       spinner and the button be checked in a browser at all. `vcs_commit` is
       deliberately **not** here and falls through with every other write: a
       commit that looked as though it had happened would be the worst kind of
       lie, since there is no working tree behind any of this.

       The sentence is about the fixture changes above, and it is written the
       way the prompt asks for one so that what a person sees in the browser is
       the shape the real thing returns. */
    if (command === 'vcs_suggest_message') {
      return 'feat: add a commit box to the Git panel'
    }
    /* The branch list, in the order `git::by_recency` would have given it: the
       branch worked on most recently first, the tail alphabetical. The current
       one is deliberately not the first, since a list where the two coincide
       could not show that the mark and the order are two different facts.
       `vcs_checkout`, `vcs_merge`, `vcs_rebase` and `vcs_abort` are absent on
       purpose and fall through to the refusal at the bottom, like every other
       write: a merge that looked like it had happened would be the worst kind,
       since nothing here has a working tree to have changed. Which leaves the
       conflict dialog reachable in a browser only through `?view=gallery`,
       where it has four frames of its own. */
    if (command === 'vcs_branches') {
      return [
        { name: 'develop', current: false },
        { name: 'feat/worktree-rename', current: true },
        { name: 'main', current: false },
        { name: 'release/7', current: false }
      ]
    }
    /* Where those branches stand against their upstreams, which is the one read
       of this panel whose answer nothing on disk could give: it is a process,
       and in a browser it is this. One branch behind, one ahead, one level with
       its upstream and one nobody has pushed — so every mark a row can carry is
       reachable in `npm run dev`, where they would otherwise be a feature only
       the gallery has ever drawn.

       The caption's two buttons are **not**, beyond the one state the branch
       this mock is on happens to be in: Pull live with three to bring in and
       Push refused with nothing to send. The current branch is fixed at
       `feat/worktree-rename` and `vcs_checkout` falls through to the refusal
       below with every other write, so no browser can put this panel on
       `develop`, `main` or `release/7` and see what the pair says there. The
       other three states are the gallery's, which draws all of them side by
       side.

       `vcs_fetch` is deliberately **not** here, and it is not a write either: it
       falls through to the refusal at the bottom because a browser has no
       remote to ask, and what that produces is the behaviour this store
       promises for a machine with no network — one line in the console and
       nothing at all on screen. Which makes the silent half checkable here too.
       `vcs_pull` and `vcs_push` are absent with the other writes. */
    if (command === 'vcs_tracking') {
      return [
        { branch: 'develop', upstream: 'origin/develop', ahead: 2, behind: 0, gone: false },
        {
          branch: 'feat/worktree-rename',
          upstream: 'origin/feat/worktree-rename',
          ahead: 0,
          behind: 3,
          gone: false
        },
        { branch: 'main', upstream: 'origin/main', ahead: 0, behind: 0, gone: false },
        { branch: 'release/7', upstream: null, ahead: 0, behind: 0, gone: false }
      ]
    }
    if (command === 'terminal_list') {
      return [
        /* The filing agent comes first deliberately: `loadSessions` repairs an
           empty selection to the *last* session in this list, and the one worth
           landing on is the one waiting on a person — it is the only way the
           `needs-you` triangle can be seen with no worker behind it. Picking
           this row is what shows the draft panel instead. */
        {
          id: 1,
          agent: 'claude',
          cwd: MOCK_PROJECTS[0],
          project: payload?.project ?? MOCK_PROJECTS[0],
          state: 'running',
          question: null,
          startedAt: new Date(Date.now() - 4 * 60000).toISOString(),
          exitCode: null,
          /* The draft rides in the session — see `SessionWork` in
             `src-tauri/src/terminal/model.rs`. There is no issue behind it and
             nothing else on the front end holds these words, so without this
             the right-hand draft panel would be unreachable in a browser.
             Priority is set and the type is left on Auto, so both halves of
             that pair can be seen at once. */
          work: {
            kind: 'newTask',
            text:
              'The log view drops lines once it is past about ten thousand of ' +
              'them, and nothing says so — it just stops scrolling back.',
            issueType: null,
            priority: 1
          }
        },
        /* A run's session, in the middle rather than at the end: the selection
           repair in `loadSessions` lands on the *last* row, and that is spoken
           for above. The id is the one `run_state` names as its session and the
           one the in_progress fixture issues carry as their `assignee` — all
           three have to agree for the caption to say anything, which is the whole
           of smetana-a5b. With them agreeing, this row is captioned with the ids
           it claimed instead of a bare "Agent", and picking it shows the claimed
           tasks in the right-hand column. */
        {
          id: 7,
          agent: 'claude',
          cwd: MOCK_PROJECTS[0],
          project: payload?.project ?? MOCK_PROJECTS[0],
          state: 'running',
          question: null,
          startedAt: new Date(Date.now() - 64 * 60000).toISOString(),
          exitCode: null,
          work: { kind: 'run' }
        },
        {
          id: 2,
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
             an issue id, set in different families.

             The id is one of the fixture board's own, not an invented one:
             picking this row opens that issue in the right column and
             highlights its card, and an id no column holds would leave both
             empty with nothing to say why. */
          work: { kind: 'editTask', id: 'bd-3c9d' }
        }
      ]
    }
    /* Every session the worker holds, of every project, as the project rail
       reads them. A read, and answered here for the reason every other read is:
       a browser has no worker, and a command that threw would have the store
       log an error on every start of `npm run dev`.

       Deliberately not derived from `terminal_list` above: that stub answers
       with the same three sessions whatever project it is asked about, which is
       fine for a panel that only ever draws one, and would make every tile on
       the rail identical. These are three states across three projects instead —
       one project waiting on a person, one working, one with nothing going on —
       so the rail's three dots can all be seen at once. The three ids on the
       first project are `terminal_list`'s own, with its states. */
    if (command === 'terminal_marks') {
      /* Each row carries its work kind as the worker's `SessionMark` does: the
         rail leaves a person's own shells out of a project's dots, so a row
         without one would be a shape the app never sees behind Tauri.

         The last row is a shell, and it is deliberately alone in the quiet
         project rather than beside a loud one: nothing in `tests/` exercises
         this stub, so a browser is the whole of the rail's verification, and
         the check worth having is one that is silent while the filter works and
         loud the moment it breaks. Here that tile stays grey exactly as it was
         before the field existed, and a filter that stopped dropping shells
         turns it loud on its own. Beside a project already `loud` the same
         break would have shown as a 2 where a 1 belongs, in a tooltip, if
         anybody happened to read it. */
      return [
        { id: 1, project: MOCK_PROJECTS[0], state: 'running', kind: 'newTask' },
        { id: 2, project: MOCK_PROJECTS[0], state: 'needs-you', kind: 'editTask' },
        { id: 7, project: MOCK_PROJECTS[0], state: 'running', kind: 'run' },
        { id: 11, project: MOCK_PROJECTS[2], state: 'running', kind: 'run' },
        { id: 12, project: MOCK_PROJECTS[1], state: 'needs-you', kind: 'shell' }
      ]
    }
    if (command === 'terminal_attach') {
      return { data: toBase64(MOCK_SESSION_OUTPUT), seq: 0 }
    }
    /* Detach and resize change nothing on disk and have nothing to lie
       about. */
    if (command === 'terminal_detach' || command === 'terminal_resize') return null
    /* Everything that *starts* something — `terminal_create` and
       `terminal_shell` — is deliberately not answered here and falls through to
       the rejection below. There is no PTY in a browser, and a session handed
       back with nothing behind it would put a row in the agents panel or a tab
       in the centre whose terminal could never say a word. The loud refusal is
       the honest answer, and it is the same one every write gets. */
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
