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
// outside ASCII (see 'вывод чужой сессии подписчику не идёт' below).
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

describe('перевод состояний', () => {
  it('exited с нулём — done, с ненулём — failed', async () => {
    const { stores } = await loadStores()
    expect(stores.terminals.toUiState(session({ state: 'exited', exitCode: 0 }))).toBe('done')
    expect(stores.terminals.toUiState(session({ state: 'exited', exitCode: 1 }))).toBe('failed')
  })

  it('простой тихий, а не готовый к работе агент', async () => {
    const { stores } = await loadStores()
    expect(stores.terminals.toUiState(session({ state: 'idle' }))).toBe('ready')
    expect(stores.terminals.toUiState(session({ state: 'needs-you' }))).toBe('needs-you')
    expect(stores.terminals.toUiState(session({ state: 'starting' }))).toBe('running')
  })
})

describe('список сессий', () => {
  it('событие состояния обновляет строку на месте', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ state: 'needs-you' }))
    await nextTick()
    expect(stores.terminals.terminalState.sessions[0].state).toBe('needs-you')
    expect(stores.terminals.terminalState.sessions).toHaveLength(1)
  })

  it('событие про незнакомую сессию добавляет её', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 2 }))
    await nextTick()
    expect(stores.terminals.terminalState.sessions.map((s) => s.id)).toEqual([1, 2])
  })

  it('сессии чужого проекта в список не попадают', async () => {
    const { stores, emit, nextTick } = await ready()
    await emit('terminal:state', session({ id: 3, project: '/other' }))
    await nextTick()
    expect(stores.terminals.terminalState.sessions.map((s) => s.id)).toEqual([1])
  })
})

describe('смена проекта', () => {
  it('устаревший ответ из старого проекта не переживает переключение', async () => {
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

describe('строки агентов', () => {
  it('строка собирает имя, переведённый статус, вопрос и время работы', async () => {
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

describe('поток вывода', () => {
  it('подключение отдаёт снимок кольца подписчику', async () => {
    const { stores } = await ready()
    const seen = []
    stores.terminals.subscribeOutput((bytes, meta) => seen.push({ text: new TextDecoder().decode(bytes), meta }))
    await stores.terminals.attach(1)
    expect(seen).toHaveLength(1)
    expect(seen[0].text).toBe('hello')
    expect(seen[0].meta.reset).toBe(true)
  })

  it('события вывода доходят до подписчика по порядку', async () => {
    const { stores, emit, nextTick } = await ready()
    const seen = []
    stores.terminals.subscribeOutput((bytes) => seen.push(new TextDecoder().decode(bytes)))
    await stores.terminals.attach(1)
    await emit('terminal:output', { id: 1, seq: 1, data: b64('a') })
    await emit('terminal:output', { id: 1, seq: 2, data: b64('b') })
    await nextTick()
    expect(seen.slice(1)).toEqual(['a', 'b'])
  })

  it('разрыв seq переподключает, а не показывает дыру', async () => {
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

  it('вывод чужой сессии подписчику не идёт', async () => {
    const { stores, emit, nextTick } = await ready()
    const seen = []
    stores.terminals.subscribeOutput((bytes) => seen.push(new TextDecoder().decode(bytes)))
    await stores.terminals.attach(1)
    await emit('terminal:output', { id: 99, seq: 1, data: b64('чужое') })
    await nextTick()
    expect(seen).toEqual(['hello'])
  })
})

describe('отключение', () => {
  it('останавливает поток, но не забывает выбор агента', async () => {
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

describe('ошибки бэкенда', () => {
  it('отказ terminal_attach не бросает, а оседает в lastError', async () => {
    const { ipc, stores } = await ready()
    ipc.fail('terminal_attach', new Error('boom'))
    await expect(stores.terminals.attach(1)).resolves.toBeUndefined()
    expect(stores.terminals.terminalState.lastError).toEqual({
      title: 'Could not read the terminal',
      description: 'The session list may be out of date. It will catch up on the next change.'
    })
  })
})

describe('ответ на вопрос', () => {
  it('кнопка шлёт то, что назвал профиль', async () => {
    const { ipc, stores } = await ready()
    await stores.terminals.answer(1, { label: 'Yes', send: '1\r' })
    expect(ipc.calls('terminal_write')).toEqual([{ id: 1, data: '1\r' }])
  })
})

describe('время работы', () => {
  it('читается человеком, а не машиной', async () => {
    const { stores } = await loadStores()
    expect(stores.terminals.formatElapsed(18 * 60 * 1000)).toBe('18m')
    expect(stores.terminals.formatElapsed(2 * 3600_000 + 14 * 60_000)).toBe('2h 14m')
    expect(stores.terminals.formatElapsed(5_000)).toBe('0m')
  })
})
