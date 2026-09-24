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
/* The intent-to-work translation, shared with `stores/terminals.js` and
   `components/agent/drivenRows.js` — see `components/agent/sessionWork.js`'s
   own header for why it lives outside every store. */
import { workOf } from '../components/agent/sessionWork.js'
import { crewRows } from '../components/agent/crewTree.js'
/* The one thing this store reads out of another, and it is read inside
   `canDrive` alone: whether the person wants the conversation panel at all.
   Nothing happens at import time — `settings.js` reaches Tauri only from its
   own functions — and the cycle this closes (settings → tabs → conversation)
   is the one `settings.js` already records as harmless for that reason. */
import { settings } from './settings.js'
/* The one sound `listenToState` below rings, the same call `terminal:state`
   makes in `terminals.js` — see that file and `.claude/rules/notifications.md`
   for the sound itself and for `onlyWhenUnfocused`, neither of which is this
   file's to decide. */
import { chime } from '../chime.js'

export const conversationState = reactive({
  /* Whether the listeners are up. Nothing branches on it today; it is here for
     the reason `terminalState.ready` is — a panel that draws before the events
     are subscribed to would be drawing a conversation that cannot move. */
  ready: false,
  /* The last refusal, as `{ session, kind, text }`, or null — **one sentence
     for a person, the session it is about, and the worker's own tag for it**.

     Two readers, and the session is what decides which of them says it. A line
     inside the conversation panel, drawn only by the panel holding *that*
     session; and a toast in `DesktopApp.vue`'s corner, which draws everything
     the panel does not. `session` is `null` for a refusal that belongs to no
     conversation at all — a start that never made one — and the corner is the
     only reader such a sentence can have.

     `kind` is the third and is for a caller rather than for a reader:
     `SessionError`'s own serde tag, or `null` for a refusal that never came
     from the worker. One caller reads it — `startAgent` in
     `views/DesktopApp.vue`, every road into a session including
     `resumeSession` — and it decides there whether the driven road ever
     actually tried anything at all. `notDriven` is the one tag that means it
     did not, so that is the one tag `startAgent` falls through to the PTY
     road on; every other tag, `badCwd` among them, means an attempt was made
     and failed, and stops there with the sentence on screen — both workers
     asking one function about one path is `badCwd`'s own instance of that
     rule, not the whole of it any more. This is read here rather than worked
     out from the text, because a sentence is what a person reads and never
     what code decides on.

     The rest is `{ session, text }` and not `terminals.js`'s `{ title,
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

/* One structured snapshot per backend-owned Crew package. A Crew node is not
   put in `started`: that array owns ordinary protocol sessions and assumes its
   numeric id can be passed to `session_attach`. The composite key below keeps
   the package root in the identity all the way to Vue. */
const crews = reactive(new Map())

const crewAddress = (id) => {
  if (typeof id !== 'string') return null
  const match = /^crew:(\d+):(\d+)$/.exec(id)
  if (!match) return null
  return { root: Number(match[1]), node: Number(match[2]) }
}

export const crewAgentsIn = (project) =>
  [...crews.values()]
    .filter((crew) => crew.project === project)
    .flatMap((crew) =>
      crewRows(crew.nodes).map((node) => ({
        id: `crew:${crew.root}:${node.id}`,
        crewRoot: crew.root,
        crewNode: node.id,
        project: crew.project,
        state: statusOf(node.state),
        elapsed: '',
        conversation: null,
        work: { kind: 'run' },
        label: node.label,
        tasks: [],
        claimed: [],
        /* Only a package root can be cleared. A native child has no safe
           provider child-stop contract, so its row is readable/selectable but
           never offers the destructive package close action. */
        clearable: node.id === crew.root,
        canMessage: node.canMessage,
        depth: node.depth
      }))
    )

export const crewConversationsIn = (project) => crewAgentsIn(project).map((row) => row.id)

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
    /* `Attached::cwd` — where this session actually runs, filled in by
       `attach`'s snapshot below. `''` until then, which `ConversationView.vue`
       passes straight through as `Markdown.vue`'s `base` prop: that file's own
       `effectiveBase` already falls back to `root` (the project) on an empty
       string, so a component reading this before the snapshot lands, or a
       session the worker never named one for, gets the project root exactly
       as the task inspector does — never a hole. */
    cwd: '',
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
  /* `SessionError::NotDriven` has **no** entry here, deliberately: its text
     is already a sentence rather than an internal one (`session::service`'s
     own two call sites word it that way, the same standard `spawn`'s
     identity entry exists to meet), so it is left to fall through to
     `sentence`'s own raw-message branch below rather than repeating that
     mapping with a second identity function nothing could tell apart from
     the fallback it duplicates — a redundant entry here is exactly the kind
     of "test passes with the feature deleted" trap this file's own review
     caught once. `kind` still carries the tag regardless of whether an
     entry exists for it (`report`, below), which is what `startAgent` in
     `views/DesktopApp.vue` reads to decide whether the PTY road is safe to
     try. Ordinarily nobody ever sees this sentence: `startAgent` takes it
     off the screen and tries the PTY road, and it only reaches a person if
     that road refuses too. */
  /* `SessionError::BadCwd` — the directory a recorded conversation was to be
     reopened in is not a folder inside the project any more. **The ordinary
     case rather than an exotic one**: a worktree is removed once its task is
     merged and the transcript stays behind, so this is what an offline row from
     a finished task answers with.

     **The sentence is `terminals.js`'s own, copied**, and the copy is the point
     rather than the cost: the same press under a harness this app cannot drive
     goes to the PTY worker, which refuses in `TerminalError::BadCwd` and words
     it there, and a person who switched their agent between two attempts must
     not be told two different things about one missing folder. The two tables
     are a pair to change together — there is nothing mechanical between them.

     It has to be a `kind` of its own rather than a `spawn`, because `spawn`
     hands the worker's own text to a person unchanged: this one would arrive as
     `that folder cannot be a working directory: /Users/…/.worktrees/…`, which
     is lower case, has an absolute path in it and is written for whoever fixes
     things. */
  badCwd: () =>
    'Smetana could not start a shell there. The tree may be out of date — refresh it.',
  noSuchSession: (id) => `Session ${id} is not running any more.`,
  noSuchQuestion: (id) =>
    `That question is not waiting for an answer any more (${id}) — it was answered already, or the agent stopped asking.`
}

function sentence(error) {
  const known = ERRORS[error?.kind]
  if (typeof known === 'function') return known(error.message)
  /* Anything else: a plain `Error` from the transport, or a refusal this store
     has no words for — `notDriven` among them now, since it carries no entry
     above. `message` beats a generic line, since a refusal nobody has written
     copy for is exactly where the raw text is worth having; `kind` is next,
     since a bare `SessionError` variant name is still more of an answer than
     nothing; `error` itself is last, only for the shape neither of the first
     two can read anything out of. `??` and not a `typeof … === 'string'`
     guard, because `String(error)` on a plain object with no usable field is
     `[object Object]`, and that is what reaching this branch at all is meant
     to stop — every producer today writes a non-empty `message`, so `??`'s
     own blind spot, an empty string surviving instead of falling through,
     is not live, but it is worth naming rather than silently trusting. */
  return String(error?.message ?? error?.kind ?? error)
}

/* `session` is the conversation the refusal belongs to, and `null` when it
   belongs to none — a start that never made one. It is the first argument
   because it is the thing a caller cannot leave out by accident: every call
   below has one to hand, and the only `null` is written as `null`. */
function report(session, what, error) {
  console.error(`[conversation] ${what} failed:`, error)
  /* `kind` is the worker's own tag and `null` for anything that did not come
     from it — a plain transport error, a refusal this store has no words for.
     It travels beside the sentence rather than instead of it: a caller that
     branched on the text would be reading copy. */
  conversationState.lastError = { session, kind: error?.kind ?? null, text: sentence(error) }
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
   second attach hands back exactly what the first did — with nothing having
   happened to the session in between, which is the ordinary reading of that
   claim and not a promise that time stood still. It is not even a promise
   that the *count* of events stands still with nothing new appended: a reply
   still streaming when the first attach read the journal and closed by the
   time a second one does will hand back a shorter list the second time —
   the deltas the first attach saw one at a time are gone from the worker's
   own copy by then (`session::journal::Journal::append`'s own header),
   folded into the one `text` event that superseded them. `journal.js`'s fold
   draws that shorter list exactly as it would have drawn the longer one live,
   which is what makes the difference invisible on screen.

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
  const address = crewAddress(id)
  const current = address
    ? invoke('crew_attach', address)
    : invoke('session_attach', { id })
  attaching.set(id, current)
  try {
    const { events, seq, state, conversation, cwd, title } = await current
    if (attaching.get(id) !== current) return
    /* Replaced whole and never merged: this *is* the conversation, and the one
       thing a snapshot is for is being trusted over whatever was drawn before
       it. */
    held.events = events ?? []
    held.seq = seq ?? 0
    held.state = state ?? 'starting'
    /* Fixed for the life of the session — the worker names it once, at the
       spawn — so this is a plain assignment rather than a `note*` helper: there
       is no second source to reconcile it against the way `noteConversation`
       reconciles the id against `session:state`. */
    held.cwd = cwd ?? ''
    /* The snapshot is the freshest thing anybody has about this session, so the
       record takes it too — a row drawn from a state event alone would be one
       event behind the panel beside it for as long as nothing moved. */
    noteState(id, held.state)
    /* And the name the row is keyed by, which arrives on the snapshot as well
       as on every state change. This is the earlier of the two roads by a
       whole turn: `session:state` goes out on a *change*, and a session that
       has just started and said nothing has not changed state yet. */
    noteConversation(id, conversation)
    /* Same reasoning, same round trip early: the snapshot is the freshest
       word on the title too. */
    noteTitle(id, title)
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
    made.push(await listenToCrew())
    made.push(await listenToCrewEvents())
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
    const { id, state, conversation, title } = event.payload
    /* The state this session was last known to be in, read before `noteState`
       overwrites it below — the same shape `before` takes in `terminals.js`'s
       own `terminal:state` listener, and for the same reason: the transition
       the chime below fires on is read against what this session was doing a
       moment ago, not against what it is about to become.

       Looked up directly rather than through `noteState`'s own find, because
       the guard below needs a second thing `noteState` does not answer —
       whether this window is still holding a record for the session at all —
       see that guard for why. */
    const record = started.find((session) => session.id === id)
    const before = record?.state
    /* Before the drop below, and deliberately not under it: the row in the
       agents panel hangs on this. The journal is emptied by `detach` and the
       record is not, so a window whose conversation panel has gone must still
       follow the state of the sessions it started — reading it out of
       `conversations` alone had the row freeze at whatever it said when
       somebody last looked at it. */
    noteState(id, state)
    noteConversation(id, conversation)
    noteTitle(id, title)
    /* An agent that has stopped to ask something rings the same sound
       `terminal:state` rings for a PTY session — same sound, same
       `onlyWhenUnfocused`, same rule: on the way *in* only, so a session
       re-announcing the same wait costs nothing.

       This sits above the `held` lookup and its early return on purpose,
       and for the PTY listener's own reason: `held` is this project's panel,
       emptied by `detach` the moment it leaves the screen, while the chime is
       owed for every project a session is running in. `started` already
       reaches all of them — "every driven session this window has, whichever
       project it belongs to" is its own header two screens up — which is the
       same reach `marks` buys the PTY side, so nothing here has to repeat the
       watcher `DesktopApp.vue` was refused for that road: there is no
       single-project store standing between this listener and every project's
       sessions the way `terminalState.sessions` stands on the PTY side.

       `record` has to still be there, and not only `before` have missed a
       match. `forget` takes a session out of `started` without necessarily
       ending it in the worker (see `forget`'s own header) — a `session:state`
       can still arrive afterwards for a session this window has explicitly
       stopped caring about, and ringing about one the person already
       dismissed would undo the dismissal.

       No `isShellSession` guard, unlike the PTY listener: nothing on this road
       is ever a shell. `session::service::drivable` starts a session from an
       `Intent`, and `SessionWork::Shell` is the one kind of session in this
       app with no `Intent` behind it at all — `terminal_shell` never calls
       `session_start`, so a shell can never be a member of `started` for this
       guard to have to ask about. */
    if (record && before !== 'needs-you' && state === 'needs-you') {
      chime(settings.notifications.needsAttention, {
        unlessFocused: settings.notifications.onlyWhenUnfocused
      })
    }
    const held = conversations.get(id)
    if (!held) return
    held.state = state
  })
}

/* The backend emits complete topology snapshots. Applying a snapshot rather
   than incremental provider ids is important: provider ids never arrive here,
   and a late node update cannot make Vue retain an orphan under a stale key. */
function listenToCrew() {
  return listen('crew:tree', (event) => {
    const { project, root, nodes } = event.payload ?? {}
    if (typeof project !== 'string' || !Number.isFinite(root) || !Array.isArray(nodes)) return
    if (!nodes.length) {
      crews.delete(root)
      return
    }
    crews.set(root, { project, root, nodes })
  })
}

function listenToCrewEvents() {
  return listen('crew:events', (event) => {
    const { root, node, events } = event.payload ?? {}
    const id = `crew:${root}:${node}`
    const held = conversations.get(id)
    if (!held || !Array.isArray(events)) return
    if (!absorb(held, events)) attach(id).catch(() => {})
  })
}

/* Which harnesses this app can drive, and the whole of the list.

   A driven session is one whose protocol the worker parses itself, and only
   Claude Code and Codex have drivers — each for every intent a person talks
   to, since `Intent::Run` is the one intent neither harness drives at all.
   Rust repeats that gate after resolving a role override or executable
   substitution, which this inexpensive front-door check cannot see.

   **This is a cheap front door and cannot be the only gate, because it cannot
   see `PATH`.** The caller asks it of `effectiveAgents.value.agent`
   (`components/settings/agentRoles.js`'s `effectiveAgentTable`,
   `.claude/rules/settings.md`) rather than the bare root field, and the first
   half of that chain is exact: `Intent::Bare` takes `agents::Role::Default`,
   which `settings::model::Settings::role_pair` answers project-aware now —
   the active project's own `agents` block where it names a harness, the root
   pair otherwise — and `effectiveAgentTable` is the front end's mirror of the
   identical choice, so a project carrying its own table moves this front door
   with it rather than leaving it to answer about the root's harness alone.
   The half it cannot see is downstream of all of that. `agents::pick`
   substitutes **the first installed profile** when the configured one is not
   on the machine, silently and by design, and `pick_with_model` is what
   `spawn_session` calls. The resolved agent ships as `claude` where nothing
   overrides it and `Settings::validate` forces anything unknown back to it,
   so a machine with only Codex on it answers `true` here and is refused by
   the driver a round trip later. What answers that is the caller: `startAgent`
   in `views/DesktopApp.vue` falls through to `createSession` when a driven
   start comes back with nothing, and `createSession` resolves whatever `pick`
   would have. Nothing here should grow a second guess at `PATH` instead — the
   front end does not have one.

   A list here rather than a capability on the harness row, because there is no
   flag for this: `agents::Capabilities` carries `resume`, `fork`, `clear`,
   `usage`, `batch` and `oneshot`, and none of them means "has a driver". The
   day a second harness grows one, this list and `driver_for` are the two places
   that have to agree, which is why this one names the other.

   **The person's own switch is inside this answer rather than beside it.**
   `settings.conversationPanel` off makes every harness answer `false` here, so
   every road into a session takes the PTY without a second condition anywhere
   — a `if (!settings.conversationPanel)` inside each of the ten starts that
   talk to an agent would be ten copies of one rule, and copies drift apart.
   `startAgent` is the one caller now, and every one of those ten routes
   through it, asking this once per press. It is in front of the list rather
   than in it: the list is what Rust can drive and is not the person's to
   edit, and `session::service::driver_for` is untouched by this switch. The
   front end simply stops asking.

   Read at the moment it is asked and never cached, which is the whole of
   "changes what starts, not what runs": a panel already on screen goes on being
   a panel, and the next session opens in a terminal. */
const DRIVEN = ['claude', 'codex']

export const canDrive = (agent) => settings.conversationPanel && DRIVEN.includes(agent)

/* Which driven sessions this window has started, and in which project.

   Kept beside the conversations rather than read out of them, because the two
   answer different questions. `conversations` above is what this window is
   drawing *right now*, and `detach` empties it the moment a panel goes away;
   this says which sessions a project has at all, which is what the centre's
   Agent tab is derived from (`hasAgentTab` in `stores/tabs.js`). Derived from
   the other, that tab would disappear the moment somebody looked at the board
   and take the way back to their agent with it.

   **One gesture takes an entry out and it is the only one: the cross on the
   row** (`forget` below). A session whose child has gone keeps its row and its
   tab until somebody closes it, on the grounds that the last words of whatever
   was running are worth reading — the terminal's behaviour, and the reason
   nothing here expires on its own; the cross is the somebody that closes it,
   and until this list was drawn there was nobody. A restart empties it — no
   session's *process* survives one — which is the same repair `restoreTabs`
   already makes for a remembered `activeTab: "terminal"`. What does survive is
   the record in `.smetana/agents.json` the worker wrote at the spawn, and the
   next launch offers it back as an offline row of the agents panel like any
   other; `conversation` below is the name the two halves meet under.

   **A record carries the session's state and the moment it started**, and
   neither is read out of `conversations` above. That map is emptied by
   `detach`, so a row built from it would go blank the moment the centre tab
   moved to the board — which is precisely when somebody glances at the panel to
   see how their agent is getting on. The state is therefore written here by the
   state listener as well, for every session this window has started rather than
   only for the ones it is drawing.

   The project is the path `startConversation` was given, which is the same
   string the terminal store keys its own sessions by: the project's own
   folder. */
const started = reactive([])

/* The driven sessions of one project, oldest first. An array of ids rather than
   of records, because that is the whole of what a caller wants — the tab is
   derived from whether there are any, and the panel from which one is picked. */
export const conversationsIn = (project) =>
  started.filter((session) => session.project === project).map((session) => session.id)

/* Every driven session this window holds, whichever project it belongs to: the
   id, the project, the worker's own word for its state and the moment it
   started.

   Copies rather than the records, so that a reader cannot write one back. This
   is what the agents panel's rows, the footer's counter and the project rail
   are all built from — `components/agent/drivenRows.js` is the rule, and
   `views/DesktopApp.vue` is the one caller — and a view able to edit the list
   it draws would be a second author of this store's state.

   Every project rather than one, because the three readers do not agree on how
   many they want: the panel and the counter are about the project on screen,
   and the rail is about all of them at once. Filtering is the caller's, and
   cheap; a second exported filter here would only be a third answer to a
   question these two already answer between them.

   `conversationsIn` above stays the narrower answer it always was: its callers
   ask whether a project has a conversation at all, not what any of them is
   doing. */
export const drivenSessions = computed(() => started.map((session) => ({ ...session })))

/* The state on the record, for a session this window has started.

   Written beside the conversation's own state rather than instead of it,
   because the two have different lifetimes and that is the whole point: a
   conversation goes at `detach` and this record stays, so a row in the sidebar
   goes on saying what its agent is doing after the panel drawing it has left
   the screen. A session this window never started has no record and nothing
   happens — `session:state` arrives for every session of every project. */
function noteState(id, state) {
  const record = started.find((session) => session.id === id)
  if (record) record.state = state
}

/* The name that outlives the session, for a session this window has started.

   **It is written rather than returned by the start**, and that is a fact about
   the worker rather than a shape chosen here: `session_start` answers with the
   worker's own session number, which counts from 1 on every launch, and the id
   the conversation is *recorded* under is minted a moment later, inside the
   spawn. It arrives twice over — on the `session_attach` snapshot, which is the
   next thing `startConversation` does, and on every `session:state` after that
   — and this is the one place either of them lands.

   `null` is an ordinary answer and stays one for a session whose harness never
   hands this app an id at all: `--fork-session` has Claude Code invent one
   this app never learns, and a machine that would not give the random bytes
   is the same absence for a different reason. Such a row is keyed by
   `drivenRowId` instead and simply does not survive a restart, which is what
   `agentMenu.js`'s `nothing to remember it by` says on its Pin. **A driven
   Codex fork is not drawn from that list any more** — its app-server invents
   the new id too, but hands it back in `thread/fork`'s own reply, and
   `session::service`'s own `note_conversation` writes it into `session:state`
   a turn or two after the snapshot this function's other caller,
   `session_attach`, already answered with `null`. This is the one path that
   still reaches a row through this function rather than through
   `session_attach`: a null that arrives here later is a real answer landing
   late, not the row's last word on the subject, and the Pin's own refusal
   stops applying to such a row the moment this fires.

   Never written back to `null` over a value: the two roads carry the same id
   and a payload that arrived without one is a build that stopped sending it,
   not a session that has lost its name. */
function noteConversation(id, conversation) {
  if (conversation == null) return
  const record = started.find((session) => session.id === id)
  if (record) record.conversation = conversation
}

/* The automatic title, for a session this window has started.

   Never written back to `null` over a value, for `noteConversation`'s own
   reason above: a payload without one is a build that stopped sending it,
   not a session that has lost its name. An empty or whitespace-only string is
   read as the same absence, since that is what the worker sends before it has
   words at all — `Restorable.title` and `StateChange.title` are both `Option`
   but the wire has no way to distinguish "not yet" from "explicitly empty",
   and there is no case where a session's title is meant to be blank. */
function noteTitle(id, title) {
  if (typeof title !== 'string' || !title.trim()) return
  const record = started.find((session) => session.id === id)
  if (record) record.title = title
}

/* This window stops holding a session at all: the record goes, and the row in
   the agents panel with it.

   The cross on that row is the whole of what calls this — the one gesture the
   header above says empties this list. The journal goes too, through `detach`,
   and that is not tidying up: the row is gone, so nothing is left that could
   ask for this conversation again, and a journal kept for it would grow in a
   window that draws none of it, which is the cost `detach` exists to avoid.

   Ending the session is the caller's other half and deliberately not done
   here: what that costs the harness is `closeConversation`'s to say (Stop's
   own `stopConversation` is a different verb since smetana-y7mv and does not
   end a session at all), and a refusal of it is a sentence for the toast
   corner rather than a reason to keep a row somebody has just dismissed. */
export function forget(id) {
  const at = started.findIndex((session) => session.id === id)
  if (at !== -1) started.splice(at, 1)
  detach(id)
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
    /* Before the attach rather than after it: the Agent tab is derived from
       this list, and a session held only once its snapshot had come back would
       leave the button somebody pressed with no visible effect for the length
       of a spawn — the same reason `terminalState.starting` exists one
       subsystem over. */
    /* `starting` and this window's own clock. The worker mints neither for a
       driven session — `session_attach` answers with a journal, a sequence
       number and a state, and nothing about when the session began — and the
       moment the start answered is within one spawn of the truth. The state is
       the word `hold` starts a conversation on, for the same reason it does: a
       session that has produced nothing yet has not failed to. */
    started.push({
      id,
      project,
      state: 'starting',
      startedAt: Date.now(),
      /* Filled in by `noteConversation` a round trip later — the worker mints
         it inside the spawn and `session_start` answers with its own session
         number alone. Until then the row is keyed by `drivenRowId`, which is
         what that function's absence has always meant. */
      conversation: null,
      work: workOf(intent),
      /* Filled in by `noteTitle`, from the same two places `conversation`
         above is filled in from: the attach snapshot and every
         `session:state` after it. `null` until the worker has words —
         `captions.js`'s `captionOf` reads that the same way it reads a fresh
         session's `conversation: null`, as "nothing yet" rather than "never
         will". */
      title: null
    })
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
    const address = crewAddress(id)
    if (address) {
      if (attachments.length) throw new Error('Crew messages cannot include attachments')
      await invoke('crew_send', { ...address, text })
    } else {
      await invoke('session_send', { id, text, attachments })
    }
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
   was asked, and it checks.

   `answers` is `AskUserQuestion`'s own: the text of each question mapped to
   what was chosen or typed, built by `askUserQuestion.js` and handed over by
   `AskUserQuestion.vue`. `null` — never `undefined`, which `invoke` would
   drop from the payload entirely and leave Rust reading a missing field
   rather than an absent one — for the ordinary allow/deny every other tool's
   `PermissionRequest.vue` still sends, and for a decline to answer, which is
   a plain `deny` whatever tool it is refusing. */
export async function answerQuestion(id, question, decision, answers = null) {
  try {
    if (crewAddress(id)) throw new Error('Crew agents do not expose permission answers')
    await invoke('session_answer', { id, question, decision, answers })
    conversationState.lastError = null
  } catch (err) {
    report(id, 'answering a question', err)
  }
}

/* Stop the turn in flight — the composer's Stop button, and never the row's
   cross. What that costs the harness is the driver's business: for Claude
   Code it is one `control_request` over stdin that ends the turn and leaves
   the session open (smetana-y7mv, `src-tauri/src/agents/claude_driver.rs`'s
   own header carries the measurement); a harness with no such answer still
   loses the child. `closeConversation` below is the other one, for ending a
   session outright. */
export async function stopConversation(id) {
  try {
    const address = crewAddress(id)
    if (address) {
      if (address.node !== address.root) throw new Error('Only the Crew lead can be stopped')
      await invoke('crew_stop', { root: address.root })
    } else {
      await invoke('session_stop', { id })
    }
    conversationState.lastError = null
  } catch (err) {
    report(id, 'stopping a session', err)
  }
}

/* End the session outright — the cross on a driven agent row
   (`DesktopApp.vue`'s `removeAgentRow`), never the composer's Stop. Always
   kills the child, on every harness, so the worker's own cleanup on the
   child's exit still runs: the permission token is forgotten, the
   `.smetana/agents.json` record is dropped and the `--mcp-config` file goes
   with it (`session/service.rs`'s `Request::Close`, smetana-y7mv). Before
   this existed `stopConversation` did that job too, because killing the
   child was the only thing Stop ever did for any harness; once Claude Code's
   own `interrupt` started leaving the child alive, `stopConversation` could
   no longer be trusted to end a session at all. */
export async function closeConversation(id) {
  try {
    const address = crewAddress(id)
    if (address) await invoke('crew_clear', { root: address.root })
    else await invoke('session_close', { id })
    conversationState.lastError = null
  } catch (err) {
    report(id, 'closing a session', err)
  }
}
