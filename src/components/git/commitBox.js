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
