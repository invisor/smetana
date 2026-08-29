/* What a session row's menu offers, what each of its copying verbs puts on the
   clipboard, and what the dialog that deletes a transcript says.

   The `fileMenu.js` / `taskMenu.js` / `sessionRow.js` family: pure, no Vue and
   no DOM, which is the whole reason it is a file of its own — a `.vue` file is
   the one thing no test in this repository can reach, so a rule left inside the
   component is a rule nothing checks. It is also one half of a pair nothing
   else joins: these `kind` strings are matched by hand in `DesktopApp.vue`, the
   same seam `fileMenu.js`/`onFileAction` and `newTabMenu.js`/`onNewTab` have,
   where a row renamed on one side draws perfectly and does nothing at all when
   pressed. The test pins this side.

   All nine of the verbs Orca offers on a session are here. The first two are
   not about the file at all — Resume in worktree and Continue in a new session
   bring a live agent back, through the same `terminal_create` every other agent
   in this app is started by — and Copy resume command stays beside them for
   somebody who would rather paste `cd … && claude --resume …` into their own
   terminal than have this app spawn anything.

   One thing is deliberately not Orca's: its second row is called `Continue in
   New Session…`, and the ellipsis is there because it asks the person
   something. Ours asks nothing — it starts an agent exactly as the row above it
   does — so the label carries no ellipsis. */
import { fileManagerName } from '../files/fileMenu.js'
import { COPIED_MS } from '../kanban/copyId.js'
import { formatBytes } from '../settings/storage.js'

/* Borrowed rather than written again, all three, and each for the reason the
   hazards list gives for pairs: `fileManagerName` is already this app's answer
   to what the platform calls Finder, `formatBytes` is already its answer to how
   many bytes read as a size, and `COPIED_MS` is already its answer to how long
   a copy says it worked for. A second copy of either would be free to drift
   into a different word for the same thing with every suite green — the file
   tree's menu saying "Reveal in Finder" while this one said "Reveal in
   Explorer" on the same machine, or a dialog calling 1024 bytes a kilobyte
   under a Storage tab that calls it a KiB, or a session's confirmation fading
   at a different speed from a task id's two panels away. None of the three
   imports pulls Vue or Tauri in: all four modules are the same pure family.

   `COPIED_MS` is re-exported rather than merely used, so that whatever draws a
   session row has one place to import its rule from and does not have to know
   that half of it is the board's. Nothing under `src/` takes it from here
   today, and that is recent: `views/Gallery.vue` did, and the policy it wanted
   the number for is `core/copyFeedback.js` now, which borrows it from
   `kanban/copyId.js` itself. The line stays anyway, because it is one name for
   one number either way — a re-export cannot drift, which is the whole
   difference between this and the three copies it replaced — and because the
   thing it stands in the way of is whoever wants a dwell next declaring the
   number again where they needed it. */
export { COPIED_MS }

/* How wide the menu may get. A ceiling and not a width — `ContextMenu` draws
   itself as wide as its widest row and clips there with an ellipsis, and a menu
   row has no tooltip and no `title`, so whatever does not fit is gone with no
   way back.

   The longest label this file can produce is one of the two launching rows
   greyed and carrying its own reason: `Resume in worktree — this agent cannot
   resume by id` and `Continue in a new session — this agent cannot fork`, 51
   characters each, against `Reveal log in file manager` at 26 for the longest
   of the plain verbs. That is the same shape `taskMenu.js`'s Run row has and
   the reason the number moved from 240 — a row that says *why* it cannot be
   pressed is worth more than a narrow panel, and those two are the only rows
   here that ever carry a sentence.

   Measured in the webview at `--text-sm` in `--font-sans`, by cloning a drawn
   row's own label span: 324px for that sentence and 155px for the reveal — 6.4px
   a character. `ContextMenu` spends 70px on chrome before the label — `MENU_W`
   in `kanban/taskMenu.js` itemises where those pixels go, and the measurement
   above confirms it, the panel coming out at exactly label + 70. So the binding
   row wants 394px, and 440 leaves 370: about a seventh of headroom for Segoe UI
   and Noto Sans, whose metrics nothing here can see, which is the share
   `taskMenu.js` left itself for the same reason.

   **The ceiling is what the refusals are worded against, and not the other way
   round.** This menu opens over a side panel — 340px in the gallery and
   narrower under a dragged splitter — so a panel grown to fit a longer sentence
   is a panel wider than the column its own trigger stands in. The fork's
   fragment is the terse `cannot fork` for that reason rather than by
   carelessness: in both of the frames it is set in, the verb is on screen
   beside it — joined to `Continue in a new session` here, and directly under a
   button of that name in the opened card — so the sentence has its subject
   either way.

   Not `taskMenu.js`'s `MENU_W` imported: two menus, two different binding rows,
   and one number serving both would move this panel every time a task's Run
   reason was reworded. */
export const SESSION_MENU_W = 440

/* ---- the two verbs that start something ---------------------------------- */

/* What the two rows are called. Orca's own names for them, in this system's
   sentence case, and its own reading of the first: the session either of them
   brings back is usually one out of a worktree, and that is the whole point —
   the agent comes up in the directory its transcript was written in.

   No ellipsis on the second, where Orca has one. An ellipsis is a promise to
   ask something, and this row asks nothing: it starts an agent exactly as the
   row above it does, in the same directory and on the same history. */
export const RESUME_LABEL = 'Resume in worktree'
export const FORK_LABEL = 'Continue in a new session'

/* And what each row raises when it is pressed. Constants rather than literals
   because these two verbs are drawn twice over — the menu rows here and the
   buttons in the opened card — and each pair has to raise the same `action` or
   the card's press would fall through `onSessionAction` doing nothing at all.
   The kinds of the seven verbs that reach a file stay literals: each is written
   once here and matched once in `DesktopApp.vue`, which is the seam this whole
   family has and the test pins. */
export const RESUME_KIND = 'resume'
export const FORK_KIND = 'fork'

/* Why one of them cannot be pressed, as lowercase fragments: the menu joins one
   onto the label with a dash the way `taskMenu.js`'s Run row does, and the
   opened card sets the same fragment as a sentence of its own. One wording, two
   frames — the alternative was two copies free to drift into two different
   accounts of one refusal.

   Two of the three fragments are shared between the verbs and one is not, and
   that is the shape of the rule rather than an economy: a missing working
   directory refuses both for the same reason and in the same words, while the
   harness is asked two separate questions and an answer of "no" to one of them
   must not be reported as the other. */
const SESSION_REASON = {
  noDirectory: 'no working directory recorded',
  gone: 'the working directory is gone'
}
const RESUME_REASON = { ...SESSION_REASON, agent: 'this agent cannot resume by id' }
const FORK_REASON = { ...SESSION_REASON, agent: 'this agent cannot fork' }

/* The agent ids that can pick a recorded session up by its id, and the ones
   that can carry one on in a new session instead.

   **The second copy of a fact Rust owns**, and the first is `Profile::resume_args`
   and `Profile::fork_args` — `agents/claude.rs` answers with `--resume <id>`
   and with that plus `--fork-session`, and `agents/codex.rs` keeps both
   defaults of `None`, because that harness's argument grammar is its own and
   this app does not get to guess it. Nothing mechanical joins the two sides: a
   profile that learned to resume and was not added here goes on drawing a
   greyed row, and one added here that Rust cannot serve is refused at the spawn
   with `TerminalError::NoResume` or `NoFork` rather than starting a fresh
   agent. Both failures are quiet, and the second is the one with a sentence
   behind it, which is why the list may be wrong here and never there.

   Two lists and not one, mirroring the two methods: reopening a transcript and
   branching it are two capabilities, and a harness that grows the first without
   the second is an ordinary shape rather than a contradiction. They happen to
   hold the same one id today.

   Written out rather than asked over the wire because the answer has to be
   known while the row is being *drawn* — a menu greyed a round trip later is a
   menu somebody has already pressed. `usageFooter.js` keeps the other
   front-end table keyed by these same ids and records the same thing about
   where the truth lives. */
const RESUMES_BY_ID = ['claude']
const FORKS_BY_ID = ['claude']

/* Whether this session can be brought back as a live agent right now, and the
   reason when it cannot. `fork` picks which of the two verbs is being asked
   about: `false` is Resume in worktree, which goes on writing into the
   transcript it opened, and `true` is Continue in a new session, which leaves
   that file alone and starts a second one from the same history.

   Three refusals, in the order they are asked. The agent first, because it is a
   fact about the whole project rather than about this row — if the configured
   harness cannot do it at all, every card says the same thing and there is
   nothing about any one session left to explain. Then the two about the session
   itself: a transcript that recorded no working directory, and one whose
   directory has gone. The second of those is the ordinary case rather than an
   exotic one — a worktree is removed once its task is merged and the transcript
   stays behind. Both verbs are refused by it, and they have to be: `--resume`
   resolves an id against the directory it is run in either way, so the fork
   would have no more of a place to run than the resume does.

   `agent` is the configured id, not whatever actually starts: `agents::pick`
   substitutes an installed harness for a configured one that is not on `PATH`,
   and nothing on screen ever learns what it picked. So a project set to an
   agent that cannot resume greys the row even in the case where the
   substitution would have made it work — the honest reading of what the app was
   told, and the alternative is a row that promises on a guess.

   `cwdExists` has to be exactly `false` to refuse. A record that does not carry
   the field at all is one this front end cannot answer for — a hand-written
   fixture, a worker older than the field — and offering the verb is the softer
   way to be wrong: the spawn's own guard refuses a directory that is not there
   and says so in words, while greying the row would take a working session away
   with no way to find out why. */
export function resumeAvailability(session, { agent = '', fork = false } = {}) {
  const refuse = (reason) => ({ available: false, reason })
  const reason = fork ? FORK_REASON : RESUME_REASON
  const able = fork ? FORKS_BY_ID : RESUMES_BY_ID
  if (!able.includes(agent)) return refuse(reason.agent)
  if (!session?.id || !session?.cwd) return refuse(reason.noDirectory)
  if (session.cwdExists === false) return refuse(reason.gone)
  return { available: true, reason: null }
}

/* The menu row's label, with the reason joined onto it when there is one.

   `taskMenu.js`'s `runLabel` is the precedent and this is the same trade: the
   reason used to live in a tooltip that grew to fit, and a menu row has neither
   a tooltip nor a `title`, so it goes in the label and the ceiling above is
   measured against it. The fragment is lowercase, which is why it joins with a
   dash rather than as a second sentence. */
export function resumeMenuLabel(reason, { fork = false } = {}) {
  const label = fork ? FORK_LABEL : RESUME_LABEL
  return reason ? `${label} — ${reason}` : label
}

/* And the same fragment as a line of its own, for the opened card, where the
   verbs are controls rather than rows of words and the reason has to stand
   under them.

   The capital and the stop are added here rather than kept in a second table:
   one wording for one refusal, set two ways. Nothing at all when there is no
   reason — the card draws no line rather than an empty one. */
export function resumeReasonLine(reason) {
  if (!reason) return ''
  return `${reason.charAt(0).toUpperCase()}${reason.slice(1)}.`
}

/* The card's whole account of why its buttons are greyed: every distinct reason
   among them, as sentences, in the order the buttons stand in.

   Distinct, which is the only reason this is a function rather than two calls
   in the template. The commonest refusal by far — the worktree is gone — stops
   both verbs in the same words, and saying it twice under two greyed buttons
   would read as two different faults. Where the two answers really do differ,
   which is a harness that reopens a transcript and cannot branch one, both
   lines are drawn, because then there genuinely are two things to say. */
export function resumeReasonLines(...reasons) {
  return [...new Set(reasons.filter(Boolean))].map(resumeReasonLine)
}

/* The nine rows, in Orca's own order and grouping.

   Five groups: the two that start an agent, the one that hands a command over,
   the three that open something somewhere else, the two that copy, and the one
   that destroys. Delete is separated from the rest and drawn in
   `--status-failed-fg` — `ContextMenu` reads `tone: 'danger'` and reaches for
   that token itself, which is why no hex appears here or there.

   The glyphs. `play` for the resume, which is the mark this app already uses
   for the one other row that sets an agent working — a task card's Run.
   `git-fork` for the row under it, which is what that verb does to a
   transcript: the history is read to the point it had reached and what is
   written after it goes down a second path, leaving the first where it was.
   Their neighbour `terminal` is the resume *command*, because what that one
   copies is a line for a shell and nothing else in this app means that; it sits
   in a group of its own for the same reason it draws a different glyph, those
   two starting something here and this one handing it over. `external-link` for
   Open log, because the file leaves this window entirely — it goes to whatever
   the desktop has registered for it — and that glyph is already the About tab's
   mark for the same promise. `folder-open` for Reveal, taken from the file
   tree's own menu rather than chosen again, so the one verb this app has for
   "show me where this is" looks the same in both places; the plain `folder` for
   Open working directory beside it, which is the pair a person tells apart by
   the second word of the label rather than by hue. `copy` twice, since the two
   copying rows differ only in what they copy and the label is where that is
   said.

   `busy` greys the lot while a delete is in flight, the same freeze
   `taskMenu.js` applies for a bd write: a live menu during that second invites
   a second choice racing the first.

   `resume` and `fork` are `resumeAvailability`'s two answers, computed by
   whoever draws the rows — this file stays pure and knows nothing about which
   agent a project is set to. Both default to "yes, with no reason", so a caller
   that has not asked still gets live rows rather than dead ones it cannot
   explain. */
export function sessionMenuItems({
  busy = false,
  userAgent = '',
  resume = { available: true, reason: null },
  fork = { available: true, reason: null }
} = {}) {
  const frozen = Boolean(busy)
  return [
    {
      kind: RESUME_KIND,
      label: resumeMenuLabel(resume?.reason),
      icon: 'play',
      disabled: frozen || resume?.available === false
    },
    {
      kind: FORK_KIND,
      label: resumeMenuLabel(fork?.reason, { fork: true }),
      icon: 'git-fork',
      disabled: frozen || fork?.available === false
    },
    { type: 'separator' },
    { kind: 'copy-resume', label: 'Copy resume command', icon: 'terminal', disabled: frozen },
    { type: 'separator' },
    { kind: 'open-log', label: 'Open log', icon: 'external-link', disabled: frozen },
    {
      kind: 'reveal-log',
      label: `Reveal log in ${fileManagerName(userAgent)}`,
      icon: 'folder-open',
      disabled: frozen
    },
    { kind: 'open-cwd', label: 'Open working directory', icon: 'folder', disabled: frozen },
    { type: 'separator' },
    { kind: 'copy-id', label: 'Copy session id', icon: 'copy', disabled: frozen },
    { kind: 'copy-path', label: 'Copy log path', icon: 'copy', disabled: frozen },
    { type: 'separator' },
    { kind: 'delete', label: 'Delete', icon: 'trash-2', tone: 'danger', disabled: frozen }
  ]
}

/* A single-quoted shell word, for the one place this app composes a command
   line rather than running one.

   POSIX quoting: inside single quotes every character is literal, and the only
   thing that cannot appear there is a single quote itself — which is closed,
   escaped and reopened, the `'\''` every shell manual writes out. A path with a
   space in it is the ordinary case this exists for; a path with an apostrophe
   in it is the case that would otherwise hand somebody a line their shell tears
   in half.

   Always quoted, never conditionally. A rule that quoted "only when it has to"
   is a second rule to be wrong about, and the quotes cost a reader nothing. */
function shellWord(text) {
  return `'${String(text).split("'").join("'\\''")}'`
}

/* The line somebody pastes into their own terminal to pick this session up
   again.

   The working directory is in it, and that is not decoration: `claude --resume`
   resolves a session id against the directory it is run in, so the same id in
   another folder is a session Claude Code has never heard of. `cd` and `&&`
   rather than two lines, because what a person does with this is paste it once.

   A session with no working directory recorded gets the bare resume — wrong to
   invent a folder for it, and the id is still the useful half. */
export function resumeCommand(session) {
  const id = session?.id
  if (!id) return ''
  const cwd = session?.cwd
  const resume = `claude --resume ${shellWord(id)}`
  return cwd ? `cd ${shellWord(cwd)} && ${resume}` : resume
}

/* What each copying verb puts on the clipboard, and what it is called in the
   confirmation afterwards.

   One table rather than a branch at the call site, because the two facts about
   one verb — the text and the noun — are one decision and would otherwise sit
   in two files: `DesktopApp.vue` would hold the noun, and no test can read it.

   Null-prototype, for `copyId.js`'s reason: the fall-back below is `??`, so an
   inherited key would be answered rather than fallen back on — `constructor`
   would come back as a function and `__proto__` as an object. Nothing on screen
   can reach those names, and a contract stated about *anything* with four holes
   in it is worse than no contract. */
const COPY_VERBS = Object.assign(Object.create(null), {
  'copy-resume': { noun: 'resume command', text: resumeCommand },
  'copy-id': { noun: 'session id', text: (session) => session?.id ?? '' },
  'copy-path': { noun: 'log path', text: (session) => session?.path ?? '' }
})

/** Whether this menu kind is one of the three that copy. */
export function isCopyKind(kind) {
  return Boolean(COPY_VERBS[kind])
}

/** What that kind copies out of this session, or `''` when there is nothing. */
export function copyPayload(kind, session) {
  return COPY_VERBS[kind]?.text(session) ?? ''
}

/** And what it is called, for the sentence that confirms it. */
export function copyNoun(kind) {
  return COPY_VERBS[kind]?.noun ?? ''
}

/* What the row's menu button says once something has been copied from it.

   `copyId.js` is the precedent and this is the same three-value vocabulary —
   `''`, `copied`, `failed` — with the same reasoning: the confirmation belongs
   on the control somebody is still looking at rather than in a corner of the
   screen, because a copy is the one action with nothing on screen to show for
   it. What differs is where it lands. A task's id is a control of its own and
   says so in its own tooltip; a session's copy is picked from a menu that
   closes on the way out, so the trigger the menu hung from is the only thing
   left to answer on. It carries the outcome for `COPIED_MS` and then goes back
   to being a menu button.

   The noun is in the sentence because three different verbs land here and
   "Copied" alone would leave a person unsure which of the three they pressed.

   Anything this function has never heard of falls to the invitation, which is
   the row's ordinary label and is never wrong. */
export function menuButtonLabel(state, noun = '') {
  if (state === 'copied') return noun ? `Copied the ${noun}` : 'Copied'
  if (state === 'failed') return noun ? `Could not copy the ${noun}` : 'Could not copy'
  return 'Session actions'
}

/* And which glyph it draws while it says so. The tick is the confirmation a
   person reads without stopping; the cross is the refusal. Neither is a colour
   change: `IconButton` has one colour for its glyph and this system does not
   spend a status hue on a menu button.

   The ordinary glyph is the board's own overflow mark, so the control reads as
   a menu before it reads as anything else. */
export function menuButtonIcon(state) {
  if (state === 'copied') return 'check'
  if (state === 'failed') return 'x'
  return 'ellipsis'
}

/* The caption of the window that asks before a transcript is deleted.

   One copy, called by both the component's own heading and the announcement
   `DesktopApp.vue` hands to `set_title` — the shape `promoteTitle.js` set and
   the one the hazards list says to copy, so the OS frame and the body cannot
   come to say different things. The mock's fixture in `stores/mockBackend.js`
   is the one literal that still stands apart from it, as it does for every
   dialog in this app.

   The id is not in it. A session id is a 36-character UUID and a heading is not
   where a person recognises one: what they check before pressing Delete is the
   sentence they opened the session with, which is the body's first line, and
   the id, the path and the size are named underneath it in mono. */
export const DELETE_SESSION_TITLE = 'Delete this session?'

/* What the dialog says will happen. The consequence rather than an apology,
   which is what this system asks a destructive confirm to say — and here the
   first clause is the one that matters: this is not a file the app made, and it
   is not in anybody's repository, so nothing else knows it existed. */
export const DELETE_SESSION_DESCRIPTION =
  'The transcript is deleted from disk outright. It is not in a repository and it does not go to the trash, so there is no undo and nothing to restore it from. The conversation itself is gone with it.'

/* The three facts the dialog names about what is about to go, in the order a
   person checks them: which session, which file, how much of it.

   A list of pairs rather than a sentence, because all three are identifiers or
   measurements and the component sets them in mono beside sans labels — the
   project's own rule, and the reason the pieces arrive tagged rather than
   joined into one string.

   The size comes off the record the worker sent rather than being asked for
   here: the list has already `stat`ed the file, and a dialog that read the disk
   again could answer nothing at all for the one case it most needs to draw — a
   transcript that has gone since the list was built. `formatBytes` answers `—`
   for a record that carries no size, which is the honest reading of a field
   this build was not told. */
export function deleteSessionFacts(session) {
  return [
    { label: 'Session id', value: session?.id ?? '' },
    { label: 'Log path', value: session?.path ?? '' },
    { label: 'Size', value: formatBytes(session?.size) }
  ]
}
