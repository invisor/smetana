/* What the subscription block on the Agents tab says, and nothing about how it
   looks.

   Another of the `branchChoice.js` family, and a neighbour of `storage.js` in
   both senses: the whole of a rule, pure, with no Vue and no DOM in it, living
   under the part of the interface it is a rule about. The rule here is words —
   which of five sentences belongs under a heading, and how a percentage and
   somebody else's phrase about a reset become one line. It is out here because
   a `.vue` file is the one thing no test in this repository can reach, and the
   sentence that tells a person their allowance could not be read has to be the
   one that is drawn when it could not.

   The answer comes from `agent_usage` whole, in Rust's own shape — the way
   `runs.js` keeps its `config` and `storage.js` keeps its survey. Nothing here
   unpacks it into flags, and a state this build has not heard of must not
   silently read as one it has: it reads as unreadable, the safe direction,
   since that sentence promises nothing about the allowance.

   **No threshold is written here.** Which band a reading falls in is decided in
   `src-tauri/src/runs/usage.rs`, by the same `decide` the run gate uses, and
   travels in the answer as a name. The two thresholds that produced it are
   spelled nowhere in this directory: a second copy of them here would be one
   that drifts from the first with nothing on screen to say it has. */

/* The three states of `runs::usage::AgentUsage`, as the tag serde writes. */
const READ = 'read'
const UNSUPPORTED = 'unsupported'

/* `N% used · resets Aug 7 at 8pm (Europe/Moscow)`, or the percentage alone.
   The reset is the harness's own words, passed through untouched — the same
   string Rust deliberately never turns into a moment in time — and its absence
   is an ordinary reading rather than a gap: a fresh allowance prints no reset
   at all, and inventing one would be worse than the missing half of a sentence.

   `null` when the percentage is not a number — a half Rust could not read
   travels as an explicit `null` under its own key, and a build newer than this
   one could put anything there. It never becomes a zero: "0% used" is a fact
   about an allowance, and drawing it over the absence of one is the mistake the
   placeholder block was replaced for. The reverse is as carefully not done:
   `0` is a number, a fresh week really does print it, and it draws. */
function limitLine(pct, resets) {
  if (!Number.isFinite(pct)) return null
  const used = `${pct}% used`
  const when = typeof resets === 'string' ? resets.trim() : ''
  return when ? `${used} · resets ${when}` : used
}

/* The rows of a reading: the halves that arrived, in the order they are drawn,
   and none at all for anything that is not a reading. Session first — it is the
   sooner of the two limits and therefore the more useful one to read.

   **Half a reading is a reading.** Either of the two lines the harness prints
   can go missing — one of them reworded, a build that prints the other alone —
   and what comes across then is one percentage and no second one. The half
   that was read is shown; the half that was not is not drawn at all, and never
   as a zero. Both directions of that are mistakes with a person on the other
   end: an invented "This week: 0% used" is a quota the app never read, and
   refusing the pair over it would have thrown away the half it did read and
   said the allowance could not be read, which sends somebody off to check a
   login that is fine (smetana-7rp). */
export function usageLines(answer) {
  if (answer?.state !== READ) return []
  return [
    { name: 'Session', value: limitLine(answer.usage?.sessionPct, answer.usage?.sessionReset) },
    { name: 'This week', value: limitLine(answer.usage?.weekPct, answer.usage?.weekReset) }
  ].filter((row) => row.value)
}

/* Which agent the answer is about — whoever actually answered the probe, which
   need not be the one showing in the dropdown above: `agents::pick` substitutes
   the first installed profile for a configured one that is not on `PATH`, so a
   block headed "Claude Code subscription" can be about Codex.

   `null` is a real answer twice over: nothing has been read yet, and nothing on
   this machine could be asked. Neither has an agent to name, and naming the
   selected one instead would be the app claiming a reading it does not have. */
export function agentOf(answer) {
  const agent = answer?.agent
  return typeof agent === 'string' && agent ? agent : null
}

/* What a run would do at this reading, in the run's own terms rather than in
   percentages — the numbers are already above, and what a person wants from
   them is what happens next. The three are `runs::usage::Decision`'s bands,
   named by Rust and never worked out here. */
const BAND_NOTE = {
  normal: 'A run would take a full batch at this level.',
  reduced: 'A run would take fewer tasks per batch at this level.',
  pause: 'A run would take no new work at this level and wait for the reset.'
}

/* The one sentence under the rows. Every state has one, and none of them is
   silent about why there is nothing to show — which is also why the choice
   among them is here rather than in the component: this file exists so that the
   sentence telling somebody their allowance could not be read is the one drawn
   when it could not.

   `error` is the channel rather than the answer — the command is infallible in
   Rust, so a refusal is `invoke` itself failing — and it comes first because
   the caller draws it as a line of its own. There is nothing to add under it:
   the reading is cleared before every read, so the honest sentence for that
   moment would be "not read yet", which the refusal directly contradicts. Not
   even the busy line, on the same ground — a caller showing both is describing
   one attempt twice, and the refusal is the account of it.

   `busy` comes next and beats whatever is on screen: a probe is somebody else's
   CLI with a minute's ceiling over it, and a block that sat there showing the
   previous answer would be claiming a reading that is being replaced as it is
   read. */
export function usageNote(answer, busy = false, error = null) {
  if (error) return ''
  if (busy) return 'Reading what is left of the allowance…'
  if (!answer) return 'The allowance has not been read yet.'
  if (answer.state === UNSUPPORTED) {
    return agentOf(answer)
      ? 'This agent does not report what is left of its subscription, so there is nothing to read here.'
      : 'No agent is installed on this machine, so there is nothing to ask.'
  }
  if (usageLines(answer).length) {
    /* A band this build has never heard of says nothing about a run rather
       than guessing which of the three it meant — the block still shows the
       percentages, which are the part that does not depend on knowing.

       The band is Rust's word about the whole reading, half of one included:
       `decide` there works off the halves it has, so the sentence under one row
       is as true as the one under two. */
    return BAND_NOTE[answer.band] ?? ''
  }
  /* Nothing was drawn above, which is a `read` answer with neither half in it
     — Rust does not send one, and a build that did would be one this cannot
     draw — or a state this build has never heard of. Both take the sentence
     that promises nothing about the allowance. */
  return 'The allowance could not be read. The agent may not be installed on this machine, or not signed in.'
}

/* Whether to offer the button at all. A press has to be able to do something:
   an agent that does not answer this question will not answer it a second time
   later, so the block says so and offers nothing to press.

   A machine with no agent at all keeps the button, and the difference is what
   a person can do about each: installing one is the fix, and there would then
   be something new to ask. Everything else — a reading, a failed probe, an
   answer still on its way — keeps it too, and `busy` is what disables it while
   one press is still out. */
export function offersRefresh(answer) {
  return !(answer?.state === UNSUPPORTED && Boolean(agentOf(answer)))
}
