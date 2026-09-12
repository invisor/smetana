/* How long a turn has taken, spelled two different ways for two different
   readers — pure, and outside `TurnResult.vue` and `Reasoning.vue` for the
   reason every module in this family sits outside the component that draws
   it: a `.vue` file is the one thing no runner in this repository can reach.

   `formatReceiptDuration` is the receipt's own voice: a number somebody reads once,
   after the fact, so it is worth a tenth of a second under a minute. It moved
   here from `TurnResult.vue` unchanged — the strip's `done` moment still
   spells `13.0 s` this way, and past a minute `2 m 14 s`, with a space either
   side of both letters.

   `formatElapsedClock` is the ticking clock's voice: a number somebody
   watches move, so it counts whole seconds and reads compactly — `4s`, `12s`,
   `2m 14s` — the exact spelling `markup-contract.md` section 6 uses for the
   strip's `waiting` and `failed` moments and `Reasoning`'s own `<summary>
   <time>`. No space between a number and the letter that names its unit: a
   clock is read at a glance and a stray space is one more thing in the way of
   that. The two never trade places — a `done` line printed in the compact
   form would lose the tenth of a second the receipt exists to show, and a
   ticking clock printed with one would flicker a digit that means nothing at
   a whole-second refresh. */

export function formatReceiptDuration(ms) {
  const value = Number(ms) || 0
  if (value < 1000) return `${Math.round(value)} ms`
  if (value < 60000) return `${(value / 1000).toFixed(1)} s`
  const minutes = Math.floor(value / 60000)
  const seconds = Math.round((value % 60000) / 1000)
  return `${minutes} m ${String(seconds).padStart(2, '0')} s`
}

export function formatElapsedClock(ms) {
  const totalSeconds = Math.max(0, Math.floor((Number(ms) || 0) / 1000))
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return minutes > 0 ? `${minutes}m ${String(seconds).padStart(2, '0')}s` : `${seconds}s`
}
