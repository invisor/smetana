import { describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

/* A transcript as `sessions_list` hands it over, with only the fields a test
   cares about spelled out per case. The defaults are an ordinary session: the
   shape is the wire contract, and writing it once here is what keeps a test
   about sorting from quietly disagreeing with it. */
const session = (id, over = {}) => ({
  id,
  path: `/Users/you/.claude/projects/-p/${id}.jsonl`,
  cwd: '/p',
  branch: 'main',
  title: 'A conversation',
  lastRole: 'assistant',
  lastText: 'Done.',
  messages: 12,
  subagents: 0,
  model: 'claude-opus-5',
  modifiedAt: '2026-08-28T12:00:00Z',
  ...over
})

describe('the sessions a project has on disk', () => {
  it('the list is read and lands in the store', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_list', [session('a')])

    await stores.sessions.loadSessionHistory('/p')

    expect(stores.sessions.sessionsState.sessions).toEqual([session('a')])
    expect(stores.sessions.sessionsState.project).toBe('/p')
    expect(ipc.calls('sessions_list')).toEqual([{ project: '/p' }])
  })

  /* The opposite order to `agentRows`, and deliberately: this list is
     historical, so "recent sessions" reads literally. The worker's own order is
     not relied on — it walks a directory, and a directory has no order worth
     depending on. */
  it('the newest session is first whatever order it arrives in', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_list', [
      session('old', { modifiedAt: '2026-01-01T00:00:00Z' }),
      session('newest', { modifiedAt: '2026-08-28T12:00:00Z' }),
      session('middle', { modifiedAt: '2026-06-15T09:30:00Z' })
    ])

    await stores.sessions.loadSessionHistory('/p')

    expect(stores.sessions.sessionsState.sessions.map((row) => row.id)).toEqual([
      'newest',
      'middle',
      'old'
    ])
  })

  /* A record nobody can date sorts to the bottom rather than to the top, where
     it would claim to be the most recent thing that happened. */
  it('a session with an unreadable date goes last', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_list', [
      session('undated', { modifiedAt: 'not a date' }),
      session('dated', { modifiedAt: '2026-01-01T00:00:00Z' })
    ])

    await stores.sessions.loadSessionHistory('/p')

    expect(stores.sessions.sessionsState.sessions.map((row) => row.id)).toEqual([
      'dated',
      'undated'
    ])
  })

  /* An empty answer is the ordinary outcome for a machine with no
     `~/.claude/projects` at all, and the panel has an empty state for it. The
     command never rejects, so nothing here is an error path. */
  it('a project with nothing on disk gets an empty list, not a failure', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_list', [])

    await stores.sessions.loadSessionHistory('/p')

    expect(stores.sessions.sessionsState.sessions).toEqual([])
    expect(stores.sessions.sessionsState.loading).toBe(false)
  })

  /* The rule the panel rests on: sessions of a project somebody has left must
     never be on screen under the name of the one they are looking at. */
  it('another project\'s sessions go the moment this one is asked about', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_list', [session('a')])
    await stores.sessions.loadSessionHistory('/p')

    ipc.on('sessions_list', () => new Promise(() => {}))
    stores.sessions.loadSessionHistory('/q')

    expect(stores.sessions.sessionsState.sessions).toEqual([])
    expect(stores.sessions.sessionsState.project).toBe('/q')
  })

  /* The other half of it: re-opening the tab on the same project reads again
     without blinking the column empty and back. */
  it('this project\'s sessions stay on screen while they are read again', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_list', [session('a')])
    await stores.sessions.loadSessionHistory('/p')

    let answer
    ipc.on('sessions_list', () => new Promise((resolve) => (answer = resolve)))
    const again = stores.sessions.loadSessionHistory('/p')

    expect(stores.sessions.sessionsState.sessions.map((row) => row.id)).toEqual(['a'])
    answer([session('b')])
    await again
    expect(stores.sessions.sessionsState.sessions.map((row) => row.id)).toEqual(['b'])
  })

  it('with no project there is nothing to list and nothing to ask', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_list', [session('a')])
    await stores.sessions.loadSessionHistory('/p')

    await stores.sessions.loadSessionHistory(null)

    expect(stores.sessions.sessionsState.sessions).toEqual([])
    expect(stores.sessions.sessionsState.loading).toBe(false)
    expect(ipc.calls('sessions_list')).toEqual([{ project: '/p' }])
  })

  /* The command itself does not reject — a missing directory, an unreadable one
     and a corrupt transcript all come back as fewer rows. Getting here means
     the call failed, which leaves nothing to draw rather than somebody else's
     list. */
  it('a failed call leaves nothing rather than a list nobody confirmed', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_list', [session('a')])
    await stores.sessions.loadSessionHistory('/p')

    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('sessions_list', 'it broke')
    await stores.sessions.loadSessionHistory('/p')

    expect(stores.sessions.sessionsState.sessions).toEqual([])
    expect(stores.sessions.sessionsState.loading).toBe(false)
  })

  /* The same guard `git.js` carries: two reads can be in flight with no
     ordering guarantee on which invoke resolves first, so the last call wins
     rather than the last answer — otherwise the column would list one project's
     sessions under another project's name. */
  it('a stale answer does not overwrite the new project\'s sessions', async () => {
    const { stores } = await loadStores()
    const pending = new Map()
    const { mockIPC } = await import('@tauri-apps/api/mocks')
    mockIPC((cmd, args) => new Promise((resolve) => pending.set(args.project, resolve)))

    const slow = stores.sessions.loadSessionHistory('/old')
    const fast = stores.sessions.loadSessionHistory('/new')

    pending.get('/new')([session('new-one')])
    await fast
    pending.get('/old')([session('old-one')])
    await slow

    expect(stores.sessions.sessionsState.sessions.map((row) => row.id)).toEqual(['new-one'])
  })

  /* What the empty state waits for. The sentence under it is a claim about the
     disk, and it must not be made in the moment before anybody has looked. */
  it('the read is marked as out until its answer lands', async () => {
    const { ipc, stores } = await loadStores()
    let answer
    ipc.on('sessions_list', () => new Promise((resolve) => (answer = resolve)))

    const reading = stores.sessions.loadSessionHistory('/p')
    expect(stores.sessions.sessionsState.loading).toBe(true)

    answer([session('a')])
    await reading
    expect(stores.sessions.sessionsState.loading).toBe(false)
  })
})
