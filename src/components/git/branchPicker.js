/* What the branch picker's list holds, and what every row of it says.

   The `reviewRows.js` / `branchMenu.js` / `branchTree.js` family: pure, no Vue,
   no DOM and no Tauri, which is the whole reason it is a file of its own — a
   `.vue` file is the one thing no test in this repository can reach, so a rule
   left inside `BranchPicker.vue` is a rule nothing checks. What stays in the
   component is boxes, tokens and events; the filtering, the order, the meta
   line and the counter are all here.

   **`origin` is a prefix and not a second side.** The window this is built for
   used to ask the question twice — a branch in one dropdown, `local`/`origin`
   in another beside it — and a person picking `origin/main` had to say `main`
   in one control and `origin` in the other, in either order, with neither
   control saying what the other held. One list answers both at once: every
   branch appears twice in a row, itself and then its `origin/` variant, so
   choosing a branch and choosing a side is one movement and the second control
   is gone. The order within the pair is not a preference — the local branch is
   the ordinary answer and the remote one is the deliberate one, so the
   deliberate one sits under the finger that has already found the name.

   **Times arrive as epoch seconds, and so does `now`.** That is the unit git
   speaks in and the unit the branch list is being extended to carry
   (smetana-fczk); mixing it with the milliseconds `Date.now()` answers in is
   how a `2h` becomes a `56y`, so every clock in this module is seconds and
   nothing here calls `Date.now()` itself. A caller with a millisecond clock
   divides on the way in — the component does exactly that, in one place.

   The field a branch carries its own time in is **`at`**, which is the epic's
   contract and not this module's choice, and the name is worth defending
   anyway: `updated_at` is already read off a bd issue in four places in this
   front end and is an ISO 8601 string there. Two names would have been a
   nuisance; one name over two types would be a bug nobody sees, since a string
   where a number is expected fails the `Number.isFinite` guard below and
   silently drops the age from every row, which is exactly what a branch with no
   stamp at all is supposed to look like.

   A missing timestamp is an ordinary outcome rather than an error: the branch
   list may be older than the field that carries the time, and a repository
   nobody has fetched into has no fetch to report. Every such piece is left out
   of the meta line entirely, which is what keeps a shortened line from ever
   being `NaN`, `Invalid Date` or an empty gap between two separators. */

/* The filter's placeholder and the footer's key hint, here rather than in the
   component for the same reason `sessionRow.js` keeps its headings: they are
   words somebody may want to check, and no test can read them out of a `.vue`.
   Sentence case, as everything in this system is. */
export const BRANCH_FILTER_LABEL = 'Filter branches'
export const PICKER_KEY_HINT = '↑↓ move · enter select · esc close'

/* What is said when the filter has left nothing. A sentence and not a blank
   area, which would read as a list that failed to draw rather than as one that
   worked and found nothing. */
export const NO_BRANCH_MATCHES = 'No branch matches that.'

/* The separator between two facts about one thing — the same middot
   `agent/sessionRow.js`, `shell/projectState.js` and `settings/usage.js` put
   there. It is joined into one string here, and that is safe in a way it was
   not there: a meta line in this list is one span in one family at one size,
   so there is nothing for a line break to fall between. */
const SEPARATOR = ' · '

const list = (value) => (Array.isArray(value) ? value : [])

const MINUTE = 60
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR
const WEEK = 7 * DAY
const YEAR = 365 * DAY

/* How old something is, in the shortest form that is still unambiguous: `2h`,
   `3d`. No "ago" on it — it sits in a line that already says what it is about,
   and the row is narrow enough that two extra words per row is a real cost.

   Deliberately not `sessionRow.js`'s `relativeTime`: that one answers a
   different question in a different voice (`18h ago`, `just now`, from an ISO
   string) for a row that is read as prose. Sharing one function between the two
   would mean one of the two saying something it does not mean.

   A time in the future clamps to `now` rather than being refused: a branch
   touched a second ago on a machine whose clock is a minute fast is an ordinary
   thing, and `-1m` in a list of branches is not. */
export function shortAge(at, now) {
  if (!Number.isFinite(at) || !Number.isFinite(now)) return null
  const secs = Math.max(0, Math.floor(now - at))
  if (secs < MINUTE) return 'now'
  if (secs < HOUR) return `${Math.floor(secs / MINUTE)}m`
  if (secs < DAY) return `${Math.floor(secs / HOUR)}h`
  if (secs < WEEK) return `${Math.floor(secs / DAY)}d`
  if (secs < YEAR) return `${Math.floor(secs / WEEK)}w`
  return `${Math.floor(secs / YEAR)}y`
}

/* When origin was last asked, for the origin rows: `fetched 2m ago`.

   This one keeps its "ago", because the fact is about a moment in the past and
   `fetched 2m` would read as a duration of the fetch itself. Under a minute it
   says so in words: `fetched now ago` is not a sentence.

   Null when there is no fetch time at all, which is the ordinary state of a
   repository nobody has fetched into — the origin row then says `origin` and
   nothing more, rather than claiming a freshness it has no evidence for. */
export function fetchedLabel(at, now) {
  if (!Number.isFinite(at) || !Number.isFinite(now)) return null
  const secs = Math.max(0, Math.floor(now - at))
  if (secs < MINUTE) return 'fetched just now'
  return `fetched ${shortAge(at, now)} ago`
}

/* How many of the project's repositories have this branch, said in words.

   Derived from `missing_in` and the project's repository count rather than
   carried as a number of its own, which is the same reading `reviewRows.js`
   takes of the same field: the absence is what the back end reports, because an
   empty `missing_in` is the ordinary case and there is nothing to do with it.

   Null when the count is not known — a picker handed no repository count at all
   is a picker that cannot honestly say `0 repos`, and the meta line is shorter
   by one piece rather than wrong by one fact. */
export function repoCountLabel(branch, repos) {
  if (!Number.isFinite(repos) || repos <= 0) return null
  const missing = list(branch?.missing_in).length
  const n = Math.max(0, Math.trunc(repos) - missing)
  return `${n} ${n === 1 ? 'repo' : 'repos'}`
}

/* The right-hand end of one row: `local · 6 repos · 2h`, or
   `origin · fetched 2m ago`.

   The two sides say different things because different things are worth knowing
   about them. A local branch is the project's own — how much of the project has
   it, and how long ago somebody touched it. What origin holds is one copy of
   one thing, and the only question about it is how stale this machine's idea of
   it is.

   The side is always the first piece and is never dropped, which is what makes
   a row readable when everything else about it is unknown: `local` alone is
   still a complete answer, and it is the piece that tells the pair of rows
   apart when the names are the same by construction. */
export function branchMeta(branch, options = {}) {
  const { origin = false, repos = 0, now = null, fetchedAt = null } = options
  const pieces = origin
    ? ['origin', fetchedLabel(fetchedAt, now)]
    : ['local', repoCountLabel(branch, repos), shortAge(branch?.at, now)]
  return pieces.filter(Boolean).join(SEPARATOR)
}

/* The branches a filter leaves, matched on the name as a substring and without
   regard to case.

   A substring and not a prefix: branch names in this tree are
   `feature/smetana-t0yh-…`, so a prefix match would mean typing the word
   `feature` before reaching anything a person is actually looking for. The
   needle is trimmed, so a trailing space picked up from a paste does not empty
   the list.

   Matched against the bare name, which is the same string on both rows of a
   pair: typing `origin` filters nothing, since the prefix is a fact about the
   row rather than part of the branch's name, and hiding every local row the
   moment somebody typed it would be the second control coming back in
   disguise. */
export function matchingBranches(branches, query) {
  const needle = typeof query === 'string' ? query.trim().toLowerCase() : ''
  const all = list(branches).filter((branch) => typeof branch?.name === 'string' && branch.name)
  if (!needle) return all
  return all.filter((branch) => branch.name.toLowerCase().includes(needle))
}

/* The list itself: two rows per branch, the local one and then the `origin/`
   one, in the order the branches arrived in.

   The order is `git::by_recency`'s and is never re-sorted here, for the reason
   `BranchList.vue` gives about the same list: the branch somebody merges into
   every day is nowhere in particular alphabetically, so sorting by name would
   bury the one row that matters. Nothing about the pair is interleaved across
   branches either — `main`, `origin/main`, `develop`, `origin/develop` — so a
   person reading down the list sees each name once, twice over.

   `key` is what a `v-for` is keyed on and the side is part of it: the two rows
   of a pair carry the same name and would otherwise be one key twice, at which
   point Vue reuses a row for the other side of the same branch. */
export function pickerRows(branches, options = {}) {
  const { query = '', repos = 0, now = null, fetchedAt = null } = options
  const rows = []
  for (const branch of matchingBranches(branches, query)) {
    for (const origin of [false, true]) {
      rows.push({
        key: `${origin ? 'origin' : 'local'}:${branch.name}`,
        name: branch.name,
        origin,
        meta: branchMeta(branch, { origin, repos, now, fetchedAt })
      })
    }
  }
  return rows
}

/* The counter at the right of the filter row: `4 of 41`.

   It counts **branches and not rows**, which is the only reading that stays
   still while somebody types: the rows are two per branch by construction, so a
   row count would open at `82 of 82` on a project of 41 branches and say
   nothing a person could use. `41 of 41` is the count of things there are to
   choose between, and the second number is the whole list rather than the
   filtered one — that is the point of it, since a filter that matches nothing
   and a project with no branches at all look identical without it. */
export function branchCountLabel(shown, total) {
  const of = Number.isFinite(total) ? Math.max(0, Math.trunc(total)) : 0
  const has = Number.isFinite(shown) ? Math.max(0, Math.trunc(shown)) : 0
  return `${Math.min(has, of)} of ${of}`
}

/* Where the highlight lands after a key press, given where it is now.

   Wrapping, and bounded by the list's own length, so the arrows always move and
   an empty list stops rather than spinning. It is here rather than in the
   component because "down from the last row is the first row" is exactly the
   sort of rule that is written once and then quietly broken by an off-by-one
   nobody can see in a screenshot. */
export function stepCursor(cursor, delta, length) {
  const n = Number.isFinite(length) ? Math.max(0, Math.trunc(length)) : 0
  if (!n) return 0
  const from = Number.isFinite(cursor) ? Math.trunc(cursor) : 0
  return (((from + delta) % n) + n) % n
}
