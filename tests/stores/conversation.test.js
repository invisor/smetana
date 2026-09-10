import { describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

/* One journal event as the worker writes it: `seq`, `at`, and the kind's own
   fields flattened in beside a kebab-case `kind` — the serde shape of
   `session::model::Event`, which is the contract rather than a convenience. */
const event = (seq, kind, over = {}) => ({ seq, at: '2026-09-10T12:00:00Z', kind, ...over })

const text = (seq, body) => event(seq, 'text', { text: body })

const permission = (seq, id = 'q1') =>
  event(seq, 'permission', { id, tool: 'Bash', detail: 'rm -rf /tmp/x', options: ['allow', 'deny'] })

/* A graph with the one read every test here needs already answered. The
   snapshot is the argument, since what `session_attach` hands back is the whole
   of what a conversation starts as. */
async function ready(snapshot = { events: [], seq: 0, state: 'ready' }) {
  const loaded = await loadStores()
  loaded.ipc.on('session_attach', snapshot)
  return loaded
}

describe('the conversation store', () => {
  it('takes the snapshot the worker hands back on attach', async () => {
    const { ipc, stores } = await ready({ events: [text(1, 'hello')], seq: 1, state: 'ready' })
    await stores.conversation.attach(1)

    expect(stores.conversation.conversationFor(1).events).toHaveLength(1)
    expect(stores.conversation.conversationFor(1).state).toBe('ready')
    expect(ipc.calls('session_attach')).toEqual([{ id: 1 }])
  })

  it('appends events that arrive in sequence', async () => {
    const { stores, emit, nextTick } = await ready({
      events: [text(1, 'a')],
      seq: 1,
      state: 'running'
    })
    await stores.conversation.attach(1)
    await emit('session:events', { id: 1, events: [text(2, 'b')] })
    await nextTick()

    expect(stores.conversation.conversationFor(1).events.map((e) => e.text)).toEqual(['a', 'b'])
  })

  /* The whole reason `seq` exists. A gap means the worker trimmed away what
     this window never saw, and drawing the tail would be drawing a conversation
     with a silent hole in the middle of it. */
  it('re-attaches rather than drawing a gap when an event arrives out of sequence', async () => {
    const { ipc, stores, emit } = await ready({
      events: [text(9, 'fresh')],
      seq: 9,
      state: 'running'
    })
    await stores.conversation.attach(1)
    await emit('session:events', { id: 1, events: [text(7, 'from the future')] })

    /* The repair starts inside the event listener and so waits for neither the
       emit nor a tick: `vi.waitFor` here is not decoration, it is the only way
       not to race the test itself — the same idiom the terminal store's own gap
       test uses. */
    await vi.waitFor(() => expect(ipc.calls('session_attach')).toHaveLength(2))

    // The fresh snapshot whole, and never it spliced onto what was drawn before.
    expect(stores.conversation.conversationFor(1).events.map((e) => e.text)).toEqual(['fresh'])
  })

  /* Dropped rather than kept: `session:events` is emitted for every session of
     every project, and a window with nowhere to draw one has nothing to gain by
     hoarding it — `session_attach` hands over the whole journal if that session
     is ever opened. */
  it('ignores events for a session this window is not holding', async () => {
    const { stores, emit, nextTick } = await ready({ events: [], seq: 0, state: 'starting' })
    await stores.conversation.attach(1)
    await emit('session:events', { id: 2, events: [text(1, 'somebody else')] })
    await nextTick()

    expect(stores.conversation.conversationFor(1).events).toHaveLength(0)
    // Nor kept under the session it was actually about: asking about 2 for the
    // first time gives an empty conversation, not the event that was dropped.
    expect(stores.conversation.conversationFor(2).events).toHaveLength(0)
  })

  it('holds the unanswered question where a component can find it', async () => {
    const { stores } = await ready({ events: [permission(1)], seq: 1, state: 'needs-you' })
    await stores.conversation.attach(1)

    expect(stores.conversation.conversationFor(1).question).toMatchObject({
      id: 'q1',
      tool: 'Bash',
      options: ['allow', 'deny']
    })
  })

  it('lets the question go once it has been answered', async () => {
    const { stores, emit, nextTick } = await ready({
      events: [permission(1)],
      seq: 1,
      state: 'needs-you'
    })
    await stores.conversation.attach(1)
    await emit('session:events', {
      id: 1,
      events: [event(2, 'permission-answered', { id: 'q1', decision: 'allow' })]
    })
    await nextTick()

    expect(stores.conversation.conversationFor(1).question).toBe(null)
  })

  it('holds only the newest question when two are somehow open', async () => {
    const { stores } = await ready({
      events: [permission(1, 'q1'), permission(2, 'q2')],
      seq: 2,
      state: 'needs-you'
    })
    await stores.conversation.attach(1)

    expect(stores.conversation.conversationFor(1).question.id).toBe('q2')
  })

  it('sends a message with its attachments', async () => {
    const { ipc, stores } = await ready()
    ipc.on('session_send', null)
    await stores.conversation.attach(1)
    await stores.conversation.sendMessage(1, 'hello', ['/tmp/a.png'])

    expect(ipc.calls('session_send')).toEqual([
      { id: 1, text: 'hello', attachments: ['/tmp/a.png'] }
    ])
  })

  it('refuses to send nothing at all', async () => {
    const { ipc, stores } = await ready()
    ipc.on('session_send', null)
    await stores.conversation.attach(1)
    await stores.conversation.sendMessage(1, '   ', [])

    expect(ipc.calls('session_send')).toHaveLength(0)
  })

  /* Optimistic clearing loses a person's words exactly when the worker is down,
     which is the one moment they cannot get them back from anywhere. */
  it('keeps the draft when the send is refused, and says why', async () => {
    const { ipc, stores } = await ready()
    ipc.fail('session_send', new Error('the worker is not running'))
    await stores.conversation.attach(1)
    stores.conversation.conversationFor(1).draft = 'unsent words'
    await stores.conversation.sendMessage(1, 'unsent words', [])

    expect(stores.conversation.conversationFor(1).draft).toBe('unsent words')
    expect(stores.conversation.conversationState.lastError).toBeTruthy()
  })

  it('lets the draft go once the message is away', async () => {
    const { ipc, stores } = await ready()
    ipc.on('session_send', null)
    await stores.conversation.attach(1)
    stores.conversation.conversationFor(1).draft = 'sent words'
    await stores.conversation.sendMessage(1, 'sent words', [])

    expect(stores.conversation.conversationFor(1).draft).toBe('')
  })

  it('carries a failure to answer into lastError rather than swallowing it', async () => {
    const { ipc, stores } = await ready({ events: [permission(1)], seq: 1, state: 'needs-you' })
    /* The worker's own refusal, in the shape `SessionError` serialises to: a
       kind, and the variant's payload as `message`. The question's id is the
       whole of what that payload is, which is why the sentence has to be worded
       around it here rather than repeated from Rust. */
    ipc.fail('session_answer', { kind: 'noSuchQuestion', message: 'q1' })
    await stores.conversation.attach(1)
    await stores.conversation.answerQuestion(1, 'q1', 'allow')

    expect(stores.conversation.conversationState.lastError).toContain('q1')
  })

  it('follows the state the worker reports', async () => {
    const { stores, emit, nextTick } = await ready({ events: [], seq: 0, state: 'starting' })
    await stores.conversation.attach(1)
    await emit('session:state', { id: 1, state: 'needs-you' })
    await nextTick()

    expect(stores.conversation.conversationFor(1).state).toBe('needs-you')
  })
})
