/* Sample state for the shell template — the same fixture the design system's
   DesktopApp template renders with. Replace with real tracker / agent data. */
const E = '\u001b'

export const columns = [
  /* bd always has an open status, so the mock has to as well: ready is the one
     column that takes new tasks, and without it the "+" is nowhere to be seen
     in the browser. */
  {
    status: 'ready',
    tasks: [{ id: 'bd-4e88', title: 'Vendor IBM Plex Mono for offline builds', status: 'ready' }]
  },
  {
    status: 'needs-you',
    tasks: [
      {
        id: 'bd-a1b2',
        title: 'Rename worktree when the branch changes',
        status: 'needs-you',
        needsResponse: true,
        blocks: 5
      }
    ]
  },
  {
    status: 'running',
    wipLimit: 3,
    tasks: [
      { id: 'bd-3c9d', title: 'Virtualise the log list above 10k lines', status: 'running', blocks: 1 },
      { id: 'bd-7f31', title: 'Tab overflow menu with keyboard cycling', status: 'running', blocks: 2 }
    ]
  },
  {
    status: 'blocked',
    tasks: [
      {
        id: 'bd-77e1',
        title: 'Persist preview tab across restarts',
        status: 'blocked',
        blockedBy: 2,
        spawnedFrom: 'bd-7f31'
      }
    ]
  },
  /* A run gave this one up over something it could not settle. Custom, like
     awaiting-review below, but unlike it this one is load-bearing rather than
     decorative: it is the only card in the browser whose menu offers "Answer
     questions", whose play is greyed, and whose move to Ready asks first — and
     the questions the dialog quotes are the `parked:` lines the mock backend
     hangs off this very status. */
  {
    status: 'parked',
    tasks: [
      {
        id: 'bd-29j1',
        title: 'Show the tracker state on a non-empty board too',
        status: 'parked'
      }
    ]
  },
  {
    // not a reserved status: colour comes from the FNV-1a hash, plus the "AR" code
    status: 'awaiting-review',
    tasks: [
      { id: 'bd-0f4a', title: 'Deterministic status colours for custom statuses', status: 'awaiting-review' }
    ]
  },
  {
    status: 'done',
    tasks: [{ id: 'bd-12cd', title: 'Bump tauri to 2.1', status: 'done' }]
  }
]

export const logLines = [
  { time: '14:02:29', text: `${E}[90m$${E}[0m cargo test --workspace` },
  { time: '14:02:44', text: `${E}[33mwarning${E}[0m: unused variable: ${E}[1mworktree${E}[0m`, level: 'warn' },
  { time: '14:02:58', text: `${E}[32m✓${E}[0m 41 passed  ${E}[31m✗${E}[0m 1 failed` },
  { time: '14:02:58', text: `${E}[31merror${E}[0m: assertion failed at src/tabs.rs:118`, level: 'error' },
  { time: '14:03:09', text: `${E}[35m?${E}[0m worktree name collides with wt/bd-77e1 — overwrite?` }
]
