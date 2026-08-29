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
  /* Its own field rather than the title, and different from it on purpose: the
     worker titles a row with Claude Code's generated line when there is one, so
     a fixture holding the same string twice would agree with a build that had
     gone back to reading `title` for the opened card. */
  firstPrompt: 'What happened to the scope bar count',
  lastRole: 'assistant',
  lastText: 'Done.',
  messages: 12,
  subagents: 0,
  model: 'claude-opus-5',
  modifiedAt: '2026-08-28T12:00:00Z',
  /* The one field no row draws: the delete confirmation names it. Here so that
     this fourth hand-written copy of `SessionSummary` stays the shape the
     worker sends — it is the copy that fails loudly, which is the whole reason
     it is written out. */
  size: 148_392,
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

describe('the three verbs that ask the desktop', () => {
  it('asks the worker to hand each path over, by its own command', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_open_log', null).on('sessions_open_cwd', null).on('sessions_reveal', null)

    expect(await stores.sessions.openSessionLog('/p/a.jsonl')).toBe(null)
    expect(await stores.sessions.openSessionDirectory('/dev/p')).toBe(null)
    expect(await stores.sessions.revealSessionLog('/p/a.jsonl')).toBe(null)

    expect(ipc.calls('sessions_open_log')).toEqual([{ path: '/p/a.jsonl' }])
    expect(ipc.calls('sessions_open_cwd')).toEqual([{ path: '/dev/p' }])
    expect(ipc.calls('sessions_reveal')).toEqual([{ path: '/p/a.jsonl' }])
  })

  /* The list is read when the tab is opened and never watched, so a row whose
     file has since gone is the ordinary case rather than an exotic one. What
     the caller needs back is the sentence, because it goes straight on the
     screen — a silent refusal is the one outcome the acceptance criteria rule
     out. */
  it('hands back the refusal as words rather than swallowing it', async () => {
    const { ipc, stores } = await loadStores()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('sessions_open_log', 'The transcript is no longer on disk.')

    expect(await stores.sessions.openSessionLog('/p/gone.jsonl')).toBe(
      'The transcript is no longer on disk.'
    )
  })

  /* The whole of the reveal's reason for being a command of ours: it has to be
     able to say *this*, and `revealInFileManager` in `app.js` answers a boolean
     whose only sentence is about a browser having no file manager. */
  it('lets the reveal say the transcript has gone, which a boolean could not', async () => {
    const { ipc, stores } = await loadStores()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('sessions_reveal', 'The transcript is no longer on disk.')

    expect(await stores.sessions.revealSessionLog('/p/gone.jsonl')).toBe(
      'The transcript is no longer on disk.'
    )
  })

  /* A channel that broke rejects with an `Error` rather than with Rust's
     string — a browser's mock is exactly that — and `[object Object]` on a
     toast explains nothing. */
  it('turns an error object into a sentence too', async () => {
    const { ipc, stores } = await loadStores()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('sessions_open_log', new Error('the command is not implemented'))

    expect(await stores.sessions.openSessionLog('/p/a.jsonl')).toBe(
      'the command is not implemented'
    )
  })

  /* A session with no working directory recorded has nothing to open, and the
     command is not asked at all. Each verb says which thing is missing, which
     is the same distinction the worker keeps on the other side of the wire. */
  it('does not ask about a path there is none of, and names what is missing', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_open_log', null).on('sessions_open_cwd', null).on('sessions_reveal', null)

    expect(await stores.sessions.openSessionLog(null)).toBe('There is no transcript to open.')
    expect(await stores.sessions.openSessionDirectory(null)).toBe(
      'This session recorded no working directory.'
    )
    expect(await stores.sessions.revealSessionLog('')).toBe('There is no transcript to show.')

    expect(ipc.calls('sessions_open_log')).toEqual([])
    expect(ipc.calls('sessions_open_cwd')).toEqual([])
    expect(ipc.calls('sessions_reveal')).toEqual([])
  })
})

describe('deleting a transcript', () => {
  /* The whole of the destructive half: the file goes and the row goes with it.
     The row leaving is this store's own consequence — nothing re-reads the disk
     — so it is checked here rather than assumed. */
  it('deletes the file and takes the row out of the list', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_list', [session('a'), session('b')])
    await stores.sessions.loadSessionHistory('/p')
    ipc.on('sessions_delete', null)

    const failure = await stores.sessions.deleteSessionTranscript(
      '/Users/you/.claude/projects/-p/a.jsonl'
    )

    expect(failure).toBe(null)
    expect(ipc.calls('sessions_delete')).toEqual([
      { path: '/Users/you/.claude/projects/-p/a.jsonl' }
    ])
    expect(stores.sessions.sessionsState.sessions.map((row) => row.id)).toEqual(['b'])
  })

  /* The other half of the same criterion: the answer was no, so nothing was
     asked and the file is where it was. The refusal is drawn from a dialog this
     store never sees, which is exactly why the check here is that the store
     makes no call of its own. */
  it('asks nothing at all until somebody confirms', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_list', [session('a')])
    await stores.sessions.loadSessionHistory('/p')
    ipc.on('sessions_delete', null)

    expect(ipc.calls('sessions_delete')).toEqual([])
    expect(stores.sessions.sessionsState.sessions.map((row) => row.id)).toEqual(['a'])
  })

  /* A delete the worker refused — the ordinary reason being a transcript that
     had already gone — leaves the list untouched and answers with the sentence.
     Taking the row out anyway would be this store claiming to know something
     about a disk it has not read since the tab was opened. */
  it('leaves the row alone when the delete was refused, and says why', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_list', [session('a')])
    await stores.sessions.loadSessionHistory('/p')
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('sessions_delete', 'The transcript is no longer on disk.')

    const failure = await stores.sessions.deleteSessionTranscript(
      '/Users/you/.claude/projects/-p/a.jsonl'
    )

    expect(failure).toBe('The transcript is no longer on disk.')
    expect(stores.sessions.sessionsState.sessions.map((row) => row.id)).toEqual(['a'])
  })

  /* A path this list does not hold is deleted all the same — the command is
     the authority on what is on disk — and nothing is spliced out from under
     the rows that are there. */
  it('takes nothing out of the list for a path the list does not hold', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_list', [session('a')])
    await stores.sessions.loadSessionHistory('/p')
    ipc.on('sessions_delete', null)

    expect(await stores.sessions.deleteSessionTranscript('/elsewhere/x.jsonl')).toBe(null)
    expect(stores.sessions.sessionsState.sessions.map((row) => row.id)).toEqual(['a'])
  })

  it('does not ask about a transcript there is no path for', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('sessions_delete', null)

    expect(await stores.sessions.deleteSessionTranscript('')).toBe(
      'There is no transcript to delete.'
    )
    expect(ipc.calls('sessions_delete')).toEqual([])
  })
})
