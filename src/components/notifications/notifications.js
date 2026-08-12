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
