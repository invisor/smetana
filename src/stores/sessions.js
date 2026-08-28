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
