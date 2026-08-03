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
  it('a row assembles the name, the translated status, the question and the elapsed time', async () => {
    vi.useFakeTimers({ now: new Date('2026-08-03T10:18:00Z') })
    try {
      const { stores } = await ready()
      const [row] = stores.terminals.agentRows.value
      expect(row.name).toBe('claude-1')
      expect(row.state).toBe('running')
      expect(row.question).toBeNull()
      expect(row.elapsed).toBe('18m')
    } finally {
      vi.useRealTimers()
    }
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

describe('creating a session', () => {
  /* An agent started from the "+ New agent" row opens on nothing; one started
     from a task opens on that task. The prompt travels with the create call,
     not as a write afterwards — bytes sent into an agent that has not finished
     starting are simply lost. */
  it('carries the opening prompt, and sends null when there is none', async () => {
    const { ipc, stores } = await ready()
    ipc.on('terminal_create', () => session({ id: 2 }))

    await stores.terminals.createSession('/p')
    await stores.terminals.createSession('/p', 'Update bd issue bd-1: ')

    expect(ipc.calls('terminal_create')).toEqual([
      { project: '/p', prompt: null },
      { project: '/p', prompt: 'Update bd issue bd-1: ' }
    ])
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
