/* A driven session's conversation on the front end: the journal, the question
   it is waiting on, and the words a person has not sent yet. Components know
   only this store; only it knows Tauri exists.

   The shape is `terminals.js`'s — module-level reactive state, an `init` that
   registers the listeners once, and every `invoke` wrapped so a refusal lands
   in `lastError` rather than in an unhandled rejection. What it is not is a
   second copy of that store: a driven session has no PTY, no bytes and no
   screen. The worker holds the journal (`src-tauri/src/session/`), and that is
   the whole reason `session_attach` can be asked as many times as a window
   likes — the second answer is the same conversation as the first.

   Three of this file's rules are load-bearing and each is written where it is
   enforced: an out-of-sequence event takes a fresh snapshot rather than
   stitching a hole, `question` is derived and never stored, and a draft
   survives a send that failed. */
import { computed, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
/* The one thing this store reads out of another, and it is read inside
   `canDrive` alone: whether the person wants the conversation panel at all.
   Nothing happens at import time — `settings.js` reaches Tauri only from its
   own functions — and the cycle this closes (settings → tabs → conversation)
   is the one `settings.js` already records as harmless for that reason. */
import { settings } from './settings.js'

export const conversationState = reactive({
  /* Whether the listeners are up. Nothing branches on it today; it is here for
     the reason `terminalState.ready` is — a panel that draws before the events
     are subscribed to would be drawing a conversation that cannot move. */
  ready: false,
  /* The last refusal, as `{ session, text }`, or null — **one sentence for a
     person and the session it is about**.

     Two readers, and the session is what decides which of them says it. A line
     inside the conversation panel, drawn only by the panel holding *that*
     session; and a toast in `DesktopApp.vue`'s corner, which draws everything
     the panel does not. `session` is `null` for a refusal that belongs to no
     conversation at all — a start that never made one — and the corner is the
     only reader such a sentence can have.

     The pair is `{ session, text }` and not `terminals.js`'s `{ title,
     description }`, because the title is the one part that does not vary: every
     refusal on this road is one thing failing to be reached, so it is a
     constant at the toast's own call site rather than a field every `report`
     below would have to invent a value for. The panel needs no title at all —
     it is drawn against the very thing the sentence is about — and what it does
     need is exactly what the title cannot give it: whether this sentence is
     about the session on screen. Without that, a spawn refusal for a session
     that never existed painted itself at the foot of a healthy conversation and
     stayed there, since that line has no dismiss and clears only on the next
     call that answers. */
  lastError: null
})

/* Session id → the conversation this window is holding for it. Only sessions
   this window has actually asked about are in here, and that is what makes the
   drop below safe: `session:events` is emitted for every session of every
   project, attached or not. */
const conversations = reactive(new Map())

/* The unsent words, kept beside the conversations rather than inside them, so
   that they outlive `detach`. A journal can be taken again from the worker
   whenever it is wanted; a sentence somebody was halfway through typing exists
   nowhere else, and losing it on a tab switch is the same loss the failed-send
   rule below refuses. Keyed by session, so two sessions never share one.

   Nothing empties it. A window's sessions are counted in tens at the very most,
   and a draft is a short string; forgetting one at the moment its session ends
   would be throwing away the one copy of it just as somebody goes looking. */
const drafts = reactive(new Map())

/* The question this conversation is waiting on: the newest `permission` with no
   `permission-answered` carrying the same id after it, or null.

   **Derived and never stored.** This is the loudest thing the panel draws, and
   a stored copy would be a second source of truth for it — one that a snapshot
   replacing the journal, or an answer arriving from another window, would leave
   standing over a question the agent has already stopped asking.

   By id and never by count, which is `session::model::is_open_question`'s rule
   on the Rust side and has to be the same one here: an answer that arrives out
   of order settles its own question and leaves the others standing. The Map
   keeps the journal's order, so the last value still in it is the newest one
   open — two at once is not a shape the worker produces, and taking the newest
   is the reading that cannot leave the panel pointing at a stale one. */
function openQuestion(events) {
  const open = new Map()
  for (const event of events) {
    if (event.kind === 'permission') open.set(event.id, event)
    else if (event.kind === 'permission-answered') open.delete(event.id)
  }
  let newest = null
  for (const question of open.values()) newest = question
  return newest
}

/* The record for a session, made if this window has none yet.

   Asking about a session is what makes this window hold it, which is the same
   line the events listener draws: a record here means there is somewhere to
   draw an event into. A component may therefore bind to a session before its
   snapshot has landed and get an empty conversation rather than a null.

   `starting` is the state until the worker says otherwise, matching
   `session::model::state_of` over an empty journal — a session that has
   produced nothing yet has not necessarily failed to. */
function hold(id) {
  const held = conversations.get(id)
  if (held) return held
  const fresh = reactive({
    id,
    events: [],
    /* The number the next event must be exactly one past. `0` before any
       snapshot, which is what the worker's own counter starts at. */
    seq: 0,
    state: 'starting',
    /* Derived, and derived once per change rather than once per read. A getter
       here read the same and refolded the whole journal every time a render
       touched it — against `journal::BUDGET`, four thousand events, while
       `events` is being pushed to on every batch. `reactive` unwraps a ref held
       as a property, so `held.question` is the same read it always was. */
    question: computed(() => openQuestion(fresh.events)),
    get draft() {
      return drafts.get(id) ?? ''
    },
    set draft(text) {
      drafts.set(id, text)
    }
  })
  conversations.set(id, fresh)
  return fresh
}

/* `session::model::SessionState` in the design system's status vocabulary.

   The translation lives in a store for the reason the terminal's own lives in
   `terminals.js` — `toUiState` there is the shape this follows rather than a
   second reading of the same question — and it is deliberately not a copy of
   that function: the two vocabularies differ, since a driven session has no
   exit code to read and `state_of` has already decided which of the two endings
   this was.

   `starting` is a session that has not spoken yet, which is a live thing and so
   `running`; `exited` is a turn that was closed when the child went, which is
   an ordinary end and so `done`. `ready`, `running`, `needs-you` and `failed`
   are already this system's words and pass through. So does a word this front
   end has never heard of, which `status/status.js` answers with a generated
   colour and a two-letter code — the honest outcome for a state added to Rust
   and not yet to this list. */
export function statusOf(state) {
  if (state === 'starting') return 'running'
  if (state === 'exited') return 'done'
  return state
}

/* What a component draws: `{ events, state, question, draft }`, reactive, with
   `draft` writable. Never null — see `hold`.

   **Called from `setup`, not from a template.** It makes the record if this
   window has none, and creating state as a side effect of a read is a write
   during a render: a component that called it in its own template would cost a
   wasted render every time the session changed under it. Take the record once
   and hold it. */
export function conversationFor(id) {
  return hold(id)
}

/* The worker's errors are diagnostics and their text is written for whoever
   fixes things; the two that a person can act on get words of their own, keyed
   on the `kind` the Rust error itself carries (`SessionError`'s serde tag).
   `message` is the variant's payload rather than its `Display` — a question id,
   a session id, a sentence — which is what lets these read as sentences at all.

   The same split `terminals.js` makes, and the same `typeof` guard, for its
   reason: a future Rust kind called `spawn` must not resolve to something that
   is not a function and be called with an id. */
const ERRORS = {
  spawn: (text) => text,
  noSuchSession: (id) => `Session ${id} is not running any more.`,
  noSuchQuestion: (id) =>
    `That question is not waiting for an answer any more (${id}) — it was answered already, or the agent stopped asking.`
}

function sentence(error) {
  const known = ERRORS[error?.kind]
  if (typeof known === 'function') return known(error.message)
  /* Anything else: a plain `Error` from the transport, or a refusal this store
     has no words for. Its own message beats a generic line, since a refusal
     nobody has written copy for is exactly where the raw text is worth having. */
  const message = error?.message
  return typeof message === 'string' && message ? message : String(error)
}

/* `session` is the conversation the refusal belongs to, and `null` when it
   belongs to none — a start that never made one. It is the first argument
   because it is the thing a caller cannot leave out by accident: every call
   below has one to hand, and the only `null` is written as `null`. */
function report(session, what, error) {
  console.error(`[conversation] ${what} failed:`, error)
  conversationState.lastError = { session, text: sentence(error) }
}

/* Append a batch, or say that it cannot be appended.

   **An event whose `seq` is not exactly one past the last one seen is a gap,
   and a gap is never patched.** It means the worker trimmed away something this
   window never saw, so the tail on its own would be a conversation with a
   silent hole in the middle of it — and the snapshot that replaces it is free,
   the journal living in Rust. A batch that goes wrong halfway is left half
   appended on purpose: the fresh snapshot replaces the journal whole, so
   nothing here has to be undone. */
function absorb(held, events) {
  for (const event of events) {
    if (event.seq !== held.seq + 1) return false
    held.events.push(event)
    held.seq = event.seq
  }
  return true
}

/* Which snapshot is the live one, per session. Two attaches can be in flight at
   once — a component remounting while a gap repair is already asking — and they
   answer in no fixed order, so the older reply must drop its result rather than
   write a journal the newer one has already replaced. The same guard
   `terminals.js` puts on its own attach, and the same reason. */
const attaching = new Map()

/* Take the whole conversation from the worker. Asked whenever a window opens on
   a session, however many times that is: the journal is the worker's, so a
   second attach hands back exactly what the first did.

   The listeners are ensured here rather than left to a caller: a snapshot taken
   before anything is subscribed would be a conversation that never moves again,
   and there is nothing a component could usefully do about that but remember to
   call the init. */
export async function attach(id) {
  const held = hold(id)
  /* Its own try, in front of the snapshot's rather than around it. `listen` is
     an `invoke` like anything else and can be refused, and this function is the
     one a component calls from `onMounted` with nothing to catch it: left
     outside, that refusal left the panel wedged with one console line to show
     for it and nothing in `lastError`. There is no snapshot to ask for if
     nothing is listening for what comes after it, so this returns rather than
     going on. */
  try {
    await initConversation()
  } catch (err) {
    report(id, 'subscribing to the session events', err)
    return
  }
  const current = invoke('session_attach', { id })
  attaching.set(id, current)
  try {
    const { events, seq, state } = await current
    if (attaching.get(id) !== current) return
    /* Replaced whole and never merged: this *is* the conversation, and the one
       thing a snapshot is for is being trusted over whatever was drawn before
       it. */
    held.events = events ?? []
    held.seq = seq ?? 0
    held.state = state ?? 'starting'
    conversationState.lastError = null
  } catch (err) {
    // A newer attach has already overtaken this one; its outcome is what the
    // panel should reflect, not this rejection.
    if (attaching.get(id) !== current) return
    report(id, 'attaching to a session', err)
  }
}

/* This window stops holding a session: the journal goes, the draft stays.

   There is no `session_detach` to make, and that is a fact about the subsystem
   rather than an omission — the worker emits `session:events` for every session
   whether anybody is looking or not, so detaching is entirely this side's
   bookkeeping. What it buys is the drop in the listeners below: a background
   session's journal would otherwise go on growing in a window that draws none
   of it, beside the copy the worker already keeps. */
export function detach(id) {
  conversations.delete(id)
  attaching.delete(id)
}

/* Registered once for the window. A promise rather than a flag, so that two
   callers racing to attach subscribe once between them rather than twice. */
let registering = null

export function initConversation() {
  /* A subscription that failed is deliberately **not** what gets kept. The
     promise is held for the life of the window, so caching a rejection would
     make one refusal permanent: every later attach would reject on this same
     settled promise without anything being tried again, and a window would need
     reloading to recover from a hiccup it could have retried out of. */
  registering ??= register().catch((err) => {
    registering = null
    throw err
  })
  return registering
}

async function register() {
  /* The handles are held here, where `terminals.js` discards its own, and the
     retry above is the whole of the difference rather than a change of taste.
     These two subscriptions are awaited in sequence, so a refusal of the second
     leaves the first one live — and a retry that started from there would
     subscribe `session:events` a second time on top of it. Every batch would
     then be absorbed twice; the second pass would find its `seq` already taken,
     read that as a gap and fire a full `session_attach` — up to
     `journal::BUDGET` events over the wire per batch, for the life of the
     window, with the panel drawing correctly the whole time and nothing on
     screen to point at it. A retry has to start from nothing subscribed, so
     half a subscription is undone before the refusal is passed on. */
  const made = []
  try {
    made.push(await listenToEvents())
    made.push(await listenToState())
  } catch (err) {
    for (const dispose of made) {
      /* Unsubscribing is itself an `invoke`, and can be refused in exactly the
         state that brought us here. There is nothing to do about that but say
         so: a rejection let out would replace the refusal being reported, which
         is the one a person can act on. */
      try {
        await dispose()
      } catch (undoing) {
        console.error('[conversation] dropping a half-made subscription failed:', undoing)
      }
    }
    throw err
  }
  conversationState.ready = true
}

function listenToEvents() {
  return listen('session:events', (event) => {
    const { id, events } = event.payload
    /* **Dropped, not buffered.** A session this window is not holding has
       nowhere for these to be drawn, and if it ever is held, `session_attach`
       hands over the whole journal including everything dropped here. Keeping
       them would be a second store of conversations nobody is reading. */
    const held = conversations.get(id)
    if (!held) return
    if (!absorb(held, events)) {
      /* Fired from a listener and awaited by nobody: `attach` reports rather
         than throws, and the `.catch` stays as a second line of defence — a
         gap must never surface as an unhandled rejection, whatever else about
         that function changes. */
      attach(id).catch(() => {})
    }
  })
}

function listenToState() {
  return listen('session:state', (event) => {
    const { id, state } = event.payload
    const held = conversations.get(id)
    if (!held) return
    held.state = state
  })
}

/* Which harnesses this app can drive, and the whole of the list.

   A driven session is one whose protocol the worker parses itself, and only
   Claude Code has a driver: `session::service::driver_for` refuses every other
   profile, and `Request::Start` refuses every intent but `Bare`. So the front
   end asks before it takes this road at all — a person whose harness is Codex
   pressing "+ New agent" must get the PTY they have always had.

   **This is a cheap front door and cannot be the only gate, because it cannot
   see `PATH`.** It is asked of `settings.agent`, and the first half of that
   chain is exact: `Intent::Bare` takes `agents::Role::Default`, which
   `settings::model::Settings::role_pair` answers with the root pair — the same
   two fields the front end holds, with no per-project override reaching it. The
   half it cannot see is downstream of all of that. `agents::pick` substitutes
   **the first installed profile** when the configured one is not on the machine,
   silently and by design, and `pick_with_model` is what `spawn_session` calls.
   `settings.agent` ships as `claude` and `Settings::validate` forces anything
   unknown back to it, so a machine with only Codex on it answers `true` here and
   is refused by the driver a round trip later. What answers that is the caller:
   `newAgent` in `views/DesktopApp.vue` falls through to `createSession` when a
   driven start comes back with nothing, and `createSession` resolves whatever
   `pick` would have. Nothing here should grow a second guess at `PATH` instead —
   the front end does not have one.

   A list here rather than a capability on the harness row, because there is no
   flag for this: `agents::Capabilities` carries `resume`, `fork`, `clear`,
   `usage`, `batch` and `oneshot`, and none of them means "has a driver". The
   day a second harness grows one, this list and `driver_for` are the two places
   that have to agree, which is why this one names the other.

   **The person's own switch is inside this answer rather than beside it.**
   `settings.conversationPanel` off makes every harness answer `false` here, so
   every road into a session takes the PTY without a second condition anywhere
   — a `if (!settings.conversationPanel)` in `newAgent` and a third in whatever
   resumes one would be two copies of one rule, and copies drift apart. It is
   in front of the list rather than in it: the list is what Rust can drive and
   is not the person's to edit, and `session::service::driver_for` is untouched
   by this switch. The front end simply stops asking.

   Read at the moment it is asked and never cached, which is the whole of
   "changes what starts, not what runs": a panel already on screen goes on being
   a panel, and the next session opens in a terminal. */
const DRIVEN = ['claude']

export const canDrive = (agent) => settings.conversationPanel && DRIVEN.includes(agent)

/* Which driven sessions this window has started, and in which project.

   Kept beside the conversations rather than read out of them, because the two
   answer different questions. `conversations` above is what this window is
   drawing *right now*, and `detach` empties it the moment a panel goes away;
   this says which sessions a project has at all, which is what the centre's
   Agent tab is derived from (`hasAgentTab` in `stores/tabs.js`). Derived from
   the other, that tab would disappear the moment somebody looked at the board
   and take the way back to their agent with it.

   **Nothing takes an entry out, and that is the terminal's behaviour rather
   than an omission.** A session whose child has gone keeps its row and its tab
   there until somebody closes it, on the grounds that the last words of
   whatever was running are worth reading; here there is not even a process to
   close, only a journal the worker still holds. A restart empties this, driven
   sessions deliberately not surviving one — the same repair `restoreTabs`
   already makes for a remembered `activeTab: "terminal"`.

   The project is the path `startConversation` was given, which is the same
   string the terminal store keys its own sessions by: the project's own
   folder. */
const started = reactive([])

/* The driven sessions of one project, oldest first. An array of ids rather than
   of records, because that is the whole of what a caller wants — the tab is
   derived from whether there are any, and the panel from which one is picked. */
export const conversationsIn = (project) =>
  started.filter((session) => session.project === project).map((session) => session.id)

/* Start a driven session and hold it. The id is the answer; `null` means it did
   not start, and the sentence saying why is in `lastError`.

   The intent is the same vocabulary `terminal_create` takes — which words reach
   the agent is Rust's business (`src-tauri/src/agents/`), and this store's is
   only to say what the session is for. The attach that follows is what makes
   the caller's next `conversationFor` draw anything: a session started and not
   attached to would sit in the worker filling a journal nothing reads. */
export async function startConversation(project, intent = { kind: 'bare' }) {
  try {
    const id = await invoke('session_start', { project, intent })
    conversationState.lastError = null
    /* Before the attach rather than after it: the Agent tab is derived from
       this list, and a session held only once its snapshot had come back would
       leave the button somebody pressed with no visible effect for the length
       of a spawn — the same reason `terminalState.starting` exists one
       subsystem over. */
    started.push({ id, project })
    await attach(id)
    return id
  } catch (err) {
    report(null, 'starting a session', err)
    return null
  }
}

/* A person's turn.

   **The draft survives a send that failed.** Clearing it the moment the button
   is pressed loses somebody's words whenever the worker is down, and those
   words exist nowhere else — so it is cleared on the way out of a call that
   answered, and never before. The person's own text goes over the wire
   unchanged; what counts as an empty message is the only thing decided here.

   An empty message is not sent at all. Blank text with an attachment on it is
   an ordinary message and does go: a picture is a thing to say. */
export async function sendMessage(id, text, attachments = []) {
  if (!String(text ?? '').trim() && attachments.length === 0) return
  try {
    await invoke('session_send', { id, text, attachments })
    conversationState.lastError = null
    /* Cleared only while it is still the words that went. A slow worker invites
       somebody to go on typing during the round trip, and an unconditional
       clear here would wipe those keystrokes with the reply to the message
       before them — the same loss the failed-send rule above refuses, arriving
       by the other road. */
    if (drafts.get(id) === text) drafts.set(id, '')
  } catch (err) {
    report(id, 'sending a message', err)
  }
}

/* A person's answer to a permission question. `question` is the id off the
   `permission` event, which is what the worker looks the waiting tool call up
   by — the journal on the Rust side is the only thing that knows which session
   was asked, and it checks. */
export async function answerQuestion(id, question, decision) {
  try {
    await invoke('session_answer', { id, question, decision })
    conversationState.lastError = null
  } catch (err) {
    report(id, 'answering a question', err)
  }
}

/* Stop the turn in flight. What that costs the harness is the driver's business
   — for Claude Code it is the child, which has no documented way of being asked
   to stop one turn — and nothing here pretends otherwise. */
export async function stopConversation(id) {
  try {
    await invoke('session_stop', { id })
    conversationState.lastError = null
  } catch (err) {
    report(id, 'stopping a session', err)
  }
}
