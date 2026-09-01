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
import { emit, listen } from '@tauri-apps/api/event'
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

/* What a run may merge into. Hoisted out of the `target_branches` answer for
   the reason the panel's own list below is hoisted out of `vcs_branches`: the
   run window's fixture is drawn against the same list, and one of them is short
   of a repository — the browser is the only place that lower group and its
   notes can be seen at all, since there is no Rust worker here to walk
   anybody's repositories. */
/* Epoch seconds, that long ago. The stamps below are ages rather than dates
   for the reason every other time in this file is relative: a fixture written
   as a date is a branch that reads as three years old on the day somebody opens
   the browser, and what these fields are drawn as is "2h". */
const secondsAgo = (seconds) => Math.floor(Date.now() / 1000) - seconds

const MOCK_TARGET_BRANCHES = [
  /* `at` is the stamp `git::combine` ordered this list by, carried out
     so a row can be captioned with it — the newest first, which is the order
     the real answer arrives in. `release/7` has none: a branch no repository
     has a reflog for is the fresh-clone case, and it is what the alphabetical
     tail of that order is made of. */
  { name: 'main', missing_in: [], at: secondsAgo(2 * 3600) },
  { name: 'staging', missing_in: [], at: secondsAgo(26 * 3600) },
  { name: 'feature/runs-project-config', missing_in: [], at: secondsAgo(5 * 86400) },
  { name: 'release/7', missing_in: ['admin', 'extension'], at: null }
]

/* The selected repository's branches. Hoisted out of the `vcs_branches` answer
   because the dialog fixture below is drawn against the same list: a New branch
   window in a browser offering branches the panel behind it does not have would
   be two fixtures disagreeing about one repository. */
const MOCK_BRANCHES = [
  { name: 'develop', current: false },
  { name: 'feat/worktree-rename', current: true },
  { name: 'main', current: false },
  { name: 'release/7', current: false }
]

/* What a dialog window is drawn with in a browser.

   `?view=dialog&kind=<name>` is this project's only verification of these
   components — there is no component test runner — and a dialog window holds no
   store: everything it draws is announced to it by the app window. In a browser
   there is no app window, so without this the screen the acceptance criteria
   rest on rendered an empty form: "Cut from ." with no branch, no list to check
   a name against and no title.

   This is a hand-written shape standing in for what the app window would say,
   the way `terminal_marks` and `vcs_compare` above stand in for a worker and for
   git. Small on purpose: a fixture per kind, holding what that dialog draws and
   nothing else. */
const DIALOG_PROPS = {
  /* The queue's run, which is the one the play in the column header starts and
     the only scope with a priority floor on screen. `branches` is the
     `target_branches` answer below rather than the panel's list: the field is
     filled from the run store's own read, and a window offering branches the
     board behind it could not merge into would be two fixtures disagreeing
     about one project.

     Three of these deliberately disagree with `RunModal`'s own fallbacks —
     Crew rather than Autopilot, P3 rather than P2, four at once rather than
     three. Those three fields are filled once, when the dialog opens, out of
     what the app window announced, so a fixture that happened to match the
     fallbacks would draw the same screen whether the announcement arrived or
     never came at all. Here it is the difference that is being checked.

     `configError` is '' and `liveCheckBlocked` is '' deliberately — a browser is
     where the ordinary state of this dialog is looked at, and both of the loud
     ones already have a project of their own in `?view=gallery`. */
  run: {
    title: 'Run the queue',
    scope: { kind: 'queue' },
    count: 12,
    partOf: null,
    branches: MOCK_TARGET_BRANCHES,
    defaultBranch: 'main',
    defaultPriority: 2,
    defaultParallel: 4,
    remembered: {
      mode: 'supervised',
      targetBranch: 'main',
      minPriority: 3,
      liveCheck: true,
      fileFindings: true
    },
    liveCheckAvailable: true,
    liveCheckBlocked: '',
    configError: '',
    error: '',
    busy: false
  },
  /* Filing a task. The images are deliberately absent: that list is the
     window's own store and not something the app window announces, and in a
     browser there is nothing to put in it — a drop needs Tauri and the picker
     is a write. The strip draws its empty state, which is the honest picture of
     this window before anybody attaches anything. */
  'new-task': {
    title: 'New task',
    busy: false,
    status: 'ready',
    parent: null
  },
  'new-branch': {
    title: 'New branch',
    from: 'feat/worktree-rename',
    branches: MOCK_BRANCHES,
    actions: { allowed: true, reason: null },
    busy: false
  },
  /* Renaming one. `from` is a branch of `MOCK_BRANCHES` above, so the list the
     name is checked against is the one this window is a question about: type
     `main` into the field and it refuses the name as taken, and put the name
     back exactly as it was and the button goes dead with no red line — which is
     the pair of rules `?view=dialog&kind=rename-branch` exists to show. */
  'rename-branch': {
    title: 'Rename branch',
    from: 'feat/worktree-rename',
    branches: MOCK_BRANCHES,
    actions: { allowed: true, reason: null },
    busy: false
  },
  /* Deleting one, in the state it opens in — the question, before git has been
     asked anything. The other two states are reached by an answer from a
     backend a browser does not have, so they are looked at in `?view=gallery`
     instead, where all three stand side by side. */
  'delete-branch': {
    title: 'Delete release/7?',
    branch: 'release/7',
    notMerged: false,
    refusal: '',
    busy: false
  },
  /* Every fixture carries a `title` beside what its dialog draws, because that
     string is the OS frame's caption in the app and there is no frame in a
     browser to notice it missing. It is the same sentence the component works
     out for itself from the props below it — see the comment each of them
     carries — and the app window announces it for the same reason this does. */
  'delete-task': {
    title: 'Delete bd-a1b2?',
    id: 'bd-a1b2',
    taskTitle: 'Rename worktree when the branch changes',
    busy: false
  },
  /* The parked card of the mock board and its own `parked:` notes, so the
     window in a browser quotes what the board behind it holds — two fixtures
     disagreeing about one issue would be worse than one. */
  'ready-task': {
    title: 'Move bd-29j1 to ready with the question unanswered?',
    id: 'bd-29j1',
    taskTitle: 'Show the tracker state on a non-empty board too',
    questions: [
      'needs a decision on where the strip sits',
      'still waiting on the design call'
    ]
  },
  'promote-column': {
    title: 'Move 12 tasks to ready?',
    count: 12,
    busy: false,
    moved: 0,
    failed: null
  },
  'setup-project': {
    title: 'Set this project up?',
    name: 'holiday-curb',
    existing: false,
    busy: false
  },
  /* The other window about the same file, and the one with fields in it.
     `branches` is the `target_branches` answer above rather than the panel's
     list, for the `run` fixture's reason: the field is filled from the same
     command, and a window offering branches the project behind it could not
     merge into would be two fixtures disagreeing about one project.

     None of the four is `DEFAULTS_FALLBACK`'s, and that is the point of the
     fixture rather than a taste in numbers — though not for quite the reason
     the `run` fixture gives above, and the difference is worth having straight.
     `RunModal` really does carry its own fall-backs, so a fixture matching them
     draws the same screen announced or not. This form carries none: its
     `defaults` prop defaults to `{}`, so an unannounced window draws empty
     fields under "Between…" lines, which is exactly what the browser saw before
     this fixture existed. What the numbers buy here is that the screen differs
     from **every** fall-back in the chain, so it is proof that the announcement
     arrived *and* was seeded — and `DEFAULTS_FALLBACK` is the one set that
     could stop being proof, if that prop default were ever tidied to it.
     `?view=dialog&kind=project-settings` is this project's only check of any of
     that. */
  'project-settings': {
    title: 'Project settings',
    defaults: {
      target_branch: 'main',
      min_priority: 1,
      max_parallel_tasks: 6,
      review_passes: 2
    },
    branches: MOCK_TARGET_BRANCHES,
    busy: false,
    error: ''
  },
  /* Deleting a Claude Code transcript. The record is the first row of
     `mockSessions` above, so what this window says in a browser is what the
     Sessions tab behind it holds — the same rule the `ready-task` fixture keeps
     about the board's parked card, and for the same reason.

     The caption is `DELETE_SESSION_TITLE` in `components/agent/sessionMenu.js`,
     which the component and the app window's announcement both call. This
     literal is the one copy that stands apart from it, as every fixture here
     does, and it is what `?view=dialog&kind=delete-session` draws. */
  'delete-session': {
    title: 'Delete this session?',
    session: {
      id: '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60',
      path: '/Users/you/.claude/projects/-Users-you-dev-smetana/9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60.jsonl',
      cwd: '/Users/you/dev/smetana',
      title:
        'Talk to me in Russian: everything you say in this project, and keep the commit messages in Russian too',
      size: 2_884_016
    },
    busy: false
  },
  /* Choosing what an agent reviews. One rule at the top and a row that differs
     under it, because that is the whole shape of this window and a fixture of
     rows that all follow the rule shows none of it.

     Four repositories and two rows on purpose. `release/7` is missing from
     `admin` and `extension` (`MOCK_TARGET_BRANCHES` above), so the rule's own
     table is the project root alone — `admin` is in it because somebody added
     it by hand, which is what draws the `man` badge, the pair of its own and
     the `x` that takes it out again. `extension` is left out and named in the
     notes block; `infra` has the branch and is simply not in the review, so
     `Add a repository` opens on both of its sentences at once. It also sits
     outside the project, which is the one repository here whose path is drawn
     `~/work/smetana-infra` rather than `./…`.

     `branches` is `MOCK_TARGET_BRANCHES` rather than the panel's list, for the
     `run` fixture's reason: the window is filled from that same command, and
     one offering branches the project behind it does not have would be two
     fixtures disagreeing about one project. `remote` and `fetchedAt` are keyed
     by path, as the app window's own announcement is. */
  'review-changes': {
    title: 'Review changes',
    form: {
      base: { ref: 'main', remote: false },
      head: { ref: 'release/7', remote: false },
      repoIds: ['/Users/you/dev/smetana', '/Users/you/dev/smetana/admin'],
      overrides: {
        '/Users/you/dev/smetana/admin': {
          base: { ref: 'main', remote: true },
          head: { ref: 'feature/runs-project-config', remote: false }
        }
      },
      manual: ['/Users/you/dev/smetana/admin']
    },
    repos: [
      { name: '.', path: '/Users/you/dev/smetana' },
      { name: 'admin', path: '/Users/you/dev/smetana/admin' },
      { name: 'extension', path: '/Users/you/dev/smetana/extension' },
      { name: 'infra', path: '/Users/you/work/smetana-infra' }
    ],
    root: '/Users/you/dev/smetana',
    home: '/Users/you',
    branches: MOCK_TARGET_BRANCHES,
    remote: {
      '/Users/you/dev/smetana': ['main', 'staging', 'release/7'],
      '/Users/you/dev/smetana/admin': ['main', 'feature/runs-project-config']
    },
    fetchedAt: {
      '/Users/you/dev/smetana': secondsAgo(2 * 60),
      '/Users/you/dev/smetana/admin': secondsAgo(3 * 3600)
    },
    fetching: [],
    fetchFailed: [],
    busy: false
  }
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
   shows whether the whole vocabulary does.

   `ignored` is what the real `files_list` answers after asking
   `git check-ignore` about the folder it just read, and it is set here on a
   folder, on everything inside that folder and on one file with ordinary
   siblings — the three shapes the muted row has to be looked at in. Without it
   the greying would be visible in `npm run tauri dev` alone, and this is a
   colour that is defined once per theme. */
export const MOCK_TREE = {
  '': [
    { name: 'src', path: 'src', kind: 'dir' },
    { name: 'target', path: 'target', kind: 'dir', ignored: true },
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
    { name: 'scratch.rs', path: 'src/scratch.rs', kind: 'file', ignored: true },
    { name: 'tabs.rs', path: 'src/tabs.rs', kind: 'file' },
    { name: 'unknown-binary', path: 'src/unknown-binary', kind: 'file' },
    { name: 'worktree.rs', path: 'src/worktree.rs', kind: 'file' }
  ],
  /* Every entry inside an ignored folder is ignored in its own right: git
     answers for each listing on its own and nothing is carried down by hand. */
  target: [
    { name: 'debug', path: 'target/debug', kind: 'dir', ignored: true },
    { name: 'CACHEDIR.TAG', path: 'target/CACHEDIR.TAG', kind: 'file', ignored: true }
  ]
}

/* The entries of the tree above that a real `files_read` would refuse. Named
   rather than sniffed: a fixture has no bytes to look at. */
const MOCK_BINARY = new Set(['src/app-icon.png', 'src/bd-aarch64.tar.gz', 'src/unknown-binary'])

/* An eight-pixel PNG, base64, and the same bytes `ATTACHMENTS[0]` in
   `views/Gallery.vue` is drawn from. What `attachment_reopen` answers with
   below, so the image window has something to fit into itself under
   `npm run dev`. Written out rather than imported from the gallery: that file
   is code-split and never in the product bundle, and this one is. */
const MOCK_IMAGE_BASE64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAD0lEQVR42mPIwwEYhpYEADyoUoFZDU7TAAAAAElFTkSuQmCC'

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

/* The Sessions tab's fixture: Claude Code transcripts as the worker reports
   them, field for field with the real `sessions_list`. Six rows, each one a
   case the row has to draw and none of them a repeat of another: a session
   Claude Code titled itself, whose `title` and `firstPrompt` are two different
   sentences, a session with subagents and one without, a long title that has to
   ellipsise, a session out of a worktree on a branch of its own, one with no
   branch at all, one nobody has titled because the transcript holds no human
   message, and one so recent the time label is not measured in hours.

   Built per call rather than written out as constants, and the times are
   offsets from now: a fixture with dates in it reads "2y ago" a year after
   somebody types it, and the label the design is about — `18h ago` — could then
   never be seen in a browser at all. The paths are this project's own
   convention (`~/.claude/projects/<cwd with separators replaced>`), so what the
   hover string shows is the shape a person will meet in the app.

   The order is deliberately not sorted here. The store sorts newest first, and
   a fixture that arrived already sorted would make a browser look correct with
   that sort deleted.

   `size` is the one field no row draws: it is the confirmation before a delete
   that names it, and the numbers here are chosen to be that dialog's own cases
   — the 16 MB transcript this subsystem was measured against, an ordinary few
   hundred kilobytes, and the zero-byte one nobody said anything in. */
const MINUTE_MS = 60 * 1000
const HOUR_MS = 60 * MINUTE_MS
const DAY_MS = 24 * HOUR_MS

function mockSessions(project) {
  const at = (ms) => new Date(Date.now() - ms).toISOString()
  const stem = (cwd) => cwd.replace(/[/.]/g, '-')
  const worktree = `${project}/.worktrees/smetana-oln-sessions-tab-disk-history`
  /* `cwdExists` is what the Resume row is greyed from, and it is `false` on the
     worktree row on purpose: a worktree is removed once its task is merged and
     the transcript stays behind, so on a real machine a good number of rows are
     in exactly that state and it is the one the browser could otherwise never
     show. The rest are directories that are still there. */
  const rows = [
    {
      id: '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60',
      cwd: project,
      cwdExists: true,
      branch: 'main',
      /* The case the second field exists for, and the one the real folder is
         mostly made of: Claude Code titled the session itself, so the row says
         what it was about while the card below still says what the person
         opened with. The two are deliberately nothing like each other here —
         a fixture where they matched would let a card that had gone back to
         drawing the title look correct. */
      title: 'Sessions tab reads Claude Code transcripts off disk',
      firstPrompt:
        'Talk to me in Russian: everything you say in this project, and keep the commit messages in Russian too',
      lastRole: 'assistant',
      lastText:
        'Done. The three columns are drawn from the tracker now, and the fixture that used to stand in for the log pane is gone with it.',
      messages: 48,
      subagents: 3,
      model: 'claude-opus-5',
      modifiedAt: at(18 * HOUR_MS),
      size: 2_884_016
    },
    {
      id: '3a7e5b10-1c2d-4e3f-9a8b-7c6d5e4f3a2b',
      cwd: project,
      cwdExists: true,
      branch: 'develop',
      title: 'Why does the scope bar count dirty files it cannot see',
      firstPrompt: 'Why does the scope bar count dirty files it cannot see',
      lastRole: 'user',
      lastText: 'Leave it for now, file it as a task instead.',
      messages: 12,
      subagents: 0,
      model: 'claude-opus-5',
      modifiedAt: at(4 * MINUTE_MS),
      size: 148_392
    },
    {
      id: '5d2f8c41-9b0a-4c1d-8e7f-6a5b4c3d2e1f',
      cwd: worktree,
      /* The worktree has been merged and removed; the transcript is still
         here, and the row says so by refusing the resume. */
      cwdExists: false,
      branch: 'feature/smetana-oln-sessions-tab-disk-history',
      /* The long one, which the row has to ellipsise and the card has to wrap.
         No generated title in this transcript, so both fields hold the one
         string — the shape every row had before that record was read, and now
         the smaller half of the folder rather than the usual case: the row
         above is the one there are more of. */
      title:
        'Implement the front-end half of the sessions tab, the row and the opened card, against the fixtures alone',
      firstPrompt:
        'Implement the front-end half of the sessions tab, the row and the opened card, against the fixtures alone',
      lastRole: 'assistant',
      lastText:
        'Both gates are green. The row draws in all four theme and density combinations; what is left is the pass over the gallery.',
      messages: 214,
      subagents: 1,
      model: 'claude-opus-5',
      modifiedAt: at(2 * DAY_MS),
      size: 16_402_771
    },
    {
      id: 'c81b0e39-4a5f-4b6c-9d0e-1f2a3b4c5d6e',
      cwd: `${project}/src-tauri`,
      cwdExists: true,
      /* A session started outside a repository, which is an ordinary thing:
         the transcript records no branch and the row simply has one piece
         fewer. */
      branch: null,
      title: 'Check whether the sidecar digest matches the pinned release',
      firstPrompt: 'Check whether the sidecar digest matches the pinned release',
      lastRole: 'user',
      lastText: 'It does. Nothing to do.',
      messages: 6,
      subagents: 0,
      model: 'claude-sonnet-4-5',
      modifiedAt: at(9 * DAY_MS),
      size: 41_508
    },
    {
      /* Nothing to title it with and nothing said in it: a transcript opened
         and abandoned. Both fallbacks of the row at once, which is the only
         way to see either. */
      id: 'e4a90d77-2b3c-4d5e-8f90-1a2b3c4d5e6f',
      cwd: project,
      cwdExists: true,
      branch: 'main',
      title: null,
      firstPrompt: null,
      lastRole: null,
      lastText: null,
      messages: 0,
      subagents: 0,
      model: null,
      modifiedAt: at(40 * DAY_MS),
      size: 0
    },
    {
      id: '7b6a5948-3c2d-4e1f-9a0b-8c7d6e5f4a3b',
      cwd: project,
      cwdExists: true,
      branch: 'staging',
      title: 'Port the branch list to the design system',
      firstPrompt: 'Port the branch list to the design system',
      lastRole: 'assistant',
      lastText: 'The rebase glyph is git-graph; lucide ships no rebase mark and that is the one about the shape of the history.',
      messages: 97,
      subagents: 12,
      model: 'claude-opus-5',
      modifiedAt: at(400 * DAY_MS),
      size: 7_115_240
    }
  ]
  return rows.map((row) => ({
    ...row,
    path: `/Users/you/.claude/projects/${stem(row.cwd)}/${row.id}.jsonl`
  }))
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

  /* Whether the note about dialog windows has been said. Once per run of the
     dev server: the three commands below are called far too often for a line
     each to be worth reading. */
  let saidAboutDialogWindows = false

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
    /* The dialog windows, for the same reason and with the same answer. All
       three are about a window and none of them is a write: opening one, closing
       one and giving one the height its content came to. Every dialog is
       reachable in a browser at `?view=dialog&kind=<name>`, which is how each is
       checked by eye, and a refusal here would put an error in the console every
       time somebody pressed the menu item that opens one — and, for the sizing,
       once per measurement, which is on every keystroke that changes the height.

       Said once rather than per call, which is what the counter is for: the
       three are pressed and measured often enough that a line each would bury
       whatever else the console was showing.

       `dialog_window_size` answers a shape now — whether the window's size is
       the person's — and `null` is still the right answer here: a browser tab
       has no window to drag, so it is never filled, and `stores/app.js` reads a
       missing answer as exactly that. */
    if (
      command === 'dialog_window_open' ||
      command === 'dialog_window_close' ||
      command === 'dialog_window_size'
    ) {
      if (!saidAboutDialogWindows) {
        saidAboutDialogWindows = true
        console.info(
          '[mockBackend] a dialog window needs Tauri — open ?view=dialog&kind=<name> instead'
        )
      }
      return null
    }
    /* A window saying it has loaded and may be handed anything it missed. In a
       browser there is no other window to have missed anything from, and the
       three windows that make this call — settings, compare and image — are
       each reachable here through their own `?view=`, so every one of them
       makes it. Answered rather than refused for `settings_window_open`'s
       reason exactly: nothing was promised and nothing was lost, and the loud
       refusal at the bottom would put a line in the console every time one of
       those three screens was opened for a look. */
    if (command === 'window_show_ready') return null
    /* The login item. A read, so it answers — otherwise the General tab could
       not be opened under `npm run dev` without an error in the console, and
       the tab is checked by eye there. `supported: false` is the honest answer
       rather than a convenient one: a browser cannot register anything with the
       operating system, so the row draws itself disabled, which is exactly what
       a development build of the app shows.

       `autostart_set` is deliberately absent and falls through to the loud
       refusal at the bottom, like every other write. The switch is disabled
       here, so nothing reaches it in the ordinary course. */
    if (command === 'autostart_state') return { supported: false, enabled: false }
    /* There is no updater in a browser, and `null` is the honest answer rather
       than a state invented for it: `stores/updates.js` reads anything that is
       not one of Rust's six tags as "there is nobody to ask", and the About tab
       then draws nothing about updates at all — the same silence `appVersion()`
       produces by answering `null` and the version line drawing a dash.

       Answered rather than left to the loud refusal at the bottom, which is
       what every unknown command gets. This one is a read, and a read that
       threw would put an error in the console on every start of `npm run dev`
       for a subsystem that is simply not there.

       `updates_check` and `updates_install` are deliberately absent and fall
       through to that refusal, like every other write. Nothing offers them
       here: with the state unavailable the About tab draws no control at all,
       so neither can be reached in the ordinary course. */
    if (command === 'updates_state') return null
    /* What a refused folder needs. A read, so it answers — otherwise every
       start of `npm run dev` would put a warning in the console about a
       subsystem that is simply not there. `'unavailable'` is the honest answer
       rather than a convenient one: a browser has no bundle for the operating
       system to have refused and nothing to reset, so the notice draws its
       sentence and no button, which is exactly what the app shows on a platform
       without `tccutil`.

       `tracker_access_reset` is deliberately absent and falls through to the
       loud refusal at the bottom, like every other write. With this answering
       `'unavailable'` nothing offers it, so it cannot be reached in the
       ordinary course. */
    if (command === 'tracker_access_repair') return 'unavailable'
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
    /* Enough branches for the dialog's field to be worth looking at. The list
       is at the top of this file, where the run window's own fixture reads it
       too. */
    if (command === 'target_branches') return MOCK_TARGET_BRANCHES
    /* What the Agents tab says about the subscription. A read, so it answers:
       without it `?view=settings&tab=agents` opens on the loud refusal at the
       bottom of this file and the block can never be looked at in a browser at
       all.

       A reading rather than one of the two empty states, and a `reduced` one
       rather than a comfortable one: the empty states are a sentence each and
       can be read off the source, while the two rows and the line about what a
       run would do are the part with a layout to check. The numbers are
       `claude.rs`'s own fixture output, so the reset strings are shaped exactly
       as the parser hands them over — the harness's words, timezone and all.

       `claude` and not whatever the settings fixture says, deliberately: the
       real command answers with the agent that was actually reachable, and the
       browser has no `PATH` to look at. */
    if (command === 'agent_usage') {
      return {
        state: 'read',
        agent: 'claude',
        usage: {
          sessionPct: 10,
          sessionReset: 'Aug 7 at 8pm (Europe/Moscow)',
          weekPct: 78,
          weekReset: 'Aug 11 at 5:59pm (Europe/Moscow)'
        },
        band: 'reduced'
      }
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
       looked for.

       `attachment_reopen` used to be absent beside them, on the argument that
       it is only ever called for a draft the app window kept across a project
       switch and there is no app window here to keep one. That stopped being
       true: the image window (`views/ImageWindow.vue`, `?view=image`) reads its
       whole content with this one command, so an absent answer would leave the
       one screen this project can check by eye drawing nothing but its empty
       state. It is a read, and the policy here is that reads answer.

       The picture is the eight-pixel PNG the gallery's own fixtures are drawn
       from, deliberately the same bytes rather than a second copy of a
       different one: what is being looked at is the window around it — the
       fitting, the caption, the frame — and a fixture smaller than the window
       is also the case that proves a small picture is not blown up to fill it.
       Every path answers but one, and the exception is what gives the window's
       other half a door. The Storage tab's button can sweep a file a draft
       still names, so the window draws an empty state carrying that name — and
       with a fixture that answers everything, the one screen this project
       checks by eye could not reach that state at all. A name beginning
       `missing` is refused instead, in the shape Rust refuses one: it is not a
       second policy but the same read answering honestly about a file that is
       not there, and no path a real store ever produced starts that way
       (`stored_name` writes a timestamp first). The other emptiness — nothing
       asked for at all — is `?view=image` with no `path`. */
    if (command === 'attachment_reopen') {
      const path = payload?.path ?? ''
      const name = path.split('/').pop() || 'mock.png'
      if (name.startsWith('missing')) {
        throw { kind: 'io', message: `${path}: no such file` }
      }
      return { path, name, bytes: 72, mime: 'image/png', data: MOCK_IMAGE_BASE64 }
    }
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
    /* The home folder the fixtures are written under, so the review window's
       Repository column draws `~/…` for a repository outside the project in
       `npm run dev` as it would in the app. A browser cannot read one, and this
       is the one place that knows what "you" means in these paths. */
    if (command === 'home_dir') return '/Users/you'
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
    /* Both halves of one answer, and the second is deliberately not empty: a
       browser with nothing unlisted would draw the state every properly set up
       project is in, which is the one state of this block that cannot be
       checked by eye anywhere else. */
    if (command === 'vcs_repos') {
      const project = payload?.project ?? MOCK_PROJECTS[0]
      return {
        repos: [
          { name: '.', path: project, branch: 'feat/worktree-rename', detached: null },
          { name: 'admin', path: `${project}/admin`, branch: null, detached: 'a1b2c3d' }
        ],
        unlisted: ['newrepo']
      }
    }
    /* The left-hand side of a diff. A browser has no git, so one fixture stands
       for every tracked file and the untracked one answers `null` — the two
       answers this command has, and the second is not a failure. */
    if (command === 'vcs_file_at_head') {
      return payload?.path === MOCK_UNTRACKED ? null : MOCK_FILE_AT_HEAD
    }
    /* The branch comparison. A browser has no git, so one fixture stands for
       both modes — the shas are made up and never leave this file, since the
       only thing that reads them is `vcs_file_at_rev` below. */
    if (command === 'vcs_compare') {
      return {
        left: '1111111111111111111111111111111111111111',
        right: '2222222222222222222222222222222222222222',
        files: [
          { path: 'src/stores/vcs.js', origPath: null, kind: 'modified' },
          { path: 'src/components/git/GitPanel.vue', origPath: null, kind: 'added' },
          { path: 'src/views/desktopAppData.js', origPath: null, kind: 'deleted' },
          {
            path: 'src/components/git/RepoList.vue',
            origPath: 'src/components/shell/RepoList.vue',
            kind: 'renamed'
          }
        ]
      }
    }
    /* Either side of a comparison. The added file has nothing on the left, which
       is the second of this command's two answers and not a failure. */
    if (command === 'vcs_file_at_rev') {
      const added = payload?.path === 'src/components/git/GitPanel.vue'
      if (added && payload?.rev?.startsWith('1')) return null
      return MOCK_FILE_AT_HEAD
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
    /* What git is part-way through, asked of the tree above the moment it shows
       an unmerged path — which the fixture's last row is. A **read** that
       changes nothing, so it answers here rather than falling through to the
       refusal, and without it every status read in a browser would log a
       failure and the `Resolve conflicts` button could never be looked at
       outside the isolated gallery frame. This project has no component runner,
       so the running app shell is where that button's place above the commit —
       under both densities, inside the real fold heights — is checked at all.

       `merge`, and the two branches the fixture beside it already names: the
       tree says it is on `feat/worktree-rename`, and `develop` is the other
       branch `MOCK_BRANCHES` offers. A rebase would be the more interesting
       shape and would be a lie about this fixture, whose HEAD is attached. */
    if (command === 'vcs_in_progress') {
      return { op: 'merge', ours: 'feat/worktree-rename', theirs: 'develop' }
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
      return MOCK_BRANCHES
    }
    /* What `origin` is known to have, which the branch-review window reads one
       repository at a time. A read like `vcs_branches` beside it — file reads
       out of `refs/remotes/origin/`, no process — so it answers here rather than
       falling through to the refusal.

       Deliberately not the same list as `vcs_branches`: `release/7` is local
       and has never been pushed, and `spike/origin-only` lives on the server
       and has never been checked out here. Those two are the whole point of the
       side switch, and a fixture where both lists matched would draw a window
       in which it did not appear to do anything. */
    if (command === 'vcs_remote_branches') {
      /* A name and the stamp of the fetch that last moved it, alphabetically —
         `git::RemoteBranch`. `spike/origin-only` carries none, which is the
         ordinary answer for a ref no fetch has ever written a log line for and
         the one a caption has to survive. */
      return [
        { name: 'develop', at: secondsAgo(40 * 60) },
        { name: 'feat/worktree-rename', at: secondsAgo(3 * 3600) },
        { name: 'main', at: secondsAgo(11 * 86400) },
        { name: 'spike/origin-only', at: null }
      ]
    }
    /* When this repository last fetched — the modification time of its
       `FETCH_HEAD`. A read like the two above it, files off the disk and no
       process, so it answers here rather than falling through to the refusal.

       Deliberately answered even though `vcs_fetch` below is not: the two are
       opposite kinds of call. Reaching a remote is something a browser cannot
       do and must be seen failing; when this machine last reached one is a
       stat, and a browser refusing it would draw "fetched" with nothing after
       it in a window whose whole subject is how fresh the refs are. */
    if (command === 'vcs_last_fetch') {
      return secondsAgo(2 * 60)
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
    /* The right column's Sessions tab: Claude Code's own transcripts, which in
       the app are read off `~/.claude/projects` by the worker. A browser has no
       worker and no home directory to walk, and the tab is one of two in a
       panel a person opens on purpose — without an answer here it would log a
       failure on every open of `npm run dev`, which is one of this project's
       two verifications. */
    if (command === 'sessions_list') {
      return mockSessions(payload?.project ?? MOCK_PROJECTS[0])
    }
    /* The session row's four verbs that leave this window — `sessions_open_log`,
       `sessions_open_cwd`, `sessions_reveal` and `sessions_delete` — are
       deliberately not answered here either, and for a plainer reason than the
       two below: a browser has no desktop to open a file with, no file manager
       to show one in, and no business unlinking one. The loud rejection reaches
       the row's menu as the sentence a refusal would have carried, which is what
       a person pressing any of them in `npm run dev` should be told.

       Everything that *starts* something — `terminal_create` and
       `terminal_shell` — is deliberately not answered here and falls through to
       the rejection below. There is no PTY in a browser, and a session handed
       back with nothing behind it would put a row in the agents panel or a tab
       in the centre whose terminal could never say a word. The loud refusal is
       the honest answer, and it is the same one every write gets.

       `files_clipboard_read` is the odd one in that list, because it is a
       **read** and every other read here answers. A fixture for it would be a
       list of absolute paths naming files that are not on this machine, which
       would un-grey Paste in the file tree over a file that cannot be pasted —
       and `files_copy_external` behind it refuses here like every other write.
       So it falls through too, which is exactly what a machine with nothing on
       its clipboard looks like to `stores/files.js`: an empty list, one warning
       in the console, and the tree's own record carrying the paste. */
    // Any write command (tracker_update/close/reopen, files_write, and
    // whatever appears later) has to reject explicitly rather than silently
    // return a plausible but foreign issue — otherwise a "write" in the browser
    // would look like it worked while doing nothing.
    throw new Error(
      `mockBackend: "${command}" is not implemented — this is a read-only stub for browser ` +
        'dev mode; writes to the tracker require the real Tauri backend (npm run tauri dev).'
    )
  }, { shouldMockEvents: true })

  /* The app window's half of the dialog contract, for the one page a browser
     has. A dialog window says hello after it has subscribed and is answered with
     the props as they stand; here that answer is the fixture.

     The hello is the only trigger worth having in a browser, and `invoke` is
     deliberately not one: a browser runs one page, so `dialog_window_open` is
     only ever called from the app view, where nothing is listening for a
     dialog's props. Answering it too would be a line that can never fire.

     Registered here rather than lazily because this runs before the app mounts,
     which is what puts the listener in place before any window can say hello.
     A kind with no fixture gets no listener at all — it simply draws with
     nothing, exactly as it did before this existed. */
  for (const [kind, fixture] of Object.entries(DIALOG_PROPS)) {
    listen(`dialog:hello:${kind}`, () => {
      emit(`dialog:props:${kind}`, fixture).catch((err) => {
        console.warn('[mockBackend] the dialog fixture did not reach the window:', err)
      })
    }).catch((err) => {
      console.warn('[mockBackend] no fixture will be offered for a dialog window:', err)
    })
  }

  return true
}
