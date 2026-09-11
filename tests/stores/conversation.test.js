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

    /* The sentence, and the session it is about: the panel draws a refusal only
       for the conversation it is holding, so a refusal that lost its session
       would be drawn by nobody or by the wrong panel. */
    expect(stores.conversation.conversationState.lastError).toMatchObject({ session: 1 })
    expect(stores.conversation.conversationState.lastError.text).toContain('q1')
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
    expect(stores.conversation.conversationState.lastError.text).toContain(
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

  /* Which driven sessions this window is holding, and for which project. The
     centre's Agent tab is derived from this list (`hasAgentTab` in
     `stores/tabs.js`), which is what makes each of the three below a rule about
     the app rather than about a getter. */
  describe('the sessions this window holds', () => {
    it('holds a started session against the project it was started in', async () => {
      const { ipc, stores } = await ready()
      ipc.on('session_start', 7)

      expect(await stores.conversation.startConversation('/p')).toBe(7)
      expect(stores.conversation.conversationsIn('/p')).toEqual([7])
      expect(stores.conversation.conversationsIn('/elsewhere')).toEqual([])
    })

    /* A start that was refused is not a session. The tab would otherwise appear
       for a conversation that does not exist and stay for the life of the
       window — nothing takes an entry out of this list. */
    it('holds nothing when the start was refused', async () => {
      const { ipc, stores } = await ready()
      ipc.fail('session_start', new Error('claude could not be started'))

      expect(await stores.conversation.startConversation('/p')).toBe(null)
      expect(stores.conversation.conversationsIn('/p')).toEqual([])
    })

    /* **The list outlives `detach`.** The two answer different questions: the
       conversations are what this window is drawing right now, and this is what
       the project has at all. Derived from the other, the Agent tab would go
       away the moment somebody looked at the board — taking with it the only
       way back to their agent. */
    it('goes on holding a session whose panel has been closed', async () => {
      const { ipc, stores } = await ready()
      ipc.on('session_start', 7)
      await stores.conversation.startConversation('/p')
      stores.conversation.detach(7)

      expect(stores.conversation.conversationsIn('/p')).toEqual([7])
    })

    /* What the row in the agents panel is drawn from, and the reason the record
       carries more than an id: the panel says what a session is doing and how
       long it has been at it, and `conversations` cannot answer either once the
       panel has left the screen. */
    it('carries the state and the moment the session started', async () => {
      const { ipc, stores } = await ready({ events: [], seq: 0, state: 'running' })
      ipc.on('session_start', 7)
      const before = Date.now()
      await stores.conversation.startConversation('/p')

      const [record] = stores.conversation.drivenSessions.value
      expect(record).toMatchObject({ id: 7, project: '/p', state: 'running' })
      expect(record.startedAt).toBeGreaterThanOrEqual(before)
      expect(record.startedAt).toBeLessThanOrEqual(Date.now())
    })

    /* The name the row is keyed by, and the one thing about a driven session
       that will still mean something after a restart. `session_start` answers
       with the worker's own counter, so the id the conversation is *recorded*
       under has to arrive by another road — and the snapshot is the earlier of
       the two, `session:state` going out on a change and a session that has
       just started not having changed yet. */
    it('takes the conversation id off the snapshot it attaches with', async () => {
      const { ipc, stores } = await ready({
        events: [],
        seq: 0,
        state: 'starting',
        conversation: '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60'
      })
      ipc.on('session_start', 7)
      await stores.conversation.startConversation('/p')

      expect(stores.conversation.drivenSessions.value[0].conversation).toBe(
        '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60'
      )
    })

    /* And the other road, which is the one a window that missed the snapshot
       has: the id travels on every state change rather than once. */
    it('takes it off a state change as well', async () => {
      const { ipc, stores, emit, nextTick } = await ready()
      ipc.on('session_start', 7)
      await stores.conversation.startConversation('/p')
      expect(stores.conversation.drivenSessions.value[0].conversation).toBe(null)

      await emit('session:state', {
        id: 7,
        state: 'running',
        conversation: '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60'
      })
      await nextTick()

      expect(stores.conversation.drivenSessions.value[0].conversation).toBe(
        '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60'
      )
    })

    /* A fork is recorded nowhere and says so with a `null`; so does the first
       frame of any session. What must never happen is the other direction — a
       payload that arrived without the field taking a name off a row that has
       one, since that is a build that stopped sending it rather than a session
       that has lost its id. */
    it('never writes a name back off a row that has one', async () => {
      const { ipc, stores, emit, nextTick } = await ready({
        events: [],
        seq: 0,
        state: 'starting',
        conversation: '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60'
      })
      ipc.on('session_start', 7)
      await stores.conversation.startConversation('/p')
      await emit('session:state', { id: 7, state: 'running' })
      await nextTick()

      expect(stores.conversation.drivenSessions.value[0].conversation).toBe(
        '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60'
      )
    })

    /* What the row is captioned by, which is this store's half of
       `Intent::work()` in Rust: which of an intent's payload is drawn, and
       which of it was only a briefing for the agent. */
    it('reduces the intent to what the row says about it', async () => {
      const { ipc, stores } = await ready()
      ipc.on('session_start', 7)
      await stores.conversation.startConversation('/p', {
        kind: 'resumeSession',
        id: '9f1c',
        cwd: '/p/.worktrees/task',
        title: 'Move the card to done',
        fork: false
      })

      expect(stores.conversation.drivenSessions.value[0].work).toEqual({
        kind: 'resumeSession',
        title: 'Move the card to done'
      })
    })

    /* **The row keeps reporting after the panel has gone.** `detach` empties
       the journal, so a listener that wrote only into it would leave the row
       frozen at whatever it said when somebody last looked at the conversation
       — which is every moment the centre tab is on the board. */
    it('follows the state of a session whose journal it no longer holds', async () => {
      const { ipc, stores, emit, nextTick } = await ready()
      ipc.on('session_start', 7)
      await stores.conversation.startConversation('/p')
      stores.conversation.detach(7)
      await emit('session:state', { id: 7, state: 'needs-you' })
      await nextTick()

      expect(stores.conversation.drivenSessions.value[0].state).toBe('needs-you')
    })

    /* A session this window never started has no record to write into, and a
       state event arrives for every session of every project. */
    it('writes no record for a session it never started', async () => {
      const { ipc, stores, emit, nextTick } = await ready()
      ipc.on('session_start', 7)
      await stores.conversation.startConversation('/p')
      await emit('session:state', { id: 9, state: 'needs-you' })
      await nextTick()

      expect(stores.conversation.drivenSessions.value).toHaveLength(1)
      expect(stores.conversation.drivenSessions.value[0].state).not.toBe('needs-you')
    })

    /* The cross on the row, which is the one thing that takes an entry out of
       this list. The journal goes with it: nothing is left that could ask for
       the conversation again. */
    it('lets a session go when the row is closed', async () => {
      const { ipc, stores } = await ready()
      ipc.on('session_start', 7)
      await stores.conversation.startConversation('/p')
      stores.conversation.forget(7)

      expect(stores.conversation.conversationsIn('/p')).toEqual([])
      expect(stores.conversation.drivenSessions.value).toEqual([])
    })

    it('forgets a session it never held without complaining', async () => {
      const { ipc, stores } = await ready()
      ipc.on('session_start', 7)
      await stores.conversation.startConversation('/p')
      stores.conversation.forget(9)

      expect(stores.conversation.conversationsIn('/p')).toEqual([7])
    })
  })

  /* The one refusal that belongs to no conversation, which is what `session:
     null` is for: there is no panel to draw it, so the toast in the corner is
     the only reader it can have. */
  it('reports a start that never happened against no session at all', async () => {
    const { ipc, stores } = await ready()
    ipc.fail('session_start', new Error('claude could not be started'))
    await stores.conversation.startConversation('/p')

    expect(stores.conversation.conversationState.lastError).toEqual({
      session: null,
      text: 'claude could not be started'
    })
  })

  /* Which harnesses this app can drive. The list decides which road
     `newAgent` takes, so a wrong answer here is either a Claude session that
     never becomes a conversation or a Codex session that cannot start at all. */
  describe('the harnesses that can be driven', () => {
    it('drives Claude Code and nothing else this build ships', async () => {
      const { stores } = await ready()

      expect(stores.conversation.canDrive('claude')).toBe(true)
      expect(stores.conversation.canDrive('codex')).toBe(false)
    })

    /* A hand-edited `settings.json`, or a harness added to Rust and not to this
       list: the PTY road is the one that still works for it. */
    it('does not drive a harness it has never heard of', async () => {
      const { stores } = await ready()

      expect(stores.conversation.canDrive('')).toBe(false)
      expect(stores.conversation.canDrive('gemini')).toBe(false)
    })

    /* The person's own switch, and the reason it lives inside this answer
       rather than beside it: every road into a session asks this one question,
       so `views/DesktopApp.vue` needs no condition of its own and there is no
       second copy of the rule to drift away from this one.

       The setting is written straight into the store rather than loaded off a
       fixture on purpose: `loadSettings` is what installs the debounced write,
       and a `settings_save` draining out of the afterEach is exactly the
       cross-test noise this file's own transport note warns about. Nothing
       here needs the disk — `canDrive` reads the live object. */
    it('drives nothing at all while the conversation panel is switched off', async () => {
      const { stores } = await ready()
      stores.settings.settings.conversationPanel = false

      expect(stores.conversation.canDrive('claude')).toBe(false)
      expect(stores.conversation.canDrive('codex')).toBe(false)

      /* Switched back on, the next session is a conversation again: the field
         is read at the moment it is asked, which is what makes this a rule
         about what starts rather than about what is already running. */
      stores.settings.settings.conversationPanel = true

      expect(stores.conversation.canDrive('claude')).toBe(true)
    })
  })

  /* `session::model::SessionState` in the design system's words. The two that
     are translated are the whole of the rule; the rest are already this
     system's and pass through, a word from a Rust that has moved on ahead of
     this list included. */
  describe('the state in the status vocabulary', () => {
    it('draws a session that has not spoken yet as live', async () => {
      const { stores } = await ready()

      expect(stores.conversation.statusOf('starting')).toBe('running')
    })

    it('draws an ordinary end as done', async () => {
      const { stores } = await ready()

      expect(stores.conversation.statusOf('exited')).toBe('done')
    })

    it('passes through the words this system already has', async () => {
      const { stores } = await ready()
      const { statusOf } = stores.conversation

      expect(['ready', 'running', 'needs-you', 'failed'].map((state) => statusOf(state))).toEqual([
        'ready',
        'running',
        'needs-you',
        'failed'
      ])
      expect(statusOf('hibernating')).toBe('hibernating')
    })
  })
})
