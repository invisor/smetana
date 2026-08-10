/* What "the same run" means now that a project holds several at once — the
   front half of the rule the worker enforces (runs/service.rs `admit`): a
   second run over the same scope is two leads racing for the same tasks and is
   refused, while a queue run beside a task run, or two runs over different
   epics, divide the board between them.

   Pure and outside the components for the reason branchChoice.js is: a `.vue`
   file is the one thing no test in this repository can reach. It is also read
   by a store — runs.js prunes a replaced run's stopped bar with `sameScope` —
   which is why nothing of Vue or the DOM may ever land here. */

/* `a` and `b` are scope objects of either provenance — the dialog's
   `{ kind, id, title }` or a run's `settings.scope` `{ kind, id }` — and only
   the kind and the id take part, which is what lets the two shapes meet. */
export function sameScope(a, b) {
  if (!a || !b || a.kind !== b.kind) return false
  return a.kind === 'queue' || a.id === b.id
}

/* The scope as a person reads it — the same words the worker's own refusal
   composes (RunScope::describe in runs/model.rs), so whichever side of the
   wire a sentence was made on, it is one vocabulary. */
export function scopeLabel(scope) {
  if (scope?.kind === 'task') return `task ${scope.id}`
  if (scope?.kind === 'epic') return `epic ${scope.id}`
  return 'the queue'
}

/* Why a play aimed at this scope is inactive, or '' when it is not. Lowercase
   because it is never shown on its own — TaskCard interpolates it into
   "Run this — …" and the column header into "Run the queue — …".

   Only a live run blocks. A stopped one is a reason to read the bar, not a
   reason to refuse — and an ending this front end has never heard of is still
   `state.kind === 'stopped'`, so an unknown stop reason cannot leave a play
   greyed over a run that is over. A state kind that is itself unknown reads
   as live — the lenient direction the store's old project-wide check took:
   the worker would refuse the duplicate anyway, with the same sentence. */
export function scopeBusyReason(scope, runs) {
  const busy = (runs ?? []).some(
    (run) => run?.state?.kind !== 'stopped' && sameScope(scope, run?.settings?.scope)
  )
  return busy ? `a run over ${scopeLabel(scope)} is already going` : ''
}
