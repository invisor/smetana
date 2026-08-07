import { describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

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

async function ready() {
  const loaded = await loadStores()
  loaded.ipc.on('terminal_list', [session()])
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
  it('a row assembles the caption, the translated status, the question and the elapsed time', async () => {
    vi.useFakeTimers({ now: new Date('2026-08-03T10:18:00Z') })
    try {
      const { stores } = await ready()
      const [row] = stores.terminals.agentRows.value
      expect(row.label).toBe('Agent')
      expect(row.tasks).toEqual([])
      // The process name survives, but only for the question block: what asks
      // is one particular agent, not the job it was handed.
      expect(row.process).toBe('claude-1')
      expect(row.state).toBe('running')
      expect(row.question).toBeNull()
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
     connection is made from the two halves already on the front end — the run
     names the session working, the tracker names what is in progress. */
  it('a run is captioned by the issues it has taken', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 7, work: { kind: 'run' } }))
    await nextTick()

    // Nothing claimed yet: it is an agent, and there is no work to name.
    expect(stores.terminals.agentRows.value.at(-1)).toMatchObject({ label: 'Agent', tasks: [] })

    stores.runs.runsState.project = '/p'
    stores.runs.runsState.run = { project: '/p', session: 7, state: { kind: 'working' } }
    stores.tracker.trackerState.issues.set('smetana-9', { id: 'smetana-9', status: 'in_progress' })
    stores.tracker.trackerState.issues.set('smetana-42', { id: 'smetana-42', status: 'in_progress' })
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
    stores.runs.runsState.run = { project: '/p', session: 7, state: { kind: 'working' } }
    stores.tracker.trackerState.issues.set('smetana-9', { id: 'smetana-9', status: 'in_progress' })
    await nextTick()

    // One list, read twice: the caption and the panel on the right cannot
    // disagree about what a run has taken.
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
      priority: 1
    })

    answer(
      session({
        id: 9,
        work: {
          kind: 'newTask',
          text: 'The log drops lines above 10k',
          issueType: 'bug',
          priority: 1
        }
      })
    )
    await started

    expect(stores.terminals.agentRows.value.at(-1).work).toEqual({
      kind: 'newTask',
      text: 'The log drops lines above 10k',
      issueType: 'bug',
      priority: 1
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
      priority: null
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
    expect(row.process).toBe('claude')
    expect(row.elapsed).toBe('starting')
    expect(row.starting).toBe(true)
    expect(stores.terminals.terminalState.activeId).toBe(row.id)

    answer(session({ id: 9 }))
    await started

    // The handover: one row, and the selection moves with it rather than being
    // left on a ticket nothing will ever fill.
    expect(stores.terminals.terminalState.starting).toEqual([])
    expect(stores.terminals.agentRows.value.map((r) => r.process)).toEqual(['claude-1', 'claude-9'])
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
    expect(stores.terminals.agentRows.value.map((r) => r.process)).toEqual(['claude-1'])
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

describe('answering the question', () => {
  it('the button sends what the profile named', async () => {
    const { ipc, stores } = await ready()
    await stores.terminals.answer(1, { label: 'Yes', send: '1\r' })
    expect(ipc.calls('terminal_write')).toEqual([{ id: 1, data: '1\r' }])
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
