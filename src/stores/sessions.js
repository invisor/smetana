/* The Claude Code sessions this project already has on disk — the right
   column's Sessions tab. One of the files in this directory that know Tauri
   exists; components see a reactive object and nothing else, which is why this
   is a store rather than a helper next to the component that draws it.

   Not `terminals.js`, deliberately, and the distinction is the whole task. That
   store holds the live PTY sessions of *this run of the app*: they are started
   here, they stream, they end, and they are drawn in the left column's Agents
   tab. This one holds a history nothing in this app created — the transcripts
   Claude Code writes under `~/.claude/projects`, which outlive every run of
   this window and mostly belong to sessions started from a terminal. A freshly
   launched app has no live sessions and hundreds of these.

   Read-only, and it stays that way: the worker reads the files, this asks for
   the list, and nothing in this app writes a transcript.

   No watcher, on purpose. The directory this reads is 844 MB for one project on
   the machine this was written against, and every live session appends to it —
   watching that is a subsystem with its own lifecycle and its own error
   reporting, while the read itself takes a fraction of a second. The list is
   refreshed on the tab being opened and on the project changing, which is the
   same answer `files.js` and `git.js` give and for the same reason. */
import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export const sessionsState = reactive({
  /* Newest first — the opposite of `agentRows`, and right here: this list is
     historical, "recent sessions" reads literally, and there is no second copy
     of it on screen in the other order to disagree with. */
  sessions: [],
  /* Whose sessions these are, and the project of the last call — one field
     serving both, since a call always claims the list it is about to replace.
     In the state rather than module-private because the panel has to be able to
     tell "this project has none" from "nobody has asked yet". */
  project: null,
  /* True between the call and its answer. What it buys is one thing: the empty
     state is a sentence claiming the disk holds nothing, and drawing it for the
     fraction of a second before the first answer would be a claim made before
     anybody looked. */
  loading: false,
  /* The clock every row's "18h ago" is measured against. It lives here rather
     than in the component so that one interval serves the whole column and
     every row of it moves at the same moment.

     A minute is the finest step any label on a row has, so a minute is the
     tick. Started lazily from initSessions(), never at module scope: the module
     loads once for a window's lifetime in the app, but the test harness rebuilds
     the graph per test, and an interval nobody clears would outlive every test
     that started one. */
  now: Date.now()
})

let clockStarted = false

export function initSessions() {
  if (clockStarted) return
  clockStarted = true
  setInterval(() => (sessionsState.now = Date.now()), 60000)
}

/* By the transcript's mtime, newest first. Parsed rather than compared as
   strings: the field is RFC 3339 and two of them are only comparable
   lexicographically while they agree on precision and offset, which is a
   property of the writer rather than of the format. An unparseable date sorts
   as 0 and lands at the bottom, which is where a record nobody can date
   belongs. */
function newestFirst(a, b) {
  const left = Date.parse(a?.modifiedAt)
  const right = Date.parse(b?.modifiedAt)
  return (Number.isFinite(right) ? right : 0) - (Number.isFinite(left) ? left : 0)
}

/* The project's sessions, read again.

   Two rules, both borrowed from `git.js` because they are the same two rules.
   A list belonging to *another* project goes the moment this one is asked
   about: sessions of a repository somebody has already left, under the name of
   the one they are looking at, would be the worst answer this panel can give.
   A list belonging to *this* project is left alone while it is read again, so
   re-opening the tab does not blink the column empty and back.

   And the guard against its own stale answer: two calls can be in flight with
   no ordering guarantee on which invoke resolves first, so the last *call*
   wins rather than the last answer.

   There is no error state to draw and that is the contract rather than an
   oversight: `sessions_list` answers a missing `~/.claude/projects`, a
   directory it cannot read and a corrupt transcript with fewer rows, never with
   a refusal. Reaching the catch means the call itself failed — the console gets
   it, the column shows the empty state, and nothing is invented.

   `loadSessionHistory` and not `loadSessions`, which is the name it would have
   had on its own: `terminals.js` already exports that one for the live PTY
   sessions of the current run, `DesktopApp.vue` imports it, and two functions
   sharing a name while meaning two different lists is exactly the pair a later
   reader calls the wrong half of. "History" is the difference between the two
   stores, so it is the word in the name. */
export async function loadSessionHistory(project) {
  if (sessionsState.project !== project) sessionsState.sessions = []
  sessionsState.project = project
  if (!project) {
    sessionsState.loading = false
    return
  }
  sessionsState.loading = true
  try {
    const rows = await invoke('sessions_list', { project })
    if (sessionsState.project !== project) return
    sessionsState.sessions = [...rows].sort(newestFirst)
  } catch (err) {
    if (sessionsState.project !== project) return
    console.error('[sessions] listing failed:', err)
    sessionsState.sessions = []
  } finally {
    if (sessionsState.project === project) sessionsState.loading = false
  }
}

/* ---- the two verbs a session row's menu has that are not the clipboard ---- */

/* Whatever a command refused with, as a sentence.
 *
 * `sessions_open` and `sessions_delete` reject with a `String` written for a
 * person — `sessions/act.rs` composes them and its header says why they are
 * sentences rather than codes. What arrives at this side of the wire is that
 * string for a refusal Rust made, and something else entirely when the call
 * itself failed: a browser's mock rejects with an `Error`, and a channel that
 * broke rejects with whatever Tauri felt like. So the string is taken when
 * there is one and the rest is turned into words, because the caller puts this
 * straight on the screen and `[object Object]` is not an explanation. */
function refusalText(err) {
  if (typeof err === 'string' && err) return err
  if (err && typeof err.message === 'string' && err.message) return err.message
  return 'Something went wrong and nothing was said about it.'
}

/* One guarded verb of the worker's, and whatever it refused with.
 *
 * Written once for the three: they differ only in the command name, and three
 * copies of the same try/catch would be three places for the console line and
 * the fall-back sentence to drift apart.
 *
 * Answered with `null` when it worked and with a sentence when it did not,
 * rather than throwing. Every caller puts the failure on the screen either way,
 * and a `try` around one line at each call site is a shape that goes wrong the
 * first time somebody adds a fourth. */
async function askWorker(command, path, missing) {
  if (!path) return missing
  try {
    await invoke(command, { path })
    return null
  } catch (err) {
    console.error(`[sessions] ${command} was refused:`, err)
    return refusalText(err)
  }
}

/* A session's transcript, handed to whatever the desktop has registered for it.
 *
 * Here rather than in `app.js`, which is where `openExternal` and
 * `revealInFileManager` live, and the line between the two is what the desktop
 * is being asked about. Those two are asks with no subject — any URL, any path
 * — and go through the opener plugin from this side of the wire. These three
 * are commands of ours, because the plugin's `open_path` is refused by its own
 * scope check unless a capability entry allows the path, and the only entry
 * wide enough for both a transcript under `~/.claude/projects` and the
 * arbitrary folder a session ran in is one that allows every path on the
 * machine. `sessions/act.rs` carries the whole argument; what it means here is
 * that the guard lives in Rust and speaks the sessions vocabulary, so the calls
 * belong to this store. */
export async function openSessionLog(path) {
  return askWorker('sessions_open_log', path, 'There is no transcript to open.')
}

export async function openSessionDirectory(path) {
  return askWorker('sessions_open_cwd', path, 'This session recorded no working directory.')
}

/* Showing the transcript in the platform's file manager.
 *
 * **Not `revealInFileManager` in `app.js`, and this one is not about a
 * capability.** That function is the file tree's: it answers a boolean, so it
 * cannot say *why*, and the single sentence its callers have for a `false` is
 * about a browser having no file manager to ask. In the built app the
 * commonest way this fails is a transcript that has gone since the list was
 * read — the plugin canonicalises the path before showing anything — and that
 * person would have been told to go and install the desktop app they are
 * already running. A command of ours answers with the same words the other
 * three do. */
export async function revealSessionLog(path) {
  return askWorker('sessions_reveal', path, 'There is no transcript to show.')
}

/* One transcript, deleted, and the row taken out of the list.
 *
 * The confirmation is the caller's — a dialog window of its own, which names
 * the id, the path and the size — and this function is deliberately not where
 * it is asked: a store that put a question on the screen would be a store that
 * draws. What is here is the write and the one consequence of it that this list
 * owns.
 *
 * The row goes only on a success. A refusal leaves the list exactly as it was,
 * including the ordinary refusal — a transcript that had already gone — and
 * that is the honest reading rather than a tidy one: this store has not read
 * the disk since the tab was opened, so the only thing it can say about that
 * row is what the command just told it, and the sentence is what says it. The
 * list is read again when the tab is next opened.
 *
 * Spliced rather than filtered into a new array so that nothing else about the
 * list moves — a fresh array would be a new identity for every row and would
 * take every open card in the column with it. */
export async function deleteSessionTranscript(path) {
  if (!path) return 'There is no transcript to delete.'
  try {
    await invoke('sessions_delete', { path })
  } catch (err) {
    console.error('[sessions] the delete was refused:', err)
    return refusalText(err)
  }
  const at = sessionsState.sessions.findIndex((session) => session.path === path)
  if (at >= 0) sessionsState.sessions.splice(at, 1)
  return null
}
