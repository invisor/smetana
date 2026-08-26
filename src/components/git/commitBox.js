/* When the commit box may be pressed, and what it says when it may not.
 *
 * Pure, of the `gitActions.js` family and here for the same reason: a `.vue`
 * file is the one thing no test in this repository can reach, so the whole of a
 * rule lives outside the component that draws it.
 *
 * The rule it is **not** allowed to hold is whether this project may be written
 * to at all — that is `gitActions.js`, which reads the project's runs, and its
 * verdict arrives here as `allowed` and `reason`. Two files deciding one thing
 * is how the panel starts contradicting itself.
 */

/* ---- how tall the field is ---------------------------------------------- *
 *
 * In **rows**, and never in pixels, which is `sectionHeights.js`'s load-bearing
 * decision one file over and holds here for the same reason with one addition
 * of its own. A row follows the density and the app-wide font size, so a count
 * survives both where a pixel height would have to be rewritten by each of them
 * — and this field measures itself in rows already: `rows` is the browser's own
 * attribute on a `<textarea>`, which `Textarea.vue` chose deliberately over a
 * computed height, so a count is the unit the control actually speaks. The one
 * pixel measurement lives at the edge, in the component, where a drag's
 * displacement is divided by the field's own line height to arrive here in
 * rows.
 *
 * The stored count and the drawn count are the same number here, and that is
 * the difference from the sections beside it. Those clamp against the panel
 * they are in now, because a section competes with its neighbours for one
 * column of height. This field does not compete with anything: the box is
 * sticky at the top of a scroller and the rows go under it, so a field somebody
 * dragged tall in a tall window is a field that scrolls in a short one — which
 * is what the whole section already does.
 */

/** Two rows, which is what this field was fixed at before it could be dragged.
 *
 * A message is a subject and, sometimes, a body; two rows is enough to see the
 * subject and to know there is a second line, and it is what everybody's
 * `settings.json` will read as until they drag it. */
export const DEFAULT_ROWS = 2

/** One row is still a field; nothing is a control that has disappeared. */
export const MIN_ROWS = 1

/** The ceiling, and it is about the panel rather than about messages.
 *
 * A commit message longer than this is an ordinary thing to write and is not
 * being refused — the field scrolls, as it always has. What the ceiling
 * protects is the section underneath: this box is sticky at the top of the
 * change list, so past a dozen rows it stops being a field over a list and
 * becomes a list nobody can see. `MAX_SECTION_ROWS` is 40 next door for a
 * section that *is* the content; this one is not. */
export const MAX_ROWS = 12

/** Stored rows → the rows to draw. A number from a hand-edited file is worth
 *  clamping rather than trusting; Rust forgets one outside the same range on
 *  its way in, so this is the second of two guards and neither is the only one. */
export function clampRows(want) {
  const rows = Math.round(Number(want))
  if (!Number.isFinite(rows)) return DEFAULT_ROWS
  return Math.min(Math.max(rows, MIN_ROWS), MAX_ROWS)
}

/**
 * Where a drag leaves the field, in rows.
 *
 * `base` is the count snapshotted at `dragstart` and `delta` the separator's
 * displacement in rows since that same moment, never since the last frame:
 * clamping against the previous frame would make each clamped move the new
 * origin and the field would drift away from the pointer, which is the drift
 * `Resizer`'s own contract warns about.
 *
 * The separator is **below** the field, so downwards grows it — the sign the
 * repositories section takes and not the branches'.
 */
export function resolveDragRows({ base, delta }) {
  return clampRows(base + delta)
}

/* Everything that has to be true at once. Written as one function rather than
   as a chain of `v-if`s in the template, because the same four facts decide the
   sentence below and the two must not be able to disagree — a dead button with
   nothing to say about itself is the failure this pair exists to prevent. */
export function canCommit({ message, changes, allowed, busy }) {
  return Boolean(allowed) && !busy && changes > 0 && message.trim().length > 0
}

/* Asking the agent for a message is a **read**, so a run holding the three
   writes has no say over it — the same line `BranchList` draws when it refuses
   to dim a folder heading. What it does need is something to describe and no
   question already in flight. */
export function canSuggest({ changes, suggesting }) {
  return changes > 0 && !suggesting
}

/* What the empty field says, which is three things at once: what it is for, how
   to send it without reaching for the button, and **where the commit is about
   to land**. The last is the one worth the width — a repository panel is a
   place people work in several branches from, and a commit is the write with no
   undo in this app.
 *
 * The key is named rather than drawn as a glyph nobody can search for, and it
 * follows the platform: `⌘` on a Mac, `Ctrl` everywhere else. A detached HEAD
 * has no branch to name and simply does not name one — "on nothing" would be
 * worse than silence, and the commit still works.
 */
export function messagePlaceholder({ branch, mac }) {
  const key = mac ? '⌘Enter' : 'Ctrl+Enter'
  return branch ? `Message (${key} to commit on "${branch}")` : `Message (${key} to commit)`
}

/* The count is on the button because the scope is the whole list: this app has
   no staging, so what a press commits is what the section above is drawing, and
   a button that said only "Commit" would leave that unsaid. */
export function commitLabel(changes) {
  if (!changes) return 'Commit'
  return `Commit ${changes} file${changes === 1 ? '' : 's'}`
}

/* Why it cannot be pressed, in one sentence, or null when there is nothing to
   explain.
 *
 * The order is the whole of the rule. A run's own sentence comes first because
 * it names something outside this panel; a clean tree comes before the missing
 * message because it is the one of the two a person cannot type their way out
 * of; and git already working says nothing at all, since that state ends by
 * itself in a moment.
 */
export function commitHint({ message, changes, allowed, reason, busy }) {
  if (!allowed) return reason ?? null
  if (busy) return null
  if (!changes) return 'Nothing to commit.'
  if (!message.trim()) return 'Write a message first.'
  return null
}
