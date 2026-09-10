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
import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export const conversationState = reactive({
  /* Whether the listeners are up. Nothing branches on it today; it is here for
     the reason `terminalState.ready` is — a panel that draws before the events
     are subscribed to would be drawing a conversation that cannot move. */
  ready: false,
  /* One sentence for a person, or null. A string and not the `{ title,
     description }` pair `terminals.js` keeps, because what draws it is a line
     inside the panel rather than the toast corner, and the worker's own words
     are the whole of what there is to say about a question that has gone. */
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
    get question() {
      return openQuestion(this.events)
    },
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

/* What a component draws: `{ events, state, question, draft }`, reactive, with
   `draft` writable. Never null — see `hold`. */
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

function report(what, error) {
  console.error(`[conversation] ${what} failed:`, error)
  conversationState.lastError = sentence(error)
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
  await initConversation()
  const held = hold(id)
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
    held.state = state
    conversationState.lastError = null
  } catch (err) {
    // A newer attach has already overtaken this one; its outcome is what the
    // panel should reflect, not this rejection.
    if (attaching.get(id) !== current) return
    report('attaching to a session', err)
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
  registering ??= register()
  return registering
}

async function register() {
  await listen('session:events', (event) => {
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
  await listen('session:state', (event) => {
    const { id, state } = event.payload
    const held = conversations.get(id)
    if (!held) return
    held.state = state
  })
  conversationState.ready = true
}

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
    await attach(id)
    return id
  } catch (err) {
    report('starting a session', err)
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
    drafts.set(id, '')
  } catch (err) {
    report('sending a message', err)
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
    report('answering a question', err)
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
    report('stopping a session', err)
  }
}
