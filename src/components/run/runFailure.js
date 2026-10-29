/* What a refused run start is told to the person as. The whole of the rule, pure, so the
   desktop's run dialog and the phone's Run screen say the same thing about the same refusal
   instead of keeping two copies of it.

   Rust's own words are used where they exist, since they are already written for a person:
   the broken-config one names the section that will not parse, and that is the whole of why it
   is worth showing rather than replacing with something of our own. `RunError` arrives as
   `{ kind, detail }`; a plain string or an `Error` is read too. */
export const runFailure = (err) => {
  /* `String(undefined)` is a truthy 'undefined', which would be shown as the reason. */
  const detail = err == null ? '' : (err.detail ?? err.message ?? String(err))
  if (err?.kind === 'not_configured') return 'This project has no run configuration yet.'
  /* Named, because "a run is already going" stopped being an answer when a
     project could hold several: the person has to hear which of them is in the
     way. The fragment is the worker's own (RunScope::describe), riding in the
     error's detail. */
  if (err?.kind === 'already_running') {
    return `A run over ${err.detail?.scope ?? 'this scope'} is already going in this project.`
  }
  /* Deliberately not the sentence above. The run this one is about has stopped
     — the bar says so in the same breath — and only its loop is still winding
     down, so claiming a run is going would read as the stop not having taken. */
  if (err?.kind === 'winding_down') return 'The previous run is still finishing. Try again in a moment.'
  return detail || 'The run could not be started.'
}
