/* What the footer strip draws out of an `agent_usage` answer, and nothing
   about how it looks.

   Another of the `headline.js` family living beside it: the whole of one rule,
   pure, with no Vue and no DOM in it, under the directory of the part of the
   interface it is a rule about. A `.vue` file is the one thing no test in this
   repository can reach, and the part worth pinning down is exactly the choice
   between a number, a dash and a sentence.

   The answer arrives whole, in Rust's own shape, the way `settings/usage.js`
   takes it — nothing between Rust and the component unpacks it into flags, and
   a state this build has never heard of must read as "nothing was read" rather
   than silently as one this build does know.

   **No threshold is written here.** Which band a reading falls in is decided in
   `src-tauri/src/runs/usage.rs` and travels in the answer as a name. The two
   numbers behind it are spelled nowhere in the front end: a second copy would
   be one that drifts from the first with nothing on screen to say it has.

   This is not an extension of `settings/usage.js`. That module makes the
   sentences of a settings row (`10% used · resets Aug 7 at 8pm`); this one
   makes the segments of a strip (`Session 10%`), and the two agree on little
   beyond "is this a finite number". What it does borrow from there is the two
   functions whose output is a *vocabulary* rather than a layout — `agentOf`,
   which is the question of who answered the probe and has one right answer,
   and `usageNote`, which is the app's one set of words for why there is nothing
   to read. Rewriting those four sentences here would be a second copy of the
   product's vocabulary, free to drift from the first while both are on screen
   in the same session, one window apart. */
import { agentOf, usageNote } from '../settings/usage.js'

/* `runs::usage::AgentUsage`'s reading, as the tag serde writes. The other two
   states are not named here: everything that is not a reading takes the same
   dashes, which is what keeps a state this build has never heard of on the
   safe side of the difference. */
const READ = 'read'

/* What stands for a half that was not read. The same character the scope bar
   above uses for a column with nothing in it. */
const DASH = '—'

/* The one place besides the settings picker that puts a name to an agent id.
   The ids are `agents::IDS` in `src-tauri/src/agents/mod.rs`, which is where
   the truth lives; these are labels for ids Rust already knows, so an id this
   list has not heard of is drawn as it stands rather than dressed up as one of
   ours or hidden. */
const AGENT_LABELS = {
  claude: 'Claude Code',
  codex: 'Codex'
}

/* With nobody to name — nothing read yet, no agent on this machine — the bare
   word, never the label of whoever is selected in the settings window.
   `agents::pick` substitutes the first installed profile for a configured one
   that is not on `PATH`, so a name taken from the picker could stand over
   another agent's allowance; and before the first answer there is no reading
   to put a name on at all. */
export function usageAgentLabel(answer) {
  const id = agentOf(answer)
  if (!id) return 'Agent'
  return AGENT_LABELS[id] ?? id
}

/* `10%`, or the dash. `null` when the percentage is not a number — a half Rust
   could not read travels as an explicit `null` under its own key, and a build
   newer than this one could put anything there.

   It never becomes a zero: "0%" is a fact about an allowance, and drawing it
   over the absence of one is the mistake smetana-7rp was filed for. The
   reverse is as carefully not done: `0` is a number, a fresh week really does
   print it, and it draws as `0%`. */
function percent(pct) {
  return Number.isFinite(pct) ? `${pct}%` : DASH
}

/* The two segments, always both of them and always in this order. Session
   first: it is the sooner of the two limits, and the one somebody runs into at
   night.

   Unlike the settings block's rows, a half that was not read is not dropped —
   a strip that changed width with the answer would move the rest of the row
   under somebody's eye, and the dash is the whole point of the pair being
   fixed. Everything that is not a reading — `unsupported`, `unreadable`, a
   state this build has never heard of, nothing asked yet — is two dashes. */
export function usageSegments(answer) {
  const usage = answer?.state === READ ? answer.usage : null
  return [
    { name: 'Session', value: percent(usage?.sessionPct) },
    { name: 'Week', value: percent(usage?.weekPct) }
  ]
}

/* `Session resets Aug 7 at 8pm (Europe/Moscow)` — the harness's own words for
   when the allowance comes back, passed through untouched, the same string
   Rust deliberately never turns into a moment in time. Absent is an ordinary
   reading rather than a gap: a fresh allowance prints no reset at all. */
function resetLine(name, resets) {
  const when = typeof resets === 'string' ? resets.trim() : ''
  return when ? `${name} resets ${when}` : null
}

/* Everything the strip itself has no room for, in one hint: the two reset
   strings first, then the one sentence `usageNote` chooses — what a run would
   do at this reading, or, when there is no reading, why there is not.

   A band this build has never heard of contributes no sentence rather than
   guessing which of the three it meant, which is `usageNote`'s own behaviour
   and the reason it is borrowed rather than reproduced.

   `busy` is passed straight through and, there, beats whatever else would have
   been said. That is the whole of what the strip promises while a probe is
   out: the numbers on it stay where they are — blanking a permanent strip every
   ten minutes is a flicker in the corner of somebody's eye, and the strip never
   labels its figure fresh, so it claims nothing by keeping it — and the hint is
   where a reading under way is admitted to.

   Empty is a real answer, and the caller has to be ready for it: a reading in a
   band this build cannot name, printing no reset times, leaves nothing true to
   say. A hint that opened on an empty panel would be worse than none. */
export function usageTooltip(answer, busy = false) {
  const usage = answer?.state === READ ? answer.usage : null
  return [
    resetLine('Session', usage?.sessionReset),
    resetLine('Week', usage?.weekReset),
    usageNote(answer, busy)
  ]
    .filter(Boolean)
    .join(' · ')
}
