/* How the run bar draws an ending: the sentence, the glyph and the tone.

   Pure and outside the component for the reason `branchChoice.js` next to it
   is: a `.vue` file is the one thing no test in this repository can reach, and
   a table saying which endings are painted as failures is exactly the sort of
   thing that goes wrong quietly.

   Each ending names the tone it wants. It used to carry a `loud` flag instead,
   and that flag meant two things at once — be prominent, and it failed — which
   was true only while every loud ending was a failure (smetana-e3o).
   `needs_answer` is not one: nothing fell over, the agent asked something and
   an unattended run had nobody in it to answer. Drawn in `--status-failed-fg`
   it was literally the same colour as `crashed` and `no_progress`, three bars
   within two lines of each other and indistinguishable. Naming the tone per
   ending is also what stops the next loud-but-not-a-failure one inheriting red
   by omission. */
export const TONE = {
  /* Over, and there is nothing here for anybody to do. */
  quiet: 'var(--text-secondary)',
  /* Something fell over, or stopped moving, or would not start. */
  failed: 'var(--status-failed-fg)',
  /* Somebody is being waited on. The status system's own colour for that
     thought, so this bar and the agent row a few centimetres away do not
     disagree about what "waiting for a person" looks like. */
  needsYou: 'var(--status-needs-you-fg)'
}

/* The glyph for an ending that has no claim of its own to make — a filled
   square, the same shape the stop button carries.

   It is written here, once, and every ending that wants it names it: an entry
   without an `icon` used to leave each caller to supply the default, and by the
   time there were two callers the bar and the bell each held their own copy of
   `?? 'square'`. Seven of the ten endings below land on it, so that was the
   glyph most runs actually draw, kept in two places, with the failure being the
   bar and the card a centimetre apart describing one run differently and
   nothing anywhere going red. */
const NEUTRAL = 'square'

/* Loud only where a person has to do something. A run that finished its queue
   is the ordinary ending and gets the quiet treatment; one that stopped because
   nothing moved, or because the harness kept failing, is the reason this bar
   is worth a colour at all.

   Every entry names both its tone and its glyph, for the same reason the tone
   note above gives: an ending added without one would take whatever default the
   last caller happened to write, which is exactly the drift this table exists
   to prevent. */
export const REASONS = {
  queue_empty: { text: 'Done — nothing left to take', tone: TONE.quiet, icon: 'check' },
  /* A Crew run's own ending: it takes one batch, merges it, and that is the
     end. Not `queue_empty`'s sentence, because tasks may well still sit in
     Ready — the run finished its batch, not the board's work. Quiet like it,
     though: this is the mode doing exactly what it said. */
  batch_done: { text: 'Done — the batch is finished', tone: TONE.quiet, icon: 'check' },
  cancelled: { text: 'Stopped', tone: TONE.quiet, icon: NEUTRAL },
  /* Quiet, like the stop button and for the same reason: a person did this on
     purpose and there is nothing here to fix. Loudness is not what it owes
     them — the sentence is. "Mid-batch" is the whole of it: a stop lets the
     batch in flight finish, while removing the session killed it where it
     stood, so there are worktrees left half-done for the next run's recovery
     phase to pick up. The person reading this line is deciding whether to go
     and look, and nothing else on screen will tell them to.

     `bare` keeps the branch suffix off this one line: "…was removed into
     staging" is a garden path, and the branch is the least of what somebody
     needs at that moment. */
  session_removed: {
    text: 'Stopped mid-batch — its agent session was removed',
    tone: TONE.quiet,
    icon: NEUTRAL,
    bare: true
  },
  /* The agent asked something and the run had nobody in it to answer — a
     Codex trust prompt in a folder it has not seen before is the case this
     was written for. Loud, because it is waiting on a person and nothing
     else on this bar is, and in the colour the rest of the app gives that
     state rather than the failure colour: this ending is the one place a run
     stops with nothing broken behind it. The detail line carries the question
     itself: what it asked is what decides whether somebody goes and answers
     it, and it is answered in the agent's own terminal, where the session is
     still sitting at the prompt. */
  needs_answer: {
    text: 'Stopped — the agent is waiting for an answer',
    tone: TONE.needsYou,
    icon: 'message-circle-question-mark'
  },
  no_progress: { text: 'Stuck — a whole batch changed nothing', tone: TONE.failed, icon: NEUTRAL },
  max_iterations: { text: 'Stopped after too many batches', tone: TONE.failed, icon: NEUTRAL },
  unreadable: { text: 'Stopped — the tracker could not be read', tone: TONE.failed, icon: NEUTRAL },
  crashed: { text: 'Stopped — the agent kept failing', tone: TONE.failed, icon: NEUTRAL },
  /* Beside `crashed` because it is the same class of trouble — the agent, not
     the board — and deliberately worded away from `no_progress` two lines up.
     The two are the pair the worker's own `StopReason::NothingDone` spends a
     paragraph keeping apart: `no_progress` is a batch that *ran* and left the
     board stuck, which sends somebody to the tracker, while this one is a
     session that came back having done nothing at all, which sends them to the
     agent. Drawn as two near-identical sentences in one colour they would be
     the failure `smetana-e3o` above records, arrived at from the other end.
     The count of those batches is the second line's, through
     `endingDetail`. */
  nothing_done: {
    text: 'Stopped — the agent came back having done nothing',
    tone: TONE.failed,
    icon: NEUTRAL
  },
  preflight: { text: 'Could not start', tone: TONE.failed, icon: NEUTRAL }
}

/* How many batches did nothing, as the second line says it.

   One batch is an ordinary sentence rather than "1 batch in a row", because a
   run allowed a single batch stops on its first empty one — `once` in
   `service.rs` — and a count of one presented as a streak reads as a threshold
   nobody reached. */
function emptyBatches(batches) {
  if (typeof batches !== 'number' || batches < 1) return ''
  return batches === 1
    ? 'one batch, and it did nothing at all'
    : `${batches} batches in a row, none of which did anything`
}

/* The second line under an ending: what the worker said about it, or failing
   that the branch the run was aimed at.

   Here rather than in the component because it is the same class of thing the
   table above is — an ending that has something to say and does not say it goes
   wrong quietly. It did: `StopReason::Preflight` carries a `detail` naming what
   would not come up, and the bar read only `question`, so a project whose
   `docker` was not on the app's `PATH` stopped with "Could not start into
   develop" — a sentence pointing at the target branch, which had nothing to do
   with it, while `sh: docker: command not found` had crossed the wire and was
   drawn nowhere.

   The order is what each ending has to say for itself. `question` first: an
   ending that has one is the agent waiting, and what it asked is what decides
   whether somebody goes and answers it. `detail` next, which is the two
   endings that could not start at all — the project that would not come up, and
   the batch that could not be spawned, since `service.rs` reports both this way.
   The empty batches next, which is the one ending whose payload is a number:
   without it `nothing_done` fell through to the branch and said "…having done
   nothing into main", the same garden path the `preflight` defect above made.
   The branch last, for every ending with nothing of its own to add. */
export function endingDetail(reason, branch = '') {
  return reason?.question || reason?.detail || emptyBatches(reason?.batches) || branch
}

export function stopReason(kind) {
  /* An unknown reason is an ordinary outcome, not a crash: this front end may
     be older than the worker. It says so plainly rather than drawing nothing.
     Its tone is the one it has always had, now written down rather than
     inherited from a flag — an ending this build cannot name is at least not
     `queue_empty`, which it does know, so the run stopped short of its queue
     and somebody has to go and read why. Its glyph is the neutral one, and it
     is here rather than left to the caller for the same reason every entry
     above names its own: what this function answers is complete, so nothing
     downstream has a default of its own to keep in step. */
  return (
    REASONS[kind] ?? {
      text: kind ? `Stopped — ${kind.replace(/_/g, ' ')}` : 'Stopped',
      tone: TONE.failed,
      icon: NEUTRAL
    }
  )
}
