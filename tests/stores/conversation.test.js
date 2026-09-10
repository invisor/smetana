import { describe, expect, it, vi } from 'vitest'
import { mockIPC } from '@tauri-apps/api/mocks'
import { loadStores } from '../support/stores.js'
import { installIpc } from '../support/ipc.js'

/* One journal event as the worker writes it: `seq`, `at`, and the kind's own
   fields flattened in beside a kebab-case `kind` — the serde shape of
   `session::model::Event`, which is the contract rather than a convenience. */
const event = (seq, kind, over = {}) => ({ seq, at: '2026-09-10T12:00:00Z', kind, ...over })

const text = (seq, body) => event(seq, 'text', { text: body })

const permission = (seq, id = 'q1') =>
  event(seq, 'permission', { id, tool: 'Bash', detail: 'rm -rf /tmp/x', options: ['allow', 'deny'] })

/* A graph with the one read every test here needs already answered. The
   snapshot is the argument, since what `session_attach` hands back is the whole
   of what a conversation starts as.

   Copied per answer, and that is hygiene rather than a rule being pinned.
   Registered as a value, the router hands back one array instance to every
   attach, and the store assigns it straight to `held.events` and pushes into
   it — so a test appending an event was quietly editing the fixture the next
   attach would answer with, and a snapshot in this file has to mean what a
   worker's reply means: a fresh object every time. No assertion below rests on
   the copying; what it removes is a test able to change another one's
   arrangement out from under it. */
async function ready(snapshot = { events: [], seq: 0, state: 'ready' }) {
  const loaded = await loadStores()
  loaded.ipc.on('session_attach', () => ({ ...snapshot, events: [...snapshot.events] }))
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

    /* The two halves of what this pins, and neither is more than it says. The
       `waitFor` above is the discriminating one: the repair happened at all.
       This is the end state the rule promises — the journal afterwards is the
       snapshot the worker handed back, with nothing of the gap in it.

       It is deliberately not an assertion about a splice, which would be a
       different test: `absorb` appends a batch's good prefix before it meets
       the event that is out of sequence, by design and said so where it does
       it, and the snapshot replacing the journal whole is what makes that
       transient harmless. */
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

  /* The other half of the same rule, and the road it arrives by is the one a
     slow worker opens: the reply to the message that went must not carry off
     the words typed while it was travelling. */
  it('keeps words typed while the message was in flight', async () => {
    const { ipc, stores } = await ready()
    let release
    ipc.on('session_send', () => new Promise((resolve) => (release = resolve)))
    await stores.conversation.attach(1)

    const held = stores.conversation.conversationFor(1)
    held.draft = 'sent words'
    const sending = stores.conversation.sendMessage(1, 'sent words', [])
    held.draft = 'sent words, and more of them'
    release(null)
    await sending

    expect(held.draft).toBe('sent words, and more of them')
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

  /* Subscribing is an `invoke` too — `plugin:event|listen` — so it can be
     refused, and `attach` is the function a component calls from `onMounted`
     with nothing to catch it. The refusal has to reach `lastError` rather than
     an unhandled rejection, and it must not be remembered: the subscription is
     held for the life of the window, so a cached rejection would wedge the
     panel until somebody reloaded it.

     Installed here rather than through `ipc.on`, and it is the one test that
     does: the router never sees an event-plugin command, `mockIPC` handling
     those itself. What is mocked is still only the transport, and still through
     the official function. */
  it('reports a refused subscription, and lets the next attach try again', async () => {
    const { stores } = await ready()
    mockIPC(() => {
      throw new Error('the event plugin is not there')
    })
    await stores.conversation.attach(1)

    expect(stores.conversation.conversationState.lastError).toBeTruthy()
    expect(stores.conversation.conversationFor(1).events).toHaveLength(0)

    const ipc = installIpc()
    /* What `settleStores` would have registered had it known about this router:
       it never went through `loadStores`, so no graph the harness holds names
       it, and a debounced `settings_save` draining in the afterEach goes
       wherever the transport points — here. Nothing in this file touches a
       setting, so this is for whoever adds the line that does, and gets the
       unregistered-command error out of a hook rather than out of their
       test. */
    ipc.on('settings_save', null)
    ipc.on('session_attach', () => ({ events: [text(1, 'hello')], seq: 1, state: 'ready' }))
    await stores.conversation.attach(1)

    expect(stores.conversation.conversationFor(1).events).toHaveLength(1)
  })

  /* The failure the retry above made reachable. These two subscriptions are
     awaited one after the other, so the second being refused leaves the first
     live — and a retry starting from there would hold two of them, absorb every
     batch twice and read its own second pass as a gap, asking for a whole
     journal per batch for the life of the window with the panel looking
     perfectly right throughout. */
  it('undoes half a subscription rather than retrying on top of it', async () => {
    const { stores, emit, nextTick } = await ready()
    const dropped = []
    mockIPC((cmd, args) => {
      if (cmd === 'plugin:event|listen') {
        if (args.event === 'session:state') throw new Error('the second subscription is refused')
        return args.handler
      }
      if (cmd === 'plugin:event|unlisten') {
        dropped.push(args.event)
        return null
      }
      throw new Error(`[test] command ${cmd} is not registered`)
    })
    await stores.conversation.attach(1)

    /* Which refusal was reported, and not merely that one was. `sentence` falls
       through to `error.message` for a plain `Error`, so the words above land
       here — and a `toBeTruthy` would be satisfied by the `session_attach` that
       follows if `register` ever stopped rethrowing, which is the other road to
       a filled `lastError` and a different fault entirely. */
    expect(stores.conversation.conversationState.lastError).toContain(
      'the second subscription is refused'
    )
    // The half that did go up came back down, so a retry starts from nothing.
    expect(dropped).toEqual(['session:events'])

    const ipc = installIpc()
    ipc.on('settings_save', null)
    ipc.on('session_attach', () => ({ events: [], seq: 0, state: 'ready' }))
    await stores.conversation.attach(1)
    await emit('session:events', { id: 1, events: [text(1, 'a')] })
    await nextTick()

    /* **`expect(dropped).toEqual(['session:events'])` above is what pins the
       rule.** The pair below is the production consequence written down, and it
       cannot fail here: `installIpc` re-installs `mockIPC`, whose
       `listeners`/`callbacks` maps are its own, so a handler registered through
       the transport it replaced is orphaned and can never be delivered to. Both
       of these hold identically with the disposal removed — in the app, where
       one transport lives for the window, they are exactly what a second live
       subscription would break. Do not read `dropped` as arrangement and this
       as the assertion: delete it and the rule is unpinned with the test still
       green. */
    expect(stores.conversation.conversationFor(1).events.map((e) => e.text)).toEqual(['a'])
    expect(ipc.calls('session_attach')).toHaveLength(1)
  })

  it('follows the state the worker reports', async () => {
    const { stores, emit, nextTick } = await ready({ events: [], seq: 0, state: 'starting' })
    await stores.conversation.attach(1)
    await emit('session:state', { id: 1, state: 'needs-you' })
    await nextTick()

    expect(stores.conversation.conversationFor(1).state).toBe('needs-you')
  })
})
