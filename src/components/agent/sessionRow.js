/* What a session row says, worked out from the record the worker hands over.

   The whole of the rule lives here rather than in `SessionRow.vue`, for the
   reason every other module of this shape exists: a `.vue` file is the one
   thing no test in this repository can reach, so the prefix, the plural, the
   ordering of the meta line and the time label are checkable and the component
   is left with nothing but boxes and tokens.

   This is deliberately not `terminals.js`'s `formatElapsed`. That one measures
   how long a live agent has been running and is written for a column of them —
   `2h 14m`, monospaced, so the digits do not wander as the clock ticks. This
   one answers a different question about a different subject: how long ago
   somebody last spoke to a session that is over, which is read once and in
   prose. Sharing a function between the two would mean one of the two lying. */

/* A session nobody has said anything to yet, and a transcript whose first human
   message the worker could not find. The id is not offered instead: the ids are
   file stems of the same length and shape, so a column of them tells a person
   less than a column of the same three words. */
const UNTITLED = 'Untitled session'

/* Who spoke last. The words are Orca's and are worth keeping: `You:` and
   `Agent:` are what a person scanning a list of conversations reads without
   stopping, while `user`/`assistant` are the transcript's own vocabulary and
   belong to the file rather than to the screen. */
const ROLE_PREFIX = { user: 'You', assistant: 'Agent' }

/* A cap on what reaches the DOM, not on what is shown: the visible cut is the
   ellipsis in the component, and the worker has already clipped both strings
   for the wire. What this guards against is a record that arrives unclipped —
   an older worker, a hand-written fixture — putting a 16 MB line into a `<span>`
   where nothing but the layout would ever notice. Generous on purpose, so that
   in every ordinary case it does not fire at all. */
const MAX_CHARS = 400

/* One line out of whatever the transcript held. Newlines and runs of spaces
   collapse to a single space: a row is two lines tall by design, and a message
   that arrived with its own line breaks would otherwise decide for itself how
   many of them it takes. Empty and blank both come back as null, which is what
   lets every caller here treat "no text" as one case. */
export function oneLine(text) {
  if (typeof text !== 'string') return null
  const flat = text.replace(/\s+/g, ' ').trim()
  if (!flat) return null
  return flat.length > MAX_CHARS ? `${flat.slice(0, MAX_CHARS)}…` : flat
}

/** The row's first line: the first thing the person said, or the words for its absence. */
export function sessionTitle(session) {
  return oneLine(session?.title) ?? UNTITLED
}

/* The row's second line: who spoke last and the start of what they said.

   Null when there is no last message *or* no role to attribute it to. An
   unattributed line is the one thing this line must not be — "Agent:" over the
   person's own words is a worse answer than no line at all, and a role this
   front end has never heard of is an ordinary outcome rather than an error. */
export function lastMessageLine(session) {
  const text = oneLine(session?.lastText)
  const who = ROLE_PREFIX[session?.lastRole]
  return text && who ? `${who}: ${text}` : null
}

/** `48 msgs`, and `1 msg` — Orca's abbreviation, on a line with no room for prose. */
export function messageLabel(count) {
  const n = Number.isFinite(count) ? Math.max(0, Math.trunc(count)) : 0
  return `${n} ${n === 1 ? 'msg' : 'msgs'}`
}

/* `3 subagents`, and nothing at all when there were none.

   The absence is the point: a session with no sidechain records is the ordinary
   case, and `0 subagents` on every second row would spend a third of the meta
   line saying that nothing happened. Spelled out rather than abbreviated,
   unlike the messages beside it, because it is the rarer of the two and the
   word is what makes it stand out from the count next to it. */
export function subagentLabel(count) {
  const n = Number.isFinite(count) ? Math.trunc(count) : 0
  if (n <= 0) return null
  return `${n} ${n === 1 ? 'subagent' : 'subagents'}`
}

const MINUTE = 60
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR
const WEEK = 7 * DAY
const YEAR = 365 * DAY

/* When the session was last written to, relative: `18h ago`.

   Relative rather than a date, and the objection to it is real and recorded in
   `kanban/TaskInspector.vue`: a label computed once turns into a lie the moment
   the app is left open overnight. It is answered rather than ignored — the
   store ticks a `now` and hands it in here, exactly as `terminals.js` does for
   an agent's elapsed time, so the label is recomputed while the window sits
   open and never drifts from the clock. That is also why `now` is an argument:
   it keeps this function pure and the only thing a test has to arrange.

   A time in the future is clamped rather than rejected. The clock ticks once a
   minute, the mtime comes off another machine's idea of the same second, and
   "in 30 seconds" for a session somebody is in right now would be a strange
   thing to read.

   The ladder stops at years: this list is a project's whole history, and the
   oldest transcripts on the machine this was written against are months old. */
export function relativeTime(iso, now) {
  const at = Date.parse(iso)
  if (!Number.isFinite(at) || !Number.isFinite(now)) return null
  const secs = Math.max(0, Math.floor((now - at) / 1000))
  if (secs < MINUTE) return 'just now'
  if (secs < HOUR) return `${Math.floor(secs / MINUTE)}m ago`
  if (secs < DAY) return `${Math.floor(secs / HOUR)}h ago`
  if (secs < WEEK) return `${Math.floor(secs / DAY)}d ago`
  if (secs < YEAR) return `${Math.floor(secs / WEEK)}w ago`
  return `${Math.floor(secs / YEAR)}y ago`
}

/* The row's third line, in pieces rather than as one string.

   `mono` is carried per piece because the line holds two vocabularies at once:
   a model id and a branch name are identifiers and go in `--font-mono`, while
   the counts and the time are prose about them and go in sans. That is the
   project's rule rather than this row's taste, and joining the pieces here
   would put the decision in the component, where no test can see it.

   The order is the one Orca reads left to right — what it is, how much of it,
   when — with the branch last because it is the piece a person looks for
   deliberately rather than one they scan past. Anything the worker could not
   answer is left out entirely: a missing model or a session with no branch is
   an ordinary outcome, and a row saying `unknown` twice would be noisier than
   one that is simply shorter. */
export function sessionMeta(session, now) {
  const subagents = subagentLabel(session?.subagents)
  const when = relativeTime(session?.modifiedAt, now)
  const pieces = [
    session?.model ? { text: session.model, mono: true } : null,
    /* The one piece that is always drawn: a session nothing else is known about
       is still a session, and a row whose third line came out empty would read
       as one that failed to draw rather than as one nobody has told us about. */
    { text: messageLabel(session?.messages), mono: false },
    subagents ? { text: subagents, mono: false } : null,
    when ? { text: when, mono: false } : null,
    session?.branch ? { text: session.branch, mono: true } : null
  ]
  return pieces.filter(Boolean)
}
