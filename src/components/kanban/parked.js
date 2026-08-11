/* What a parked task is, and what is still unanswered on it.

   The `branchChoice.js` / `columnOrder.js` / `taskMenu.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — no test in
   this repository can reach a `.vue`, so a rule left inside a component is a
   rule nothing checks.

   Three readers, which is why it is not folded into `taskMenu.js`: the menu
   asks whether to offer the row, `DesktopApp` asks whether a status write needs
   a question first, and the dialog asks what the questions actually were. */

/* bd's own strings, and this is the front end's only copy of either.

   `parked` is a custom status rather than one of bd's eleven — the project
   creates it with `bd config set status.custom` — so nothing in bd guarantees
   it. It is nonetheless a settled word here: Rust writes it (`runs::queue::PARKED`),
   the `running-tasks` skill tells a lead to write it, and `status/status.js`
   already gives it a glyph. The copy across the language boundary is the same
   one `EXPECTED_BD_VERSION` keeps, and it fails the same quiet way: a rename on
   one side leaves a card that no longer offers to resolve anything, with
   nothing on screen to say so.

   `open` is bd's word for what the board calls Ready, and it is the value the
   status picker sends. */
export const PARKED = 'parked'
export const READY = 'open'

export const isParked = (bdStatus) => bdStatus === PARKED

/* The prefix both writers use. `runs::queue::parking_note` formats exactly
   this, and `smetana:running-tasks` tells a lead by hand to write the same
   thing, so one vocabulary reaches the notes whoever did the parking. */
const PARKED_LINE = /^\s*parked:\s*(.+)$/i
const RESOLVED_LINE = /^\s*resolved:\s*/i

/* The questions still open on a task, newest parking first in the order the
   notes hold them.

   Everything after the **last** `resolved:` line, and that rule is the whole of
   the pairing. Matching each answer to its own question would need the two to
   be written in step, which nothing enforces — a person answering three
   questions in one sentence writes one `resolved:` line, and a positional pair
   would then declare two of them still open. What is true instead is the
   sequence: a resolving session answers everything open at that moment and
   unparks the task, so a `parked:` line below the last `resolved:` is a
   question asked since, and one above it has been through a session that
   settled it.

   An empty answer is an ordinary outcome rather than a gap: a task parked by
   hand may carry no note at all, and the dialog says so in its own words
   instead of drawing an empty list. */
export function openQuestions(notes) {
  if (!notes) return []
  const lines = String(notes).split('\n')
  let from = 0
  lines.forEach((line, i) => {
    if (RESOLVED_LINE.test(line)) from = i + 1
  })
  return lines
    .slice(from)
    .map((line) => line.match(PARKED_LINE)?.[1]?.trim())
    .filter(Boolean)
}

/* Does this status write need a question asked first?

   Only parked → Ready, and deliberately not parked → anything. Done closes the
   task, which is a decision that the question no longer matters; Pinned takes
   it off the queue entirely. Ready is the one that puts it back where an agent
   will take it, and the next agent to take it meets exactly the question that
   parked it — which is what makes it worth interrupting somebody for. */
export const needsReadyWarning = (bdStatus, next) => isParked(bdStatus) && next === READY
