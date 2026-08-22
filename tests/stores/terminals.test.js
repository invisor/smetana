import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

/* The DOM half of the sound, stood in for — the same seam `runs.test.js` uses,
   and for the same reason: what this store is answerable for is which sound it
   asks for, not whether an element played it. */
const chime = vi.fn()
vi.mock('../../src/chime.js', () => ({ chime: (id) => chime(id) }))

beforeEach(() => {
  chime.mockClear()
})

const session = (over = {}) => ({
  id: 1,
  agent: 'claude',
  cwd: '/p',
  project: '/p',
  state: 'running',
  question: null,
  startedAt: '2026-08-03T10:00:00Z',
  exitCode: null,
  work: { kind: 'bare' },
  ...over
})

// PTY output is arbitrary bytes; btoa() alone only accepts Latin1, so route
// through TextEncoder first — same path the Rust side takes for anything
// outside ASCII (see "another session's output does not reach the subscriber"
// below).
const b64 = (text) => btoa(String.fromCharCode(...new TextEncoder().encode(text)))

/* One row of what `terminal_marks` answers: the whole of what the rail is told
   about a session it has no row for. The default kind is an agent's, matching
   `session()` above — every test here that does not say otherwise is about an
   agent, and the rail counts those. */
const mark = (over = {}) => ({ id: 1, project: '/p', state: 'running', kind: 'bare', ...over })

async function ready() {
  const loaded = await loadStores()
  loaded.ipc.on('terminal_list', [session()])
  loaded.ipc.on('terminal_marks', [])
  loaded.ipc.on('terminal_attach', { data: b64('hello'), seq: 0 })
  loaded.ipc.on('terminal_detach', null)
  loaded.ipc.on('terminal_write', null)
  loaded.ipc.on('terminal_resize', null)
  await loaded.stores.terminals.initTerminals()
  await loaded.stores.terminals.loadSessions('/p')
  return loaded
}

describe('state translation', () => {
  it('exited with zero is done, with non-zero it is failed', async () => {
    const { stores } = await loadStores()
    expect(stores.terminals.toUiState(session({ state: 'exited', exitCode: 0 }))).toBe('done')
    expect(stores.terminals.toUiState(session({ state: 'exited', exitCode: 1 }))).toBe('failed')
  })

  it('idle is a quiet agent, not one ready for work', async () => {
    const { stores } = await loadStores()
    expect(stores.terminals.toUiState(session({ state: 'idle' }))).toBe('ready')
    expect(stores.terminals.toUiState(session({ state: 'needs-you' }))).toBe('needs-you')
    expect(stores.terminals.toUiState(session({ state: 'starting' }))).toBe('running')
  })
})

describe('the session list', () => {
  it('a state event updates the row in place', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ state: 'needs-you' }))
    await nextTick()
    expect(stores.terminals.terminalState.sessions[0].state).toBe('needs-you')
    expect(stores.terminals.terminalState.sessions).toHaveLength(1)
  })

  it('an event about an unknown session adds it', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 2 }))
    await nextTick()
    expect(stores.terminals.terminalState.sessions.map((s) => s.id)).toEqual([1, 2])
  })

  it('another project\'s sessions do not reach the list', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 3, project: '/other' }))
    await nextTick()
    expect(stores.terminals.terminalState.sessions.map((s) => s.id)).toEqual([1])
  })
})

/* Not every removal is this window's own doing: a run kills the session of a
   batch that stopped on a question (smetana-8pe), and the worker announces it
   with terminal:removed. Without the listener the row kept the session's last
   emitted state — needs-you, a question nobody can answer behind a process
   that is gone — and over a night those dead loud rows accumulated. */
describe('a session removed by the worker', () => {
  it('the removed event drops the row and repairs the selection', async () => {
    const { stores, emit, nextTick } = await ready()
    expect(stores.terminals.terminalState.activeId).toBe(1)
    await emit('terminal:removed', { id: 1 })
    await nextTick()
    expect(stores.terminals.terminalState.sessions).toHaveLength(0)
    expect(stores.terminals.terminalState.activeId).toBe(null)
  })

  it('an event about an id already gone changes nothing', async () => {
    /* The front end's own removeSession has already dropped the row by the
       time the worker's event arrives, so the same event serves both callers
       only if replaying it is a no-op — the list stays, and a selection
       pointing elsewhere is not touched. */
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 2 }))
    await nextTick()
    expect(stores.terminals.terminalState.activeId).toBe(1)

    await emit('terminal:removed', { id: 2 })
    await emit('terminal:removed', { id: 2 })
    await emit('terminal:removed', { id: 99 })
    await nextTick()
    expect(stores.terminals.terminalState.sessions.map((s) => s.id)).toEqual([1])
    expect(stores.terminals.terminalState.activeId).toBe(1)
  })
})

/* A run's sessions are not started from this window: the run worker asks the
   terminal worker directly, and the only thing the front end ever sees is a
   state event. Without this the row appeared in the panel unselected, the
   terminal kept showing whichever agent was there before — often a finished one,
   or nothing at all — and the person watching a run had to click every batch
   back into view. */
describe('a session a run started', () => {
  it('is selected as soon as it arrives, and says so', async () => {
    const { stores, emit, nextTick } = await ready()
    expect(stores.terminals.lastRunStart.value).toBeNull()

    await emit('terminal:state', session({ id: 7, work: { kind: 'run' } }))
    await nextTick()

    expect(stores.terminals.terminalState.activeId).toBe(7)
    expect(stores.terminals.lastRunStart.value).toBe(7)
  })

  /* Every batch of a run is a session of its own, and the one before it has
     exited by the time the next starts: staying on the first would leave a
     person watching a dead terminal for the rest of the run. */
  it('the next batch takes the selection from the one that finished', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 7, work: { kind: 'run' } }))
    await emit('terminal:state', session({ id: 8, work: { kind: 'run' } }))
    await nextTick()

    expect(stores.terminals.terminalState.activeId).toBe(8)
    expect(stores.terminals.lastRunStart.value).toBe(8)
  })

  /* Only the arrival moves anything. A run's session goes on emitting state
     for as long as it lives — every question, every exit — and re-selecting it
     on each one would drag a person back off whatever row they had picked. */
  it('a later state event about it does not take the selection back', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 7, work: { kind: 'run' } }))
    stores.terminals.terminalState.activeId = 1

    await emit('terminal:state', session({ id: 7, work: { kind: 'run' }, state: 'needs-you' }))
    await nextTick()

    expect(stores.terminals.terminalState.activeId).toBe(1)
  })

  /* The same rule the list itself applies: a session belonging somewhere else
     is not in this panel, and pointing the selection at a row nobody can see
     would black the terminal out with no way back to it. */
  it("another project's run does not move the selection", async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 7, project: '/other', work: { kind: 'run' } }))
    await nextTick()

    expect(stores.terminals.terminalState.activeId).toBe(1)
    expect(stores.terminals.lastRunStart.value).toBeNull()
  })

  /* Sessions this window started are `createSession`'s business, and it has
     rules of its own — a person who picked another agent mid-start keeps their
     place. The worker announces those sessions by event too, and following the
     event here would overrule that. */
  it('a session that is not a run is left to the start that asked for it', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 2, work: { kind: 'newTask' } }))
    await nextTick()

    expect(stores.terminals.terminalState.activeId).toBe(1)
    expect(stores.terminals.lastRunStart.value).toBeNull()
  })
})

describe('switching project', () => {
  it('a stale answer from the old project does not survive the switch', async () => {
    const { ipc, stores } = await loadStores()
    /* terminal_list here resolves on demand, not immediately, so the test
       — not the event loop — decides which of the two calls lands first. */
    const pending = new Map()
    ipc.on('terminal_list', ({ project }) => new Promise((resolve) => pending.set(project, resolve)))

    const first = stores.terminals.loadSessions('/p1')
    const second = stores.terminals.loadSessions('/p2')

    // The ordering that produced the defect: the newer call's response is
    // let all the way through first — awaited here, not just resolved, so
    // the ordering is real and not an accident of unrelated microtask
    // depth — and only afterwards does the older, now-stale response for
    // the project that is no longer open arrive.
    pending.get('/p2')([session({ id: 2, project: '/p2' })])
    await second
    pending.get('/p1')([session({ id: 1, project: '/p1' })])
    await first

    // Both assertions matter together: project alone would pass even if
    // sessions still held /p1's stale rows under /p2's name.
    expect(stores.terminals.terminalState.project).toBe('/p2')
    expect(stores.terminals.terminalState.sessions.map((s) => s.id)).toEqual([2])
  })
})

describe('agent rows', () => {
  it('a row assembles the caption, the translated status and the elapsed time', async () => {
    vi.useFakeTimers({ now: new Date('2026-08-03T10:18:00Z') })
    try {
      const { stores } = await ready()
      const [row] = stores.terminals.agentRows.value
      expect(row.label).toBe('Agent')
      expect(row.tasks).toEqual([])
      expect(row.state).toBe('running')
      expect(row.elapsed).toBe('18m')
    } finally {
      vi.useRealTimers()
    }
  })

  /* The point of the whole change: a column of `claude-1`…`claude-5` said
     nothing about who was doing what. Each intent gets its own caption, and
     the two halves are kept apart because the component sets them
     differently — prose in sans, issue ids in mono. */
  it('every intent names its own work', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 2, work: { kind: 'newTask' } }))
    await emit('terminal:state', session({ id: 3, work: { kind: 'editTask', id: 'smetana-42' } }))
    await emit('terminal:state', session({ id: 4, work: { kind: 'setup' } }))
    await nextTick()

    expect(stores.terminals.agentRows.value.map((r) => [r.label, r.tasks])).toEqual([
      ['Agent', []],
      ['Creating a task', []],
      ['Editing', ['smetana-42']],
      ['Project setup', []]
    ])
  })

  /* The one work in the list that is about a repository rather than an issue.
     Both identifiers are drawn and the repository is drawn by its folder's
     name: the absolute path is most of a 252px row on its own, and the panel
     already says which project this is. */
  it('a conflict names the repository and the branch coming in', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit(
      'terminal:state',
      session({
        id: 2,
        work: { kind: 'resolveConflict', repo: '/p/backend', theirs: 'develop' }
      })
    )
    await nextTick()

    expect(stores.terminals.agentRows.value.at(-1)).toMatchObject({
      label: 'Conflict',
      tasks: ['backend', 'develop']
    })
  })

  /* A session the worker described with something this front end has never
     heard of is an ordinary outcome, not an error: it is still an agent, and a
     row that says so is worth more than a blank one. */
  it('work with no caption of its own still draws a row', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 2, work: { kind: 'somethingLater' } }))
    await emit('terminal:state', session({ id: 3, work: undefined }))
    await nextTick()

    expect(stores.terminals.agentRows.value.map((r) => r.label)).toEqual(['Agent', 'Agent', 'Agent'])
  })

  /* There is no channel saying "this session took that issue": the agent runs
     `bd update --claim` itself and the app only sees the tracker move. So the
     connection is made from the two halves already on the front end — each run
     names the session working, the tracker names what is in progress and under
     whom. The **assignee** is the session's own bd actor (`run_actor` in
     src-tauri/src/terminal/model.rs) — that is what `bd update --claim` writes —
     and it is what keeps two concurrent runs' rows apart.

     Every claimed fixture below carries a person in `owner` and the actor in
     `assignee`, which is what bd actually emits. That is deliberately the shape
     a filter on `owner` cannot pass: reading one field for the other is
     smetana-a5b, and it made every run row read a bare "Agent". */
  it('a run is captioned by the issues it has taken', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 7, work: { kind: 'run' } }))
    await nextTick()

    // Nothing claimed yet: it is an agent, and there is no work to name.
    expect(stores.terminals.agentRows.value.at(-1)).toMatchObject({ label: 'Agent', tasks: [] })

    stores.runs.runsState.project = '/p'
    stores.runs.runsState.runs = [
      { token: 1, project: '/p', session: 7, state: { kind: 'working' } }
    ]
    stores.tracker.trackerState.issues.set('smetana-9', {
      id: 'smetana-9',
      status: 'in_progress',
      owner: 'merazent@gmail.com',
      assignee: 'smetana-run-7'
    })
    stores.tracker.trackerState.issues.set('smetana-42', {
      id: 'smetana-42',
      status: 'in_progress',
      owner: 'merazent@gmail.com',
      assignee: 'smetana-run-7'
    })
    stores.tracker.trackerState.issues.set('smetana-7', { id: 'smetana-7', status: 'open' })
    await nextTick()

    // Sorted, so a second issue appearing does not reorder the first, and only
    // what is actually in progress — an open issue is nobody's work yet.
    expect(stores.terminals.agentRows.value.at(-1)).toMatchObject({
      label: null,
      tasks: ['smetana-42', 'smetana-9']
    })

    // And it belongs to the run's own session, not to every agent on screen.
    expect(stores.terminals.agentRows.value[0]).toMatchObject({ label: 'Agent', tasks: [] })
  })

  /* Two runs going at once is the case the actor filter exists for: with the
     old "everything in_progress" reading, both rows would have carried both
     batches' work. */
  it("two concurrent runs' rows each name their own claims", async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 7, work: { kind: 'run' } }))
    await emit('terminal:state', session({ id: 8, work: { kind: 'run' } }))
    stores.runs.runsState.project = '/p'
    stores.runs.runsState.runs = [
      { token: 1, project: '/p', session: 7, state: { kind: 'working' } },
      { token: 2, project: '/p', session: 8, state: { kind: 'working' } }
    ]
    stores.tracker.trackerState.issues.set('smetana-9', {
      id: 'smetana-9',
      status: 'in_progress',
      owner: 'merazent@gmail.com',
      assignee: 'smetana-run-7'
    })
    stores.tracker.trackerState.issues.set('smetana-42', {
      id: 'smetana-42',
      status: 'in_progress',
      owner: 'merazent@gmail.com',
      assignee: 'smetana-run-8'
    })
    // Claimed by a person, not by either run: nobody's caption.
    stores.tracker.trackerState.issues.set('smetana-3', {
      id: 'smetana-3',
      status: 'in_progress',
      owner: 'merazent@gmail.com',
      assignee: 'flexo'
    })
    await nextTick()

    const rows = stores.terminals.agentRows.value
    expect(rows.find((r) => r.id === 7)).toMatchObject({ tasks: ['smetana-9'] })
    expect(rows.find((r) => r.id === 8)).toMatchObject({ tasks: ['smetana-42'] })
  })

  /* smetana-a5b from the other side, and the whole of why that bug was invisible
     in this file: an issue whose `owner` happens to be the run's actor was never
     claimed by it, because a claim writes `assignee` and leaves `owner` alone. A
     fixture setting only `owner` used to make the caption pass here while the
     app captioned every run row "Agent", so this is the test that has to fail if
     the filter ever moves back. */
  it('an issue carrying the actor in owner alone is not one of the run\'s claims', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 7, work: { kind: 'run' } }))
    stores.runs.runsState.project = '/p'
    stores.runs.runsState.runs = [
      { token: 1, project: '/p', session: 7, state: { kind: 'working' } }
    ]
    stores.tracker.trackerState.issues.set('smetana-9', {
      id: 'smetana-9',
      status: 'in_progress',
      owner: 'smetana-run-7',
      assignee: null
    })
    await nextTick()

    expect(stores.terminals.agentRows.value.at(-1)).toMatchObject({
      label: 'Agent',
      tasks: [],
      claimed: []
    })
  })
})

/* The right-hand column follows the selected agent, and everything it needs to
   draw that agent's work rides on the row: the panel has no second channel to
   the worker, and half the rows are starts with no session behind them at all.
   `claimed` is separate from the caption's `tasks` on purpose — an edit's
   `tasks` is the issue it is editing, which is not a claim. */
describe('what a row says about the work behind it', () => {
  it('the work rides whole, and only a run carries claims', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 3, work: { kind: 'editTask', id: 'smetana-42' } }))
    await nextTick()

    expect(stores.terminals.agentRows.value.map((r) => [r.work?.kind, r.claimed])).toEqual([
      ['bare', []],
      ['editTask', []]
    ])
    expect(stores.terminals.agentRows.value.at(-1).work).toEqual({
      kind: 'editTask',
      id: 'smetana-42'
    })
  })

  it("a run's claims are on the row as well as in its caption", async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 7, work: { kind: 'run' } }))
    stores.runs.runsState.project = '/p'
    stores.runs.runsState.runs = [
      { token: 1, project: '/p', session: 7, state: { kind: 'working' } }
    ]
    stores.tracker.trackerState.issues.set('smetana-9', {
      id: 'smetana-9',
      status: 'in_progress',
      owner: 'merazent@gmail.com',
      assignee: 'smetana-run-7'
    })
    await nextTick()

    // One list, read twice: the caption and the panel on the right cannot
    // disagree about what a run has taken.
    expect(stores.terminals.agentRows.value.at(-1)).toMatchObject({
      claimed: ['smetana-9'],
      tasks: ['smetana-9']
    })
  })

  /* A batch claims the merge lock under its own actor while it merges, and the
     lock is coordination rather than work — so it is left out of the claimed
     list and out of the caption, exactly as the board leaves it out. The second
     issue is the other half of the assertion: the filter is the label and not
     the actor. */
  it('the merge lock it holds is not among its claims, while the rest of them are', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 7, work: { kind: 'run' } }))
    stores.runs.runsState.project = '/p'
    stores.runs.runsState.runs = [
      { token: 1, project: '/p', session: 7, state: { kind: 'working' } }
    ]
    stores.tracker.trackerState.issues.set('smetana-lock-1', {
      id: 'smetana-lock-1',
      title: 'Merge lock',
      status: 'in_progress',
      owner: 'merazent@gmail.com',
      assignee: 'smetana-run-7',
      labels: ['smetana-lock']
    })
    stores.tracker.trackerState.issues.set('smetana-9', {
      id: 'smetana-9',
      status: 'in_progress',
      owner: 'merazent@gmail.com',
      assignee: 'smetana-run-7',
      labels: ['chore']
    })
    await nextTick()

    expect(stores.terminals.agentRows.value.at(-1)).toMatchObject({
      claimed: ['smetana-9'],
      tasks: ['smetana-9']
    })
  })

  /* The whole reason the draft rides in `SessionWork` rather than in a map
     beside the start ticket: a start becomes a session about a second later,
     and the panel must not go blank across the handover. */
  it('a filing agent carries its draft, before and after the worker answers', async () => {
    const { ipc, stores } = await ready()
    let answer
    ipc.on('terminal_create', () => new Promise((resolve) => (answer = resolve)))

    const started = stores.terminals.createSession('/p', {
      kind: 'newTask',
      brainstorm: 'off',
      draft: {
        text: 'The log drops lines above 10k',
        issue_type: 'bug',
        priority: 1,
        images: ['/data/attachments/20260806-121314-mock.png']
      }
    })

    // The placeholder. `issue_type` is what the dialog and bd call the field;
    // `issueType` is what comes back over the wire, and the row has to speak
    // the wire's language or the panel would read Auto over a chosen type for
    // the second the start lasts. The attached image does not come along — it
    // is the agent's briefing, and nothing draws it.
    expect(stores.terminals.agentRows.value.at(-1).work).toEqual({
      kind: 'newTask',
      text: 'The log drops lines above 10k',
      issueType: 'bug',
      priority: 1,
      parent: null
    })

    answer(
      session({
        id: 9,
        work: {
          kind: 'newTask',
          text: 'The log drops lines above 10k',
          issueType: 'bug',
          priority: 1,
          // `SessionWork::NewTask` always serializes the key, null or not.
          parent: null
        }
      })
    )
    await started

    expect(stores.terminals.agentRows.value.at(-1).work).toEqual({
      kind: 'newTask',
      text: 'The log drops lines above 10k',
      issueType: 'bug',
      priority: 1,
      parent: null
    })
  })

  /* Auto travels as absence, never as the word — the same invariant TaskDraft
     and prompt.rs hold on the Rust side. A placeholder that substituted a
     default here would have the draft panel claim a choice nobody made for the
     one second before the session lands. */
  it('a draft left on Auto reaches the row as null', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_create', () => new Promise(() => {}))

    stores.terminals.createSession('/p', {
      kind: 'newTask',
      brainstorm: 'auto',
      draft: { text: 'Something', issue_type: null, priority: null, images: [] }
    })

    expect(stores.terminals.agentRows.value.at(-1).work).toEqual({
      kind: 'newTask',
      text: 'Something',
      issueType: null,
      priority: null,
      parent: null
    })
  })
})

/* The one field a placeholder could drop without anything failing: the panel
   would simply stop drawing the Follow-up to row, then start drawing it a
   second later when the session lands. A flicker is the loudest this can get
   on its own, so it is pinned here instead. */
describe('a follow-up being filed', () => {
  it('carries the parent it was opened from into the placeholder row', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_create', () => new Promise(() => {}))

    stores.terminals.createSession('/p', {
      kind: 'newTask',
      brainstorm: 'off',
      draft: { text: 'The tooltip clips', issue_type: null, priority: null, images: [], parent: 'smetana-3uv' }
    })

    expect(stores.terminals.agentRows.value.at(-1).work).toEqual({
      kind: 'newTask',
      text: 'The tooltip clips',
      issueType: null,
      priority: null,
      parent: 'smetana-3uv'
    })
  })
})

describe('the output stream', () => {
  it('attaching hands the ring snapshot to the subscriber', async () => {
    const { stores } = await ready()
    const seen = []
    stores.terminals.subscribeOutput((bytes, meta) => seen.push({ text: new TextDecoder().decode(bytes), meta }))
    await stores.terminals.attach(1)
    expect(seen).toHaveLength(1)
    expect(seen[0].text).toBe('hello')
    expect(seen[0].meta.reset).toBe(true)
  })

  it('output events reach the subscriber in order', async () => {
    const { stores, emit, nextTick } = await ready()
    const seen = []
    stores.terminals.subscribeOutput((bytes) => seen.push(new TextDecoder().decode(bytes)))
    await stores.terminals.attach(1)
    await emit('terminal:output', { id: 1, seq: 1, data: b64('a') })
    await emit('terminal:output', { id: 1, seq: 2, data: b64('b') })
    await nextTick()
    expect(seen.slice(1)).toEqual(['a', 'b'])
  })

  it('a gap in seq reattaches rather than showing a hole', async () => {
    const { ipc, stores, emit } = await ready()
    const seen = []
    stores.terminals.subscribeOutput((bytes, meta) => seen.push({ text: new TextDecoder().decode(bytes), meta }))
    await stores.terminals.attach(1)
    ipc.on('terminal_attach', { data: b64('whole screen'), seq: 7 })
    await emit('terminal:output', { id: 1, seq: 5, data: b64('lost') })
    /* The reattach starts inside the event listener and therefore does not
       wait for either emit or nextTick: vi.waitFor here is not decoration,
       it is the only way to avoid racing the test itself. */
    await vi.waitFor(() => expect(seen.at(-1).text).toBe('whole screen'))
    expect(seen.at(-1).meta.reset).toBe(true)
    expect(seen.map((s) => s.text)).not.toContain('lost')
  })

  it("another session's output does not reach the subscriber", async () => {
    const { stores, emit, nextTick } = await ready()
    const seen = []
    stores.terminals.subscribeOutput((bytes) => seen.push(new TextDecoder().decode(bytes)))
    await stores.terminals.attach(1)
    await emit('terminal:output', { id: 99, seq: 1, data: b64("somebody else's") })
    await nextTick()
    expect(seen).toEqual(['hello'])
  })
})

describe('detaching', () => {
  it('stops the stream but does not forget the selected agent', async () => {
    const { stores } = await ready()
    await stores.terminals.attach(1)
    expect(stores.terminals.terminalState.activeId).toBe(1)
    await stores.terminals.detach(1)
    // The worker stops streaming to this window, but the human's selection
    // is not the transport's to forget — the agent list highlights this
    // same field, and leaving the terminal tab must not un-pick a row.
    expect(stores.terminals.terminalState.activeId).toBe(1)
  })
})

describe('starting a session', () => {
  it('sends the configured agent and the intent, not a prompt', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('terminal_create', session({ id: 7, agent: 'codex' }))
    stores.settings.settings.agent = 'codex'

    await stores.terminals.createSession('/p', {
      kind: 'editTask',
      id: 'smetana-7',
      title: 'x y'
    })

    const args = ipc.calls('terminal_create').at(-1)
    expect(args.agent).toBe('codex')
    expect(args.intent).toEqual({ kind: 'editTask', id: 'smetana-7', title: 'x y' })
    expect(args.prompt).toBeUndefined()
  })

  it('a session started from the "+ New agent" row carries a bare intent', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('terminal_create', session({ id: 8 }))

    await stores.terminals.createSession('/p')

    expect(ipc.calls('terminal_create').at(-1).intent).toEqual({ kind: 'bare' })
  })

  /* The panel drew its empty state at the exact moment somebody had asked for
     an agent — a spawn takes about a second, and until the worker answered
     there was nothing in the list to draw. */
  it('the row is there and picked before the worker has answered', async () => {
    const { ipc, stores } = await ready()
    let answer
    ipc.on('terminal_create', () => new Promise((resolve) => (answer = resolve)))

    const started = stores.terminals.createSession('/p', { kind: 'bare' })

    const row = stores.terminals.agentRows.value.at(-1)
    expect(row.elapsed).toBe('starting')
    expect(row.starting).toBe(true)
    expect(stores.terminals.terminalState.activeId).toBe(row.id)

    answer(session({ id: 9 }))
    await started

    // The handover: one row, and the selection moves with it rather than being
    // left on a ticket nothing will ever fill.
    expect(stores.terminals.terminalState.starting).toEqual([])
    expect(stores.terminals.agentRows.value.map((r) => r.id)).toEqual([1, 9])
    expect(stores.terminals.terminalState.activeId).toBe(9)
  })

  /* The placeholder is captioned from the very intent being sent, so it says
     what the session will say. Without that the row would be captioned twice
     over the second it exists — once as a generic agent and once as the work —
     and it would visibly change under somebody who had just started reading
     it. */
  it('a start is captioned as the session it becomes', async () => {
    const { ipc, stores } = await ready()
    let answer
    ipc.on('terminal_create', () => new Promise((resolve) => (answer = resolve)))

    const started = stores.terminals.createSession('/p', {
      kind: 'editTask',
      id: 'smetana-42',
      title: 'Some title'
    })
    expect(stores.terminals.agentRows.value.at(-1)).toMatchObject({
      label: 'Editing',
      tasks: ['smetana-42'],
      starting: true
    })

    answer(session({ id: 9, work: { kind: 'editTask', id: 'smetana-42' } }))
    await started

    expect(stores.terminals.agentRows.value.at(-1)).toMatchObject({
      label: 'Editing',
      tasks: ['smetana-42']
    })
  })

  /* A ticket becoming a session is the one move of `activeId` that is a
     continuation rather than a repair, and nothing outside this store can tell
     the two apart afterwards — the ticket is gone from `starting` by then.
     Anything following the selection (the right column's focus, in
     DesktopApp.vue) would otherwise treat the handover as a loss and let go of
     the row a person is still looking at, a second after they filed a task. */
  it('a start becoming a session is announced, ticket and session together', async () => {
    const { ipc, stores } = await ready()
    expect(stores.terminals.lastHandover.value).toBeNull()

    let answer
    ipc.on('terminal_create', () => new Promise((resolve) => (answer = resolve)))
    const started = stores.terminals.createSession('/p', { kind: 'bare' })
    const ticket = stores.terminals.terminalState.activeId

    // Nothing yet: the worker has not answered, and there is no session to
    // hand over to.
    expect(stores.terminals.lastHandover.value).toBeNull()

    answer(session({ id: 9 }))
    await started

    expect(stores.terminals.lastHandover.value).toEqual({ ticket, session: 9 })
    expect(stores.terminals.terminalState.activeId).toBe(9)
  })

  /* The other half: a start whose project was switched away from under it
     handed nothing over *here*. Saying it did would point a follower at a row
     that is not in this panel — the same reason `createSession` sends the
     selection back to `before` rather than to the session in this case. */
  it('a start that landed in another project hands nothing over', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_create', session({ id: 9, project: '/elsewhere' }))

    await stores.terminals.createSession('/p', { kind: 'bare' })

    expect(stores.terminals.lastHandover.value).toBeNull()
  })

  /* Picking another agent while one is starting is a person saying what they
     want to look at, and an answer arriving afterwards does not overrule it. */
  it('a start that lands late does not steal a selection somebody has moved', async () => {
    const { ipc, stores } = await ready()
    let answer
    ipc.on('terminal_create', () => new Promise((resolve) => (answer = resolve)))

    const started = stores.terminals.createSession('/p', { kind: 'bare' })
    stores.terminals.terminalState.activeId = 1
    answer(session({ id: 9 }))
    await started

    expect(stores.terminals.terminalState.activeId).toBe(1)
    expect(stores.terminals.terminalState.sessions.map((s) => s.id)).toEqual([1, 9])
  })

  it('nothing started means no row and the selection back where it was', async () => {
    const { ipc, stores } = await ready()
    stores.terminals.terminalState.activeId = 1
    ipc.fail('terminal_create', { kind: 'noAgent', message: 'claude, codex' })

    await expect(stores.terminals.createSession('/p')).rejects.toBeTruthy()

    expect(stores.terminals.terminalState.starting).toEqual([])
    expect(stores.terminals.agentRows.value.map((r) => r.id)).toEqual([1])
    expect(stores.terminals.terminalState.activeId).toBe(1)
  })

  /* Neither id is one the worker has ever heard of, and asking it about one
     would come back as `no session` — a failure reported at the one moment
     nothing has failed. */
  it('the transport is never asked about an agent that has not started', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_create', () => new Promise(() => {}))
    stores.terminals.createSession('/p', { kind: 'bare' })
    const ticket = stores.terminals.terminalState.activeId

    await stores.terminals.attach(ticket)
    await stores.terminals.detach(ticket)
    await stores.terminals.send(ticket, 'x')
    await stores.terminals.resize(ticket, 80, 24)

    expect(ipc.calls('terminal_attach')).toEqual([])
    expect(ipc.calls('terminal_detach')).toEqual([])
    expect(ipc.calls('terminal_write')).toEqual([])
    expect(ipc.calls('terminal_resize')).toEqual([])
    expect(stores.terminals.terminalState.lastError).toBeNull()
  })

  /* The defect this task was filed for: a list request that went out before the
     session existed comes back without it, and replacing the list wholesale
     dropped both the row and the selection — leaving the agent somebody had
     just started unreachable, with the terminal saying "No agent selected" and
     nothing due to arrive that would put it back. */
  it('a list answer older than the session does not take it away again', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_create', session({ id: 9 }))
    let answer
    ipc.on('terminal_list', () => new Promise((resolve) => (answer = resolve)))

    const listing = stores.terminals.loadSessions('/p')
    await stores.terminals.createSession('/p', { kind: 'bare' })
    // The worker had not made session 9 when it was asked this question.
    answer([session({ id: 1 })])
    await listing

    expect(stores.terminals.terminalState.sessions.map((s) => s.id)).toEqual([1, 9])
    expect(stores.terminals.terminalState.activeId).toBe(9)
  })

  /* A spawn takes about a second and a person can switch project inside it. The
     start is not cancelled — it is somebody's agent and it keeps coming up —
     but it belongs to the panel it was asked for, exactly as a session does. */
  it('a start does not follow the person to another project', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_create', () => new Promise(() => {}))
    stores.terminals.createSession('/p', { kind: 'bare' })
    expect(stores.terminals.agentRows.value.at(-1).starting).toBe(true)

    ipc.on('terminal_list', [])
    await stores.terminals.loadSessions('/elsewhere')

    expect(stores.terminals.agentRows.value).toEqual([])
    expect(stores.terminals.terminalState.activeId).toBeNull()
  })

  /* The other half of the same rule: a row the answer does not mention and that
     was already there when the question went out really is gone. */
  it('a session that was there before the question and is not in the answer goes', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_list', [])
    await stores.terminals.loadSessions('/p')
    expect(stores.terminals.terminalState.sessions).toEqual([])
    expect(stores.terminals.terminalState.activeId).toBeNull()
  })
})

/* The person's own shell. It is a worker session like any other and not an
   agent, and every assertion here is one half of that sentence: it is in
   `sessions`, and it is in nothing else. */
describe('a shell session', () => {
  const shell = (over = {}) => session({ work: { kind: 'shell' }, agent: '/bin/zsh', ...over })

  it('is asked for by project alone — no agent, no intent', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_shell', shell({ id: 5 }))

    const opened = await stores.terminals.createShell('/p')

    // `cwd` null is the project's root, which is what every caller but the file
    // tree's menu means and what this command meant before that menu existed.
    expect(ipc.calls('terminal_shell')).toEqual([{ project: '/p', cwd: null }])
    expect(opened.id).toBe(5)
    expect(stores.terminals.terminalState.sessions.map((s) => s.id)).toEqual([1, 5])
  })

  /* The file tree's menu, which is the one caller that names a folder. The path
     is relative to the project root and travels as it stands: what may be a
     working directory is decided in Rust, beside the spawn, and a second rule
     here would be a second thing to keep true. */
  it('carries the folder it was opened from, untouched', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_shell', shell({ id: 5 }))

    await stores.terminals.createShell('/p', 'src/components')

    expect(ipc.calls('terminal_shell')).toEqual([{ project: '/p', cwd: 'src/components' }])
  })

  /* The agents panel is rows of work, and a shell has none: nothing asked it to
     do anything, and there would be nothing to caption the row with. */
  it('is not a row in the agents panel', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_list', [session({ id: 1 }), shell({ id: 5 })])
    await stores.terminals.loadSessions('/p')

    expect(stores.terminals.agentRows.value.map((row) => row.id)).toEqual([1])
    expect(stores.terminals.shellSessions.value.map((s) => s.id)).toEqual([5])
    expect(stores.terminals.hasAgentSession.value).toBe(true)
  })

  it('does not make a project with only shells in it look like one with agents', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_list', [shell({ id: 5 })])
    await stores.terminals.loadSessions('/p')

    expect(stores.terminals.agentRows.value).toEqual([])
    expect(stores.terminals.hasAgentSession.value).toBe(false)
  })

  /* `activeId` is the row a person picked in the agents panel, and a shell has
     no row there. One that landed in this field would highlight nothing in the
     panel while the Agent tab drew somebody else's shell — the two meanings this
     field used to carry, back again. */
  it('is never what the selection falls back to when an agent goes', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_list', [session({ id: 1 }), shell({ id: 5 })])
    ipc.on('terminal_remove', null)
    await stores.terminals.loadSessions('/p')
    expect(stores.terminals.terminalState.activeId).toBe(1)

    await stores.terminals.removeSession(1)

    expect(stores.terminals.terminalState.activeId).toBeNull()
  })

  /* Attaching is the transport's half; selecting is the person's. The pane for a
     shell attaches without picking anything, which is what lets a terminal tab
     be open while the Agent tab still shows the agent it was showing. */
  it('can be attached to without moving the selected agent', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_list', [session({ id: 1 }), shell({ id: 5 })])
    await stores.terminals.loadSessions('/p')

    await stores.terminals.attach(5)

    expect(stores.terminals.terminalState.activeId).toBe(1)
  })

  /* The other half of that split: output follows what the window is attached to
     and not what the panel has selected, or a shell's tab would sit blank while
     the worker streamed to it. */
  it('its output reaches the subscriber while another session is selected', async () => {
    const { ipc, stores, emit, nextTick } = await ready()
    ipc.on('terminal_list', [session({ id: 1 }), shell({ id: 5 })])
    await stores.terminals.loadSessions('/p')
    const seen = []
    stores.terminals.subscribeOutput((bytes) => seen.push(new TextDecoder().decode(bytes)))

    await stores.terminals.attach(5)
    await emit('terminal:output', { id: 5, seq: 1, data: b64('$ ') })
    await nextTick()

    expect(stores.terminals.terminalState.activeId).toBe(1)
    expect(seen.at(-1)).toBe('$ ')
  })

  /* The condition that makes the split correct, and the one no other test
     reaches. A detach and an attach travel as two IPC calls with no ordering
     guarantee, so the old view's detach can arrive *after* the new one has
     attached — a `streaming = null` written unconditionally would then drop
     every byte of the session the person is now looking at, with no error and
     no event to say why. Two sessions and a late detach is exactly that
     sequence. */
  it('a detach that arrives after the next attach does not silence it', async () => {
    const { ipc, stores, emit, nextTick } = await ready()
    ipc.on('terminal_list', [session({ id: 1 }), session({ id: 2 })])
    await stores.terminals.loadSessions('/p')
    const seen = []
    stores.terminals.subscribeOutput((bytes) => seen.push(new TextDecoder().decode(bytes)))

    await stores.terminals.attach(2)
    // The view being left detaches last, naming the session it is leaving.
    await stores.terminals.detach(1)
    await emit('terminal:output', { id: 2, seq: 1, data: b64('still here') })
    await nextTick()

    expect(seen.at(-1)).toBe('still here')
  })

  /* And the same rule the other way: the session that was left goes quiet, or
     the tab just opened would be written into by the one just closed. */
  it('what the window has left stops reaching the subscriber', async () => {
    const { stores, emit, nextTick } = await ready()
    const seen = []
    stores.terminals.subscribeOutput((bytes) => seen.push(new TextDecoder().decode(bytes)))

    await stores.terminals.attach(1)
    await stores.terminals.detach(1)
    await emit('terminal:output', { id: 1, seq: 1, data: b64('too late') })
    await nextTick()

    expect(seen).not.toContain('too late')
  })
})

describe('back-end errors', () => {
  it('a terminal_attach refusal does not throw but settles into lastError', async () => {
    const { ipc, stores } = await ready()
    ipc.fail('terminal_attach', new Error('boom'))
    await expect(stores.terminals.attach(1)).resolves.toBeUndefined()
    expect(stores.terminals.terminalState.lastError).toEqual({
      title: 'Could not read the terminal',
      description: 'The session list may be out of date. It will catch up on the next change.'
    })
  })

  /* The one refusal a person can do something about, and the reason it is
     worth a message of its own: with filing a task now going through an
     agent, "nothing was created" would be the whole explanation for a
     machine that has no agent to create it with. The names come from the
     error — Rust holds the only copy of that list. */
  it('nothing installed to run says so, and says what was looked for', async () => {
    const { ipc, stores } = await ready()
    ipc.fail('terminal_create', { kind: 'noAgent', message: 'claude, codex' })

    await expect(
      stores.terminals.createSession('/p', { kind: 'bare' })
    ).rejects.toBeTruthy()

    expect(stores.terminals.terminalState.lastError).toEqual({
      title: 'No coding agent is installed',
      description:
        'Smetana looked for claude, codex on your PATH. It starts one to file a task and to edit an issue, so install one and try again.'
    })
  })

  it('any other write refusal is still the generic one', async () => {
    const { ipc, stores } = await ready()
    ipc.fail('terminal_create', { kind: 'spawn', message: 'the agent did not start: boom' })

    await expect(stores.terminals.createSession('/p')).rejects.toBeTruthy()

    expect(stores.terminals.terminalState.lastError).toEqual({
      title: 'Could not complete the action',
      description: 'Nothing was created, removed, or sent.'
    })
  })
})

describe('elapsed time', () => {
  it('reads for a human, not for a machine', async () => {
    const { stores } = await loadStores()
    expect(stores.terminals.formatElapsed(18 * 60 * 1000)).toBe('18m')
    expect(stores.terminals.formatElapsed(2 * 3600_000 + 14 * 60_000)).toBe('2h 14m')
    expect(stores.terminals.formatElapsed(5_000)).toBe('0m')
  })

  /* The row's clock ticks once every thirty seconds, and a session is born
     between ticks: until the next one its startedAt lies in the future relative
     to the last time taken. Without the clamp, floor of a negative gives minus
     an hour and minus a minute — "-1h -1m" in a freshly created row. */
  it('a just-created agent does not go negative', async () => {
    const { stores } = await loadStores()
    expect(stores.terminals.formatElapsed(-1_000)).toBe('0m')
    expect(stores.terminals.formatElapsed(-90 * 60_000)).toBe('0m')
  })
})

/* The scope bar's second counter, which is this list minus the rows that have
   finished. Three cases carry the whole rule: what stops counting, what keeps
   counting when it would be easiest to drop, and whose starts count. */
describe('the live agent count in the scope bar', () => {
  it('an agent that has exited is a row to read, not one that is running', async () => {
    const { stores, emit, nextTick } = await ready()
    expect(stores.terminals.liveAgentCount.value).toBe(1)

    await emit('terminal:state', session({ state: 'exited', exitCode: 0 }))
    await nextTick()

    expect(stores.terminals.terminalState.sessions).toHaveLength(1)
    expect(stores.terminals.liveAgentCount.value).toBe(0)
  })

  /* The one state it would be tempting to drop, and the reason the rule is "not
     exited" rather than "running": an agent waiting for an answer is why
     somebody is looking at this bar at all, and a counter that fell by one on a
     demand for attention would point away from it. */
  it('an agent waiting for a person still counts', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ state: 'needs-you' }))
    await emit('terminal:state', session({ id: 2, state: 'idle' }))
    await nextTick()

    expect(stores.terminals.liveAgentCount.value).toBe(2)
  })

  /* Starts count from the moment their row is drawn — a spawn takes about a
     second and the counter must not disagree with the list beside it for that
     second — but only the ones that belong to the project on screen, which is
     the same filter `agentRows` applies and not a second copy of it. */
  it('a start counts before the worker answers, and only in its own project', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_create', () => new Promise(() => {}))

    stores.terminals.createSession('/other', { kind: 'bare' })
    expect(stores.terminals.terminalState.starting).toHaveLength(1)
    expect(stores.terminals.agentRows.value).toHaveLength(1)
    expect(stores.terminals.liveAgentCount.value).toBe(1)

    stores.terminals.createSession('/p', { kind: 'bare' })
    expect(stores.terminals.agentRows.value).toHaveLength(2)
    expect(stores.terminals.liveAgentCount.value).toBe(2)
  })

  /* A shell is a session of the worker's and not an agent: it has no row in the
     panel this number is read against, so counting one would put the bar one
     ahead of the list with nothing on screen to say why. The two rules meet in
     `agentSessions`, which is what both this counter and `agentRows` filter
     through — this is the test that would have caught them coming apart. */
  it('a shell is not a running agent', async () => {
    const { stores, emit, nextTick } = await ready()
    expect(stores.terminals.liveAgentCount.value).toBe(1)

    await emit('terminal:state', session({ id: 2, work: { kind: 'shell' }, agent: '/bin/zsh' }))
    await emit('terminal:state', session({ id: 3, work: { kind: 'shell' }, agent: '/bin/zsh' }))
    await nextTick()

    // Both shells are in the worker's list, and in neither of the two things a
    // person compares: the count and the rows.
    expect(stores.terminals.terminalState.sessions).toHaveLength(3)
    expect(stores.terminals.liveAgentCount.value).toBe(1)
    expect(stores.terminals.agentRows.value).toHaveLength(1)
  })

  /* The counter and the list, tied together in one assertion, because what the
     bar promises is the list minus the rows that have finished and the two are
     only equal through `toUiState`: `exited` is the one session state it maps
     onto `done` and `failed`, and the counter skips exactly that state. A
     seventh state mapped onto a finished-looking row tomorrow would move the
     list and leave the counter behind, silently, and the arithmetic here is the
     only thing that would notice. */
  it('the count is the list minus the rows that have finished', async () => {
    const { ipc, stores, emit, nextTick } = await ready()
    ipc.on('terminal_create', () => new Promise(() => {}))

    await emit('terminal:state', session({ id: 2, state: 'exited', exitCode: 0 }))
    await emit('terminal:state', session({ id: 3, state: 'exited', exitCode: 1 }))
    await emit('terminal:state', session({ id: 4, state: 'needs-you' }))
    await emit('terminal:state', session({ id: 5, state: 'idle' }))
    /* A shell among them, because the equality is what the criterion is worded
       as and a shell is the one session that must not disturb either side of
       it: it is in the list the worker keeps and in neither of these two. */
    await emit('terminal:state', session({ id: 6, work: { kind: 'shell' }, agent: '/bin/zsh' }))
    await nextTick()
    stores.terminals.createSession('/p', { kind: 'bare' })

    const rows = stores.terminals.agentRows.value
    expect(stores.terminals.terminalState.sessions).toHaveLength(6)
    expect(rows.map((r) => r.state)).toEqual(['running', 'done', 'failed', 'needs-you', 'ready', 'running'])
    expect(stores.terminals.liveAgentCount.value).toBe(
      rows.filter((r) => r.state !== 'done' && r.state !== 'failed').length
    )
    expect(stores.terminals.liveAgentCount.value).toBe(4)
  })
})

/* The same agents split for the scope bar's headline. What these are here to
   hold is the pair of promises the sentence makes: a shell can never produce
   either half of it, and the number in "N agents running" is the number in the
   counter drawn beside it. */
describe('the agent counts behind the headline', () => {
  it('counts the agents waiting on the person, and the rest as live', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 2, state: 'needs-you' }))
    await emit('terminal:state', session({ id: 3, state: 'needs-you' }))
    await emit('terminal:state', session({ id: 4, state: 'idle' }))
    await nextTick()

    expect(stores.terminals.agentCounts.value).toEqual({ loud: 2, live: 2 })
  })

  /* The whole of the blocking defect this computed exists for. A shell rings
     the bell when a build finishes and the worker reads that as `needs-you`
     like any other session, so the rail's per-project map — which is told a
     session's state and never its kind — would have had the bar announce an
     agent waiting on somebody in a project with no agent in it. */
  it('a shell is neither waiting nor running, whatever state it reaches', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 2, work: { kind: 'shell' }, agent: '/bin/zsh', state: 'needs-you' }))
    await emit('terminal:state', session({ id: 3, work: { kind: 'shell' }, agent: '/bin/zsh', state: 'running' }))
    await emit('terminal:state', session({ id: 4, work: { kind: 'shell' }, agent: '/bin/zsh', state: 'starting' }))
    await nextTick()

    expect(stores.terminals.terminalState.sessions).toHaveLength(4)
    expect(stores.terminals.agentCounts.value).toEqual({ loud: 0, live: 1 })
  })

  it('says nothing at all in a project where every agent has finished', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ state: 'exited', exitCode: 0 }))
    await nextTick()

    expect(stores.terminals.agentCounts.value).toEqual({ loud: 0, live: 0 })
  })

  /* The tie to the counter, asserted as arithmetic rather than as two numbers
     that happen to match today: the live sentence is only ever drawn when
     nothing is waiting, and in that case it must read exactly what the `bot`
     counter one gap away reads, tooltip and all. An idle agent — a `ready` row,
     one sitting at its prompt between turns — is the case that had them apart. */
  it('the live half equals the scope bar counter whenever nothing is waiting', async () => {
    const { ipc, stores, emit, nextTick } = await ready()
    ipc.on('terminal_create', () => new Promise(() => {}))

    await emit('terminal:state', session({ id: 2, state: 'idle' }))
    await emit('terminal:state', session({ id: 3, state: 'idle' }))
    await emit('terminal:state', session({ id: 4, state: 'exited', exitCode: 0 }))
    await nextTick()
    stores.terminals.createSession('/p', { kind: 'bare' })

    expect(stores.terminals.agentCounts.value.loud).toBe(0)
    expect(stores.terminals.agentCounts.value.live).toBe(stores.terminals.liveAgentCount.value)
    expect(stores.terminals.liveAgentCount.value).toBe(4)
  })

  /* And with somebody waiting, the two are allowed to differ — the counter
     keeps counting the waiting agent, because it is the reason a person is
     looking at the bar, while the sentence has already moved to the loud one. */
  it('the waiting agent stays in the counter and leaves the live half', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 2, state: 'needs-you' }))
    await nextTick()

    expect(stores.terminals.liveAgentCount.value).toBe(2)
    expect(stores.terminals.agentCounts.value).toEqual({ loud: 1, live: 1 })
  })
})

/* A file dropped on the window. Tauri intercepts the gesture before the webview
   sees it, so these arrive as real events through the same transport the state
   deltas do — the store's part is the units and the plumbing, and nothing else:
   whose drop it is belongs to the pane that can ask what is drawn at the point.

   `onDragDropEvent` is four `listen` calls deep, each one a round trip through
   the mocked transport, so the subscription is not in place on the next
   microtask. */
describe('drops on the window', () => {
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0))

  const watch = async (stores) => {
    const seen = []
    const stop = stores.terminals.watchSessionDrops({
      over: (at) => seen.push({ type: 'over', ...at }),
      leave: () => seen.push({ type: 'leave' }),
      drop: (at) => seen.push({ type: 'drop', ...at })
    })
    await settle()
    return { seen, stop }
  }

  it('hands over the paths and the point they were let go at', async () => {
    const { stores, emit } = await ready()
    const { seen } = await watch(stores)

    await emit('tauri://drag-drop', { paths: ['/tmp/a.png'], position: { x: 40, y: 60 } })
    await settle()

    expect(seen).toEqual([{ type: 'drop', x: 40, y: 60, paths: ['/tmp/a.png'] }])
  })

  /* The event's position is physical and `document.elementFromPoint` reads CSS
     pixels, so the conversion happens here — the one place that knows which
     units arrived. On a retina screen an unconverted point lands at twice the
     distance from the corner, which on a three-column window is a different
     panel altogether. */
  it('turns the physical point into CSS pixels', async () => {
    const { stores, emit } = await ready()
    vi.stubGlobal('devicePixelRatio', 2)
    const { seen } = await watch(stores)

    await emit('tauri://drag-drop', { paths: ['/tmp/a.png'], position: { x: 40, y: 60 } })
    await settle()
    vi.unstubAllGlobals()

    expect(seen).toEqual([{ type: 'drop', x: 20, y: 30, paths: ['/tmp/a.png'] }])
  })

  /* Entering carries the paths and the events in the middle of the drag do not,
     which is the whole reason `over` is handed them at all: a caller saying how
     many files are coming has nowhere else to read the number. */
  it('carries the paths on entering and null while moving', async () => {
    const { stores, emit } = await ready()
    const { seen } = await watch(stores)

    await emit('tauri://drag-enter', { paths: ['/tmp/a.png', '/tmp/b.png'], position: { x: 1, y: 2 } })
    await emit('tauri://drag-over', { position: { x: 3, y: 4 } })
    await settle()

    expect(seen).toEqual([
      { type: 'over', x: 1, y: 2, paths: ['/tmp/a.png', '/tmp/b.png'] },
      { type: 'over', x: 3, y: 4, paths: null }
    ])
  })

  it('the drag leaving the window ends the response', async () => {
    const { stores, emit } = await ready()
    const { seen } = await watch(stores)

    await emit('tauri://drag-leave', {})
    await settle()

    expect(seen).toEqual([{ type: 'leave' }])
  })

  it('after unsubscribing a drop reaches nothing', async () => {
    const { stores, emit } = await ready()
    const { seen, stop } = await watch(stores)

    stop()
    await emit('tauri://drag-drop', { paths: ['/tmp/a.png'], position: { x: 1, y: 2 } })
    await settle()

    expect(seen).toEqual([])
  })

  /* Unmounting before the subscription has finished being set up: the pane goes
     as soon as the person switches tab, and the promise behind
     `onDragDropEvent` may still be in flight — without the flag the listener
     would be installed after its owner was gone and would never come off. */
  it('unsubscribing before the subscription lands still leaves nothing listening', async () => {
    const { stores, emit } = await ready()
    const seen = []
    const stop = stores.terminals.watchSessionDrops({ drop: (at) => seen.push(at) })
    stop()
    await settle()

    await emit('tauri://drag-drop', { paths: ['/tmp/a.png'], position: { x: 1, y: 2 } })
    await settle()

    expect(seen).toEqual([])
  })
})

/* The second structure beside `terminalState.sessions`: the state of every
   project's sessions, which is what a tile on the project rail draws. The list
   above is the active project's and stays that way — a row in the agents panel
   for a project this window is not pointed at would offer a button that kills
   somebody else's process. */
describe('project states', () => {
  it('a project with a running session is live, one with a waiting session is loud', async () => {
    const loaded = await loadStores()
    loaded.ipc.on('terminal_list', [])
    loaded.ipc.on('terminal_marks', [
      mark({ id: 1, project: '/a', state: 'running' }),
      mark({ id: 2, project: '/b', state: 'needs-you' })
    ])
    await loaded.stores.terminals.initTerminals()

    expect(loaded.stores.terminals.projectState('/a')).toBe('live')
    expect(loaded.stores.terminals.projectState('/b')).toBe('loud')
  })

  it('a waiting session outweighs a running one in the same project', async () => {
    const loaded = await loadStores()
    loaded.ipc.on('terminal_list', [])
    loaded.ipc.on('terminal_marks', [
      mark({ id: 1, project: '/a', state: 'running' }),
      mark({ id: 2, project: '/a', state: 'needs-you' })
    ])
    await loaded.stores.terminals.initTerminals()

    expect(loaded.stores.terminals.projectState('/a')).toBe('loud')
    expect(loaded.stores.terminals.projectStates.value['/a']).toEqual({ state: 'loud', live: 1, loud: 1 })
  })

  /* The rail counts agents and not the person's own shells, and the pair below
     is one fixture read twice. The kind is the only thing that differs between
     them, so the second test cannot pass on an empty map: the first proves the
     very same rows reach it and are counted. */
  const busy = [
    mark({ id: 1, project: '/a', state: 'needs-you' }),
    mark({ id: 2, project: '/a', state: 'running' }),
    mark({ id: 3, project: '/a', state: 'starting' })
  ]

  it('sessions waiting, working and starting all count towards their project', async () => {
    const loaded = await loadStores()
    loaded.ipc.on('terminal_list', [])
    loaded.ipc.on('terminal_marks', busy.map((m) => ({ ...m, kind: 'run' })))
    await loaded.stores.terminals.initTerminals()

    expect(loaded.stores.terminals.projectState('/a')).toBe('loud')
    expect(loaded.stores.terminals.projectStates.value['/a']).toEqual({ state: 'loud', live: 2, loud: 1 })
  })

  it("the same sessions as the person's own shells leave the project idle", async () => {
    const loaded = await loadStores()
    loaded.ipc.on('terminal_list', [])
    loaded.ipc.on('terminal_marks', busy.map((m) => ({ ...m, kind: 'shell' })))
    await loaded.stores.terminals.initTerminals()

    expect(loaded.stores.terminals.projectState('/a')).toBe('idle')
    expect(loaded.stores.terminals.projectStates.value['/a']).toBeUndefined()
  })

  /* The path the reproduction actually takes: the first read is one snapshot,
     and a shell opened after it arrives as an event. */
  it('a shell that arrives by event does not light its project either', async () => {
    const loaded = await loadStores()
    loaded.ipc.on('terminal_list', [])
    loaded.ipc.on('terminal_marks', [])
    await loaded.stores.terminals.initTerminals()
    await loaded.stores.terminals.loadSessions('/p')

    const shell = { project: '/other', work: { kind: 'shell' }, agent: '/bin/zsh' }
    await loaded.emit('terminal:state', session({ id: 9, ...shell, state: 'running' }))
    await loaded.emit('terminal:state', session({ id: 10, ...shell, state: 'needs-you' }))
    await loaded.nextTick()
    expect(loaded.stores.terminals.projectState('/other')).toBe('idle')

    // and an agent in the same project, by the same path, still does
    await loaded.emit('terminal:state', session({ id: 11, project: '/other', state: 'needs-you' }))
    await loaded.nextTick()
    expect(loaded.stores.terminals.projectState('/other')).toBe('loud')
  })

  it('a project whose sessions have all exited, and one nobody has heard of, are both idle', async () => {
    const loaded = await loadStores()
    loaded.ipc.on('terminal_list', [])
    loaded.ipc.on('terminal_marks', [mark({ id: 1, project: '/a', state: 'exited' })])
    await loaded.stores.terminals.initTerminals()

    expect(loaded.stores.terminals.projectState('/a')).toBe('idle')
    expect(loaded.stores.terminals.projectState('/nowhere')).toBe('idle')
  })

  it("an event about another project moves that project's state and adds no row here", async () => {
    const loaded = await loadStores()
    loaded.ipc.on('terminal_list', [])
    loaded.ipc.on('terminal_marks', [])
    await loaded.stores.terminals.initTerminals()
    await loaded.stores.terminals.loadSessions('/p')

    await loaded.emit('terminal:state', session({ id: 9, project: '/other', state: 'needs-you' }))
    await loaded.nextTick()

    expect(loaded.stores.terminals.projectState('/other')).toBe('loud')
    // and the panel's own list is still only the active project's
    expect(loaded.stores.terminals.terminalState.sessions).toHaveLength(0)
  })

  it('a removed session stops counting towards its project', async () => {
    const loaded = await loadStores()
    loaded.ipc.on('terminal_list', [])
    loaded.ipc.on('terminal_marks', [mark({ id: 3, project: '/a', state: 'running' })])
    await loaded.stores.terminals.initTerminals()
    expect(loaded.stores.terminals.projectState('/a')).toBe('live')

    await loaded.emit('terminal:removed', { id: 3 })
    await loaded.nextTick()

    expect(loaded.stores.terminals.projectState('/a')).toBe('idle')
  })

  /* Nothing else in the window depends on this read, so it is not allowed to
     take the rest of the store down with it: the agents panel is the whole
     point of this store and it is fed by other commands entirely. */
  it('a failing marks read is reported and leaves the rest of the store working', async () => {
    const loaded = await loadStores()
    const complained = vi.spyOn(console, 'error').mockImplementation(() => {})
    loaded.ipc.fail('terminal_marks', new Error('the terminal worker is not running'))
    loaded.ipc.on('terminal_list', [session()])

    await expect(loaded.stores.terminals.initTerminals()).resolves.toBeUndefined()
    await loaded.stores.terminals.loadSessions('/p')

    expect(complained).toHaveBeenCalled()
    expect(loaded.stores.terminals.terminalState.ready).toBe(true)
    expect(loaded.stores.terminals.terminalState.sessions).toHaveLength(1)
    expect(loaded.stores.terminals.projectState('/p')).toBe('idle')
    complained.mockRestore()
  })
})

describe('the sound an agent waiting for an answer makes', () => {
  it('plays on the way into needs-you, and not on every event after it', async () => {
    const { emit, stores, nextTick } = await ready()
    stores.settings.settings.notifications.needsAttention = 'sound-4'

    await emit('terminal:state', session({ state: 'needs-you', question: { text: 'May I?' } }))
    await nextTick()
    expect(chime).toHaveBeenCalledWith('sound-4')

    await emit('terminal:state', session({ state: 'needs-you', question: { text: 'May I?' } }))
    await nextTick()
    expect(chime).toHaveBeenCalledTimes(1)

    // Answered, then asked again: that is a second wait and a second sound.
    await emit('terminal:state', session({ state: 'running' }))
    await emit('terminal:state', session({ state: 'needs-you', question: { text: 'And this?' } }))
    await nextTick()
    expect(chime).toHaveBeenCalledTimes(2)
  })

  it('a session of another project is announced too', async () => {
    // The marks cover every project — that is what the rail's dots are drawn
    // from — and somebody supervising two projects overnight is waiting on
    // both, which a watcher over `terminalState.sessions` could not have said.
    const { emit, nextTick } = await ready()

    await emit('terminal:state', session({ id: 9, project: '/other', state: 'needs-you' }))
    await nextTick()

    expect(chime).toHaveBeenCalledTimes(1)
  })

  it('sessions already waiting when the window opens are silent', async () => {
    // The first read of `terminal_marks` is the past, not an event. Announcing
    // it would make starting the app a noise about something that happened
    // before it.
    const loaded = await loadStores()
    loaded.ipc.on('terminal_list', [])
    loaded.ipc.on('terminal_marks', [mark({ id: 4, state: 'needs-you' })])
    await loaded.stores.terminals.initTerminals()

    expect(chime).not.toHaveBeenCalled()
  })

  it("a shell ringing its own bell is silent, and an agent beside it is not", async () => {
    /* A shell reaches `needs-you` by the shortest path there is — any BEL byte
       becomes `NeedsYou` through layer A of `detect.rs`, no profile involved —
       so an ambiguous tab completion would otherwise play the sound at somebody
       typing into that very tab. `projectStates` skips shells for the same
       reason and by the same word; without this the sound would be the third
       population, and the loud one. */
    const { emit, nextTick } = await ready()

    await emit('terminal:state', session({ id: 5, state: 'needs-you', work: { kind: 'shell' } }))
    await nextTick()
    expect(chime).not.toHaveBeenCalled()

    await emit('terminal:state', session({ id: 6, state: 'needs-you' }))
    await nextTick()
    expect(chime).toHaveBeenCalledTimes(1)
  })

  it('work this front end has never heard of still rings', async () => {
    // The question is "is a shell", not "is an agent": an unknown kind is an
    // agent everywhere else in this file, and must not go quiet here.
    const { emit, nextTick } = await ready()

    await emit('terminal:state', session({ id: 7, state: 'needs-you', work: { kind: 'audit' } }))
    await nextTick()

    expect(chime).toHaveBeenCalledTimes(1)
  })

  it('off is silence, not a default sound', async () => {
    const { emit, stores, nextTick } = await ready()
    stores.settings.settings.notifications.needsAttention = 'off'

    await emit('terminal:state', session({ state: 'needs-you' }))
    await nextTick()

    // `chime` refuses `off` itself, so there is one place that knows what
    // silence is.
    expect(chime).toHaveBeenCalledWith('off')
  })
})
