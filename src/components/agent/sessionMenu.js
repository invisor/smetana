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

   Two of the nine verbs Orca offers on a session are deliberately absent:
   Resume in worktree and Continue in a new session. Those are not about the
   file at all — they bring a live agent back — and they are a task of their
   own. Copy resume command is the one that stands in for them here, and it is
   also the answer for somebody who would rather paste it into their own
   terminal than have this app spawn anything. */
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

   Measured against the longest label this file can produce, which is
   `Reveal log in file manager` — the Linux and BSD fall-back, longer than
   either "Finder" or "Explorer". `ContextMenu` spends 70px on chrome before the
   label (see `MENU_W` in `kanban/taskMenu.js`, which works the number out), so
   240 leaves 170px for a 26-character label at `--text-sm`. Nowhere near
   `taskMenu.js`'s 424: nothing here interpolates a sentence, and every row is a
   verb of two or three words. */
export const SESSION_MENU_W = 240

/* The seven rows, in Orca's own order and grouping less the two launching verbs
   that are not this task's.

   Four groups: the one that hands a command over, the three that open something
   somewhere else, the two that copy, and the one that destroys. Delete is
   separated from the rest and drawn in `--status-failed-fg` — `ContextMenu`
   reads `tone: 'danger'` and reaches for that token itself, which is why no hex
   appears here or there.

   The glyphs. `terminal` for the resume command, because what is copied is a
   line for a shell and nothing else in this app means that. `external-link` for
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
   a second choice racing the first. */
export function sessionMenuItems({ busy = false, userAgent = '' } = {}) {
  const frozen = Boolean(busy)
  return [
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
