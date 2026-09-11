/* What a driven conversation is in the three places this app counts agents: as
   a row of the agents panel, in the footer's two numbers, and in the state a
   project's tile carries in the rail. Pure, no Vue and no DOM in it, which is
   what makes the whole of the rule reachable by a test — `agentOrder.js`'s
   reason, one file over.

   **The merge lives here rather than in `stores/terminals.js`.** That store is
   the PTY sessions and nothing else, and teaching it a second store is exactly
   the growth epic smetana-79j5 shut the door on: the terminal is going away,
   and what it knows is not worth enlarging on the way out. Every consumer of
   the three lists it exports — `agentRows`, `agentCounts`/`liveAgentCount` and
   `projectStates` — is read in `views/DesktopApp.vue` and nowhere else, so that
   is where the two kinds of session meet, and this is the rule they meet by.

   **Everything here is in the design system's status vocabulary**
   (`status/status.js`), never `session::model::SessionState`'s. The caller
   translates once, through `statusOf` in `stores/conversation.js`, and by the
   time a driven session reaches this file it is spelled the way a PTY one
   already is — which is what lets one sentence cover both. It is also what
   keeps this module out of the stores: `statusOf` lives in one of the few files
   allowed to import Tauri, and borrowing it here would pull Tauri into a family
   defined by having neither it nor Vue. The translation is a single call at the
   seam; the rules are all below it. */

/* The prefix a driven row's id carries, and the whole reason it has one.

   `SessionId` is a `u64` counter in the session worker that starts at 1, and
   the terminal worker's own counter starts at 1 as well: the two spaces collide
   on their first session apiece. What that collides in is not cosmetic —
   `agentKey` is `row.conversation ?? row.id`, and a driven session deliberately
   has no conversation id (`session_id: None` in `spawn_session`, the stage that
   adds restoring being the one that mints it), so the key falls through to the
   number. That key is what carries the drawn order, the pins and the `v-for`,
   and two rows under one key is two agents fighting over one place in the list.

   A prefix parts the two spaces and leaves `conversation` honestly empty rather
   than inventing a value for it. A start ticket's `start-N` is the same trick
   one list over and cannot be confused with this one either. */
export const DRIVEN_PREFIX = 'conversation:'

export const drivenRowId = (session) => `${DRIVEN_PREFIX}${session}`

/* The session behind such a row, and `null` for every other row the panel
   draws. This is what routes a click and a cross: the panel hands back the id
   it was given and both verbs have to know which of the two worlds it names
   before they do anything at all, so the test is here rather than in either
   caller.

   A number comes back, because that is what the worker answers to — the string
   is this window's own bookkeeping and stops at the wire. Anything that is not
   this file's own id is `null`, including a bare prefix and a tail that is not
   a number: a row of some future kind must read as "not a conversation" rather
   than as conversation `NaN`. */
export function drivenSessionOf(rowId) {
  if (typeof rowId !== 'string' || !rowId.startsWith(DRIVEN_PREFIX)) return null
  const tail = rowId.slice(DRIVEN_PREFIX.length)
  if (!tail) return null
  const session = Number(tail)
  return Number.isFinite(session) ? session : null
}

/* One driven session as a row of the agents panel.

   The caption is written out rather than borrowed, and it is deliberately the
   same word: `CAPTION.bare` in `stores/terminals.js` captions a session started
   by "+ New agent" as `Agent`, and this is that same button's other road. That
   table is private to the store, and this file could not reach into it without
   dragging Tauri in behind it, so the copy is what the seam costs — the two are
   a pair to change together if a bare agent is ever captioned anything else.

   `conversation: null` is the truth about this row rather than a hole in it,
   and the menu already has the sentence for it: `nothing to remember it by`. A
   driven conversation does not survive a restart of the app, so there is
   genuinely nothing for a pin to bring it back under — the refusal is honest
   twice over.

   `clearable: false` is the same shape of fact. Clearing is a line written into
   a PTY — `Profile::clear_command` — and this session has no PTY to write into.
   The menu refuses it with `this agent cannot do it`, which is a sentence about
   the road rather than about Claude Code, since the harness itself clears
   perfectly well. If that ever reads as a lie to somebody standing in front of
   it, the answer is a reason of its own in `agentMenu.js` and not a `true`
   here.

   `starting` is deliberately absent, unlike a start ticket's. A ticket stands
   for the second before the worker answers; this row exists only because
   `session_start` already has, so there is a session behind it to stop and the
   cross is live from the first frame. */
export function drivenAgentRow({ id, state, elapsed }) {
  return {
    id: drivenRowId(id),
    conversation: null,
    clearable: false,
    work: { kind: 'bare' },
    claimed: [],
    label: 'Agent',
    tasks: [],
    state,
    elapsed
  }
}

/* The panel's rows, both kinds, in the order the panel puts them before the
   person's own is applied on top.

   Driven rows go after the live sessions and the starts, where a newly started
   agent belongs — at the end, which is where somebody who has never dragged
   anything looks for the one they have just pressed a button for — but **in
   front of the rows a previous run of the app left behind.** That is the
   terminal store's own rule about those, written where it builds them: they are
   the project's past, and the agents somebody is actually watching keep the top
   of the column. Appending blindly would have put a live conversation
   underneath yesterday's offline record, which is the one arrangement that rule
   refuses. The restored rows are the tail of the list by construction, so the
   first of them is where these go in.

   The whole list is then handed to `orderAgents` at once: a drag can carry a
   driven row past a PTY one, the panel being one flat zone, and these are not a
   fourth group with a rule of their own.

   With no driven session the rows come back by reference, which is the contract
   `moveAgent` and `orderAgents` already keep on this road: the caller's next
   step is `orderAgents`, which leans on identity to tell "never arranged" from
   "arranged, and this is what it came to". */
export function mergeAgentRows(rows, sessions) {
  const driven = (sessions ?? []).map(drivenAgentRow)
  if (!driven.length) return rows
  const list = rows ?? []
  const past = list.findIndex((row) => row?.restored)
  const at = past === -1 ? list.length : past
  return [...list.slice(0, at), ...driven, ...list.slice(at)]
}

/* The states in which a driven session is over.

   The pair is exactly what the terminal store's counter leaves out, arrived at
   from the other end: there a session is counted unless its raw state is
   `exited`, and `toUiState` turns that one raw state into these two words by
   reading the exit code. A driven session has no exit code — `state_of` has
   already decided which of the two endings it was — so the same exclusion has
   to be spelled as the pair.

   A closed list of the dead states rather than of the live ones, which is
   `agentMenu.js`'s choice one file over and the same reason: a word added to
   `SessionState` and not yet to this list reads as a live agent, and a counter
   that is one too high for a state nobody has heard of is a better failure than
   one that quietly stops counting a working agent. */
const ENDED = ['done', 'failed']

const liveCount = (sessions) => (sessions ?? []).filter((s) => !ENDED.includes(s.state)).length

const loudCount = (sessions) => (sessions ?? []).filter((s) => s.state === 'needs-you').length

/* The footer's agents counter, with the driven sessions of the same project
   added to it. `needs-you` counts, as it does on the terminal's side: an agent
   waiting on an answer is the reason somebody is reading this bar. */
export function mergeLiveAgentCount(count, sessions) {
  return (count ?? 0) + liveCount(sessions)
}

/* The same agents split the way `components/shell/headline.js` needs them.

   `live` stays "alive but not waiting", which is the shape the terminal store
   hands over, so the driven sessions are added the same way they are counted
   above and the loud ones taken back out. The clamp is the store's own and is
   kept for its reason: the subtraction is exact today — a `needs-you` session
   is one of the live ones by construction — and if that ever stops being true
   the sentence goes quiet instead of announcing "-1 agents running".

   The sentence and the counter beside it are one thing to a person reading the
   bar, so both are merged here rather than one of them: a driven session
   counted in the number and not in the words would be two readings of the same
   project an inch apart, which is the failure the store already carries a
   paragraph about. */
export function mergeAgentCounts(counts, sessions) {
  const loud = loudCount(sessions)
  const live = liveCount(sessions)
  return {
    loud: (counts?.loud ?? 0) + loud,
    live: Math.max(0, (counts?.live ?? 0) + live - loud)
  }
}

/* The rail's map, with the driven sessions counted into it.

   The map's own rule is copied rather than reached for, because the store keeps
   it inside a computed: a session waiting on somebody is `loud`, one working is
   `live`, and anything else leaves the project as it found it. `running` alone
   is the live word here and that is the translation being exact rather than
   thin — the store counts raw `running` *and* `starting`, and `statusOf` folds
   both of those into `running` before this file sees them. A session that has
   spoken and is waiting on nobody is `ready`, which the store's own `idle`
   reads as too, and which counts for neither: a project is not "live" because
   an agent is sitting there with nothing to do.

   A fresh map with fresh rows, never the store's own objects: that map is a
   computed and writing into it would be a second author of a derived value. The
   `state` of every row is worked out again from its counts, including the rows
   no driven session touched — it is the same rule over the same numbers, so the
   answer for those is the one the store already gave. */
export function mergeProjectStates(states, sessions) {
  const out = {}
  for (const [project, row] of Object.entries(states ?? {})) out[project] = { ...row }
  for (const session of sessions ?? []) {
    const row = (out[session.project] ??= { state: 'idle', live: 0, loud: 0 })
    if (session.state === 'needs-you') row.loud += 1
    else if (session.state === 'running') row.live += 1
  }
  for (const row of Object.values(out)) {
    row.state = row.loud ? 'loud' : row.live ? 'live' : 'idle'
  }
  return out
}
