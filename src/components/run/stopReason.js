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

/* Loud only where a person has to do something. A run that finished its queue
   is the ordinary ending and gets the quiet treatment; one that stopped because
   nothing moved, or because the harness kept failing, is the reason this bar
   is worth a colour at all. */
export const REASONS = {
  queue_empty: { text: 'Done — nothing left to take', tone: TONE.quiet, icon: 'check' },
  cancelled: { text: 'Stopped', tone: TONE.quiet },
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
  no_progress: { text: 'Stuck — a whole batch changed nothing', tone: TONE.failed },
  max_iterations: { text: 'Stopped after too many batches', tone: TONE.failed },
  unreadable: { text: 'Stopped — the tracker could not be read', tone: TONE.failed },
  crashed: { text: 'Stopped — the agent kept failing', tone: TONE.failed },
  preflight: { text: 'Could not start', tone: TONE.failed }
}

export function stopReason(kind) {
  /* An unknown reason is an ordinary outcome, not a crash: this front end may
     be older than the worker. It says so plainly rather than drawing nothing.
     Its tone is the one it has always had, now written down rather than
     inherited from a flag — an ending this build cannot name is at least not
     `queue_empty`, which it does know, so the run stopped short of its queue
     and somebody has to go and read why. */
  return (
    REASONS[kind] ?? {
      text: kind ? `Stopped — ${kind.replace(/_/g, ' ')}` : 'Stopped',
      tone: TONE.failed
    }
  )
}
