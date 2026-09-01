/* What the two run-gate rows on the Agents tab may be set to, and nothing about
   how they look.

   Another of the `usage.js` family next door: the whole of one rule, pure, with
   no Vue and no DOM in it, out here because a `.vue` file is the one thing no
   test in this repository can reach.

   The ladder is written out a second time in `src-tauri/src/settings/model.rs`
   as `SUBSCRIPTION_STEPS`, and the two copies move together. It is doubled
   rather than fetched because each end needs it for a different job: Rust
   refuses a hand-edited number that is not on it, and this end has to have
   something for a `Dropdown` to draw. A rung missing from Rust's copy is a
   choice that reverts on the next open; one missing from this copy is simply
   not offered.

   Nothing here decides anything about a run. Which band a reading falls in is
   `runs::usage::decide`'s answer and arrives already named — see `usage.js`. */

/* `0` is off rather than a percentage: "pause when nothing has been used" is
   not a setting anybody could mean, and the wire between the two windows cannot
   carry a `null` — `adopt()` in `views/SettingsWindow.vue` skips one. */
export const SUBSCRIPTION_STEPS = [0, 50, 60, 70, 75, 80, 85, 90, 95]

export function thresholdOptions() {
  return SUBSCRIPTION_STEPS.map((value) => ({
    value,
    label: value === 0 ? 'Off' : `${value}%`
  }))
}

/* The guard `applyPatch` uses. A number and on the ladder — a string `'90'`
   would sit in the file as a type Rust drops on the way in, and the choice
   would revert at the next open with nothing on screen to say so. */
export function isThreshold(value) {
  return typeof value === 'number' && SUBSCRIPTION_STEPS.includes(value)
}

/* The front end's copy of the last rule in `SubscriptionSettings::validate`
   (`src-tauri/src/settings/model.rs`): a `reducedAt` at or above an enabled
   `pauseAt` is turned off rather than moved, because there is no band left
   between the two for it to name.

   The copy exists because Rust applies that rule inside `merge()`, on the way
   to disk, where nothing on screen can hear it. With 75 in "Take fewer tasks
   at", choosing 75 or less in "Pause a run at" wrote `reducedAt: 0` to the file
   while the dropdown went on showing 75; the gate then acted on Off, and the
   row read Off at the next open with nothing having said so. Applied here the
   dropdown moves at the moment of the choice, and `announce()` carries the
   corrected value straight back to the settings window.

   A `pauseAt` of `0` is off, so there is no pause to be under and `reducedAt`
   stands whatever it is — the same edge case Rust's `pause_at != OFF` guard
   makes. */
export function reconcile(pauseAt, reducedAt) {
  return pauseAt !== 0 && reducedAt >= pauseAt ? 0 : reducedAt
}
