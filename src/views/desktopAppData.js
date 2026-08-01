/* Sample state for the shell template — the same fixture the design system's
   DesktopApp template renders with. Replace with real tracker / agent data. */
const E = '\u001b'

export const agents = [
  { name: 'claude-1', task: 'bd-a1b2', elapsed: '2h 14m', state: 'needs-you' },
  { name: 'claude-2', task: 'bd-3c9d', elapsed: '1h 02m', state: 'running' },
  { name: 'claude-3', task: 'bd-7f31', elapsed: '18m', state: 'running' }
]

export const tabs = [
  { id: 'chat', kind: 'pinned', label: 'Chat' },
  { id: 'kanban', kind: 'pinned', label: 'Kanban' },
  { id: 'tabs.rs', kind: 'file', label: 'tabs.rs', dirty: true },
  { id: 'agent.rs', kind: 'preview', label: 'agent.rs' }
]

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

export const scope = {
  repo: 'invisor/smetana',
  worktree: 'wt/bd-a1b2',
  branch: 'feat/worktree-rename',
  dirtyCount: 3,
  agentsActive: 2,
  notifications: 2
}

export const inspector = {
  id: 'bd-a1b2',
  status: 'needs-you',
  title: 'Rename worktree when the branch changes',
  waitingFor: '4m',
  question: 'Worktree name collides with wt/bd-77e1. Overwrite?',
  collidesWith: 'wt/bd-77e1',
  blocksDownstream: 5
}
