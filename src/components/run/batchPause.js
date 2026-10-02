/* The pause between two batches of an autopilot run, as a rule rather than as a
   form, and the words the run bar uses while one is being waited out.

   The `projectDefaults.js` family: pure, no Vue and no DOM, which is the whole
   reason it is a file of its own — no test in this repository can reach a
   `.vue`, so a rule left inside the dialog is a rule nothing checks.

   The pair is a closed interval of whole minutes, and the run picks one number
   out of it, inclusive at both ends, before every batch but the first. `min ==
   max` is a fixed pause and `0`/`0` is no pause at all. The same bounds are in
   `RunSettings::validate` and `RunDefaults::validate` on the Rust side, which is
   what refuses a pair that arrives anyway. */

/* The same pair `RunDefaults` defaults to in src-tauri/src/settings/model.rs.
   Frozen so a field filled from it has to take a copy. */
export const BATCH_PAUSE_DEFAULTS = Object.freeze({ min: 10, max: 30 })

/* Twelve hours. A ceiling on a typo — "3000" for "30" would park a run for two
   days — and not a statement about what a night is. */
export const BATCH_PAUSE_CEILING = 720

/* What the dialog opens on: what the project remembers, else the defaults, each
   half on its own so a file carrying one does not lose it to the other. */
export function pauseFrom(kept) {
  return {
    min: kept?.batchPauseMin ?? BATCH_PAUSE_DEFAULTS.min,
    max: kept?.batchPauseMax ?? BATCH_PAUSE_DEFAULTS.max
  }
}

/* A number field hands back text, and an empty or fractional one is not a count
   of minutes. `null` is "not a valid number", which is a different thing from
   zero and must stay different. */
export function toMinutes(raw) {
  if (typeof raw === 'number') return Number.isInteger(raw) && raw >= 0 ? raw : null
  const text = String(raw ?? '').trim()
  if (!/^\d+$/.test(text)) return null
  return Number(text)
}

/* `{}` when the pair is a valid one, else a sentence per field that is wrong.
   `min` is blamed for a pair that runs backwards only when each half is fine by
   itself, so one mistake draws one sentence. */
export function validatePause(min, max) {
  const errors = {}
  const low = toMinutes(min)
  const high = toMinutes(max)
  if (low === null || low > BATCH_PAUSE_CEILING) errors.min = `Between 0 and ${BATCH_PAUSE_CEILING}.`
  if (high === null || high > BATCH_PAUSE_CEILING) errors.max = `Between 0 and ${BATCH_PAUSE_CEILING}.`
  if (!errors.min && !errors.max && low > high) errors.max = 'Not less than the minimum.'
  return errors
}

export const isValidPause = (min, max) => Object.keys(validatePause(min, max)).length === 0

/* The two numbers as the payload carries them, or null for a pair that cannot
   be sent. */
export function pausePayload(min, max) {
  return isValidPause(min, max) ? { min: toMinutes(min), max: toMinutes(max) } : null
}

/* `Resting { until, minutes }` on the Rust side, serialised as kind `resting`.
   The words are the bar's label and its detail line: how long it was told to
   wait and when that ends, since a quiet run is indistinguishable from a hung
   one. */
export function restLabel(state) {
  if (state?.kind !== 'resting') return null
  return `Pausing — next batch in ${state.minutes} min`
}

export function restDetail(state, locale) {
  if (state?.kind !== 'resting') return null
  const at = new Date(state.until)
  if (Number.isNaN(at.getTime())) return null
  const time = at.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })
  return `until ${time}`
}
