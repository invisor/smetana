/* When the bell has something to say about the attachment store, and what it
   says.

   Another of the `branchChoice.js` family — the whole of one rule, pure, with no
   Vue, no DOM and no Tauri in it, living under the part of the interface it is a
   rule about. That is not tidiness: a `.vue` file is the one thing no test in
   this repository can reach, so a threshold ladder left inside a component is a
   ladder nothing checks.

   The list of notifications is **derived, never accumulated**. Nothing about a
   notification survives a restart except one number per project — the highest
   threshold already announced — because everything this app has any use for
   saying is a statement about something it can look at right now. A stored copy
   would be a second source of truth going stale the moment the first one moves,
   and the failure it produces is a bell shouting about a folder somebody emptied
   an hour ago. */
import { basename } from '../../paths.js'
/* The store's own vocabulary for its own numbers, borrowed rather than written
   again. A second formatter here would eventually disagree with the screen this
   card sends a person to, which is the same failure two commands measuring one
   folder would produce — and it is exactly what `projectBytes` living over there
   with the rest of the survey's readers is for. */
import { formatBytes, projectBytes } from '../settings/storage.js'
/* The ending's own sentence and glyph, from the table the run bar already draws
   them out of. Borrowed for exactly the reason the formatter above is: a second
   copy here would eventually disagree with the bar a few centimetres away, and
   the two would be describing the same run. */
import { stopReason } from '../run/stopReason.js'

export { projectBytes }

/* Binary units, the same ones `settings/storage.js` spells out and the same ones
   the store's own ceiling is documented in. */
export const MIB = 1024 * 1024

/* The ladder, in MiB and ascending. Written out a second time in
   `settings/model.rs`, which validates the remembered number against it — the
   same doubling `SIDE_TABS` and `THEMES` carry, and the same warning: both
   copies move together, and a value here that is not there loses itself on the
   next read of the file, at the cost of one repeated warning. */
export const THRESHOLDS_MIB = [10, 50, 100]

/* A measurement that has not happened, or one whose answer says nothing, is not
   a size of zero — see `projectBytes` for why the difference matters. */
const known = (bytes) => typeof bytes === 'number' && Number.isFinite(bytes) && bytes >= 0

/* The highest threshold this size still reaches, or null when it reaches none.
   This is the whole of the re-arming rule: it is what the app remembers after
   every measurement, so a folder cleaned back under 10 MiB forgets that 10 was
   ever announced and the next crossing speaks again. */
export function reachedThreshold(bytes) {
  if (!known(bytes)) return null
  let reached = null
  for (const step of THRESHOLDS_MIB) {
    if (bytes >= step * MIB) reached = step
  }
  return reached
}

/* The threshold to announce now, or null when there is nothing new to say.
   `announced` is what the project remembers, which is null on a project nobody
   has ever been warned about and a number from the ladder afterwards. */
export function crossedThreshold(bytes, announced) {
  const reached = reachedThreshold(bytes)
  if (reached === null) return null
  const before = known(announced) ? announced : 0
  return reached > before ? reached : null
}

/* What the project remembers once this measurement is over — the announcement
   and the re-arming are one write, which is why there is no separate "already
   warned" flag and no separate "dismissed" one. Dismissing a card is this same
   write for the size measured at that moment, and there is nothing a second flag
   could express that this number does not.

   A measurement that did not happen changes nothing: taking it as zero would
   forget an announcement the folder has not shrunk out of, and the next readable
   measurement would say the same thing a second time. */
export function rememberAfter(bytes, announced) {
  if (!known(bytes)) return known(announced) ? announced : null
  return reachedThreshold(bytes)
}

/* Whether a card announced at `threshold` is still a true statement about the
   folder. A card outlives the measurement that made it — it stays on screen
   until somebody dismisses it or cleans the folder — so every later measurement
   is asked this, and the card goes the moment the answer is no. */
export function stillOver(bytes, threshold) {
  if (!known(bytes) || !known(threshold)) return false
  return bytes >= threshold * MIB
}

/* The card itself. `source` is what the panel and the acting code switch on, and
   it is the whole of the shape a second source would plug into: an id nothing
   else can collide with, a glyph, two lines of prose and the label of the one
   action that is not Dismiss.

   The prose names the folder, the size and the threshold, and says where the
   button leads rather than what it deletes — Clean up opens a screen, it does
   not remove anything, and a card promising otherwise would be the app claiming
   an irreversible act happens without a press. */
export function storageNotification(project, bytes, threshold) {
  return {
    id: `storage:${project}:${threshold}`,
    source: 'storage',
    project,
    bytes,
    threshold,
    icon: 'hard-drive',
    title: 'Attachment storage is growing',
    body: `${basename(project)} has ${formatBytes(bytes)} of stored images, past the ${threshold} MiB mark. Clean up opens Storage in settings, where the images no open task refers to can be deleted.`,
    actionLabel: 'Clean up'
  }
}

/* ---- the second source: a run that is over -------------------------- */

/* How long the run took, read as somebody would say it: hours and minutes, or
   minutes, or seconds. One wall-clock number and no breakdown by state — the
   only number about a run's time that cannot be computed wrongly, since a run
   spends its night in the preflight, in batches, in pauses and in backoff, and
   nothing anywhere measures those separately.

   `null` for anything that is not a finite, non-negative number, which is the
   same distinction `projectBytes` draws: a duration nobody measured is not a
   duration of zero, and the card simply says nothing about the time rather than
   claiming the run took no time at all. */
export function formatDuration(seconds) {
  if (typeof seconds !== 'number' || !Number.isFinite(seconds) || seconds < 0) return null
  const whole = Math.floor(seconds)
  const hours = Math.floor(whole / 3600)
  const minutes = Math.floor((whole % 3600) / 60)
  if (hours > 0) return `${hours}h ${minutes}m`
  if (minutes > 0) return `${minutes}m`
  return `${whole}s`
}

/* The card for a run that has stopped, or `null` for one that has not.

   Deliberately short: the ending, the two counts, the duration, and one button
   through to the document that holds everything else. A card that restated the
   report would be the question block removed in smetana-s4f all over again —
   it repeated what was already on screen and pushed what mattered down the
   column.

   The ending's own sentence goes into the body rather than into the title, and
   that is the one wording decision here. Every entry in `REASONS` is already a
   whole statement about how a run ended, several of them carrying an em dash of
   their own ("Done — nothing left to take"), so folding one into a title after
   a second dash reads as two sentences run together — and lower-casing it to
   make that fit would be this file rewriting prose whose only authored copy is
   next door. The title says what the card is about and the body says what
   happened, which leaves `stopReason.js` the single source of the words.

   A stopped run carrying no `summary` at all still gets a card: this front end
   may be older than the worker, and an ending nobody can describe is still an
   ending worth announcing. */
export function runNotification(run) {
  if (run?.state?.kind !== 'stopped') return null
  const ending = stopReason(run.state.reason?.kind)
  const summary = run.summary ?? null
  const tasks = summary?.tasks ?? null
  const duration = formatDuration(summary?.seconds)

  const parts = [ending.text]
  /* Three cases, not two, and the third is why. An unread board is never a zero
     — the same rule `projectBytes` and `cleanup::refusal` keep, and the reason
     `RunSummary.tasks` is an option at all — so a summary whose `tasks` is null
     says the board could not be read rather than "0 closed · 0 parked". But a
     run carrying **no summary at all** has not failed to read anything: nothing
     has looked yet. `request_stop` ends a run with nothing in flight at once,
     while the account is made a moment later by the loop and arrives through
     `Run::take_summary_from`, so every press of Stop between batches passes
     through this state on its way to the real counts. Saying "the board could
     not be read" there would announce a failure that did not happen, for the
     seconds it takes the summary to land. Saying nothing about the board is the
     truth in that window and reads as the ending alone. */
  if (tasks) parts.push(`${tasks.closed?.length ?? 0} closed`, `${tasks.parked?.length ?? 0} parked`)
  else if (summary) parts.push('the board could not be read')
  if (duration) parts.push(duration)

  return {
    /* One card per stopped run, named by the token, which is the one name that
       is never two runs': a project holds several at once, and the badge counts
       them all. */
    id: `run:${run.token}`,
    source: 'run',
    token: run.token,
    project: run.project,
    /* Absolute, as the worker wrote it. Turning it into the tab path is the
       acting code's job and `reportTab.js`'s rule. */
    report: summary?.report ?? null,
    /* The glyph the bar draws for this ending, taken whole. `stopReason`
       answers with one for every ending, known or not, so there is no default
       to write here — and writing one is exactly how this card and the bar
       would come to disagree about a run they are both describing. */
    icon: ending.icon,
    title: 'Run finished',
    body: parts.join(' · '),
    /* No document, no button. A card offering details that are not there is
       worse than one carrying nothing but Dismiss. */
    ...(summary?.report ? { actionLabel: 'Show details' } : {})
  }
}
