/* Everything the command palette decides, pure, with no Vue and no DOM in it —
   which is what makes it the one part of the palette a test in this repository
   can reach.

   It searches the snapshot the tracker store already holds, and that snapshot is
   `bd list --all`: every task in the project, closed ones included. So this
   reaches past the board deliberately — a task in a hidden column, or one
   outside the period the board is filtered to, is found here and nowhere else on
   screen. What it does not do is exclude the merge lock; that rule lives beside
   `isLockIssue` in `stores/tracker.js`, which imports Tauri, and the caller
   hands over a list with the lock already gone. */

/* Twenty rows is a list somebody reads; the panel scrolls at 320px and a longer
   list is a second screenful nobody asked for. */
export const SEARCH_LIMIT = 20

/* Tail of an id — `holiday-curb-bhyv` is four characters of signal behind a
   project prefix every row on screen shares. The relation column has room for
   the four and not for the prefix. */
export const shortId = (id) => {
  const at = String(id ?? '').lastIndexOf('-')
  return at < 0 ? String(id ?? '') : String(id).slice(at + 1)
}

/* Where the match sits, then the newest, and the id last of all — so that two
   tasks updated inside the same second cannot trade places between one
   keystroke and the next. */
const compare = (a, b) =>
  a.at - b.at ||
  (b.issue.updated_at ?? '').localeCompare(a.issue.updated_at ?? '') ||
  a.issue.id.localeCompare(b.issue.id)

/* Id and title only, joined by a space so a query spanning both still matches.
   The prose fields are deliberately not read: the palette's text mode answers
   "which task is this", and "which task mentions this" is what meaning mode is
   for. The narrower guarantee is said out loud rather than half-kept — a
   ninety-character snippet of somebody's notes has no column to sit in here. */
export function filterIssues(issues, query, limit = SEARCH_LIMIT) {
  const needle = (query ?? '').trim().toLowerCase()
  if (!needle) return []

  const hits = []
  for (const issue of issues) {
    const at = `${issue.id ?? ''} ${issue.title ?? ''}`.toLowerCase().indexOf(needle)
    if (at >= 0) hits.push({ issue, at })
  }

  return hits
    .sort(compare)
    .slice(0, limit)
    .map(({ issue }) => ({ id: issue.id, title: issue.title, status: issue.status }))
}

/* The one relation a row has room for.

   The order is the point: a blocker is why the task cannot be started, a parent
   is where it sits, a downstream count is what waits on it — most actionable
   first, and the first that answers wins.

   The blocker and the downstream count come from the store's own maps rather
   than from `dependencies` or `dependent_count` on the issue. Those maps carry a
   rule the raw fields do not: a blocker that is closed, or absent from the
   board, no longer blocks. A second implementation of that rule here would
   disagree with the board's own hatching within a month. */
export function relationOf(issue, edges) {
  const blockers = edges.blockedBy.get(issue.id) ?? []
  if (blockers.length) return { icon: 'lock', label: shortId(blockers[0]) }
  if (issue.parent) return { icon: 'corner-down-right', label: shortId(issue.parent) }
  const downstream = edges.blocking.get(issue.id) ?? []
  if (downstream.length) return { icon: 'git-fork', label: String(downstream.length) }
  return null
}

/* Exactly one heading, ever — a heading plus an empty-state line is the pair the
   design was redrawn to break up.

   It reads `answered` and not the mode, and that is the whole of the honesty
   here: the meaning tier is an agent with a ninety-second deadline, so between
   the keypress and the answer the rows below are still the text matches. A
   heading following the mode would say "By meaning" over them for a minute and a
   half. */
export const sectionLabel = ({ query, answered }) => {
  if (!query.trim()) return 'Recent'
  return answered ? 'By meaning' : 'Matching text'
}

/* How much of the scope the list is showing.

   Silent at either end of the fraction, and both silences are the same rule:
   the counter is worth drawing only when it is telling somebody something. A
   scope of nothing has nothing to count, and a count of nothing is already said
   — and said better — by the empty state a row below it, which is exactly the
   pair of competing blocks this design was redrawn to stop drawing. The query
   itself is not read here: the caller passes a shown count of 0 while nothing is
   typed, because the rows under an empty query are the recents and counting
   those against the whole project would answer a question nobody asked. */
export const counterLabel = (shown, total) => (shown && total ? `${shown} of ${total}` : '')

/* What the wait says while the agent has the question.

   It sits here beside the other two labels rather than in the component for
   the reason the whole file exists: a `.vue` is the one thing no test in this
   repository can reach, and this string is the only part of the waiting row
   that can be got wrong quietly.

   The seconds are the whole of it. The call has a ninety-second deadline
   (`agents/oneshot.rs`), which is long enough that the question a person is
   really asking a few seconds in is "is it working or has it hung", and only a
   number that moves answers that — a fixed "this may take a while" does not
   tell the two apart. Whole seconds and never a fraction: it is read at a
   glance, off a row that repaints once a second. Anything unusable — a
   negative, a `NaN` from arithmetic on a clock that was never started — reads
   as nought rather than reaching the screen as itself. */
export const waitingLabel = (seconds) => {
  const whole = Math.max(0, Math.floor(Number(seconds) || 0))
  return `Asking the agent… ${whole}s`
}

/* Wrapping at both ends, so ↓ off the bottom row lands on the first. */
export const stepIndex = (current, by, length) =>
  length ? (current + by + length) % length : 0
