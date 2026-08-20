/* Finding a task by typing at it: the whole of the instant tier, pure, with no
   Vue and no DOM in it, which is what makes it the one part of the search a
   test can reach.

   It searches the snapshot the tracker store already holds, and that snapshot
   is `bd list --all` — every task in the project, closed ones included, prose
   included. So this reaches past the board deliberately: a task in a hidden
   column, or one outside the period the board is filtered to, is found here and
   nowhere else on screen.

   What it does *not* do is exclude the merge lock. That rule lives in
   `stores/tracker.js` beside `isLockIssue`, which imports Tauri; the caller
   hands over a list with the lock already gone. */

/* Twenty rows is a list somebody reads. Forty is a second screen with no
   scrollbar worth drawing in a bar this size. */
export const SEARCH_LIMIT = 20

/* How much text a prose hit is worth showing: wide enough for the match to sit
   in a phrase, narrow enough that the rows stay one line each. */
const SNIPPET = 90

/* The rank tiers, best first. They are separate from the fields below because
   two of them are about the id, which is not a field somebody typed prose
   into: an id is either the thing you meant or it is not. */
const ID_EXACT = 0
const ID_PREFIX = 1
const TITLE = 2
const PROSE = 3

/* The prose fields, in the order one issue's own fields are tried: the first
   that matches is the field the row names, and the rest are not looked at. It
   decides nothing between two issues — prose is a single tier, and two issues
   in it are ordered by where the match sits, whichever fields those matches
   were found in.

   bd's own names on the left, since that is what the snapshot carries; the
   camelCase name on the right is what the row draws, because the interface is
   sentence case English and `acceptance_criteria` is neither. */
const PROSE_FIELDS = [
  ['description', 'description'],
  ['acceptance_criteria', 'acceptanceCriteria'],
  ['design', 'design'],
  ['notes', 'notes']
]

/* A window of text around the match, with an ellipsis on whichever side was
   actually cut — an ellipsis on a side that was not cut says the text goes on
   when it does not. */
const snippetAround = (text, at, length) => {
  const start = Math.max(0, at - Math.floor((SNIPPET - length) / 2))
  const end = Math.min(text.length, start + SNIPPET)
  const body = text.slice(start, end).replace(/\s+/g, ' ').trim()
  return `${start > 0 ? '…' : ''}${body}${end < text.length ? '…' : ''}`
}

/* The one match for one issue, or nothing. The first tier that answers wins,
   so an issue matching in both its title and its notes is a title hit — the
   stronger reason is the one worth drawing. */
const match = (issue, needle) => {
  const id = (issue.id ?? '').toLowerCase()
  if (id === needle) return { issue, tier: ID_EXACT, at: 0, field: 'id', snippet: '' }
  if (id.startsWith(needle)) return { issue, tier: ID_PREFIX, at: 0, field: 'id', snippet: '' }

  const title = issue.title ?? ''
  const at = title.toLowerCase().indexOf(needle)
  if (at >= 0) return { issue, tier: TITLE, at, field: 'title', snippet: '' }

  for (const [key, name] of PROSE_FIELDS) {
    const text = issue[key]
    if (!text) continue
    const found = text.toLowerCase().indexOf(needle)
    if (found >= 0) {
      return {
        issue,
        tier: PROSE,
        at: found,
        field: name,
        snippet: snippetAround(text, found, needle.length)
      }
    }
  }

  const labels = (issue.labels ?? []).join(' ')
  const inLabels = labels.toLowerCase().indexOf(needle)
  if (inLabels >= 0) {
    return { issue, tier: PROSE, at: inLabels, field: 'labels', snippet: labels }
  }

  return null
}

/* Tier, then how early the match sits, then the newest — and the id last of
   all, so that two tasks updated inside the same second do not trade places
   between one keystroke and the next. */
const compare = (a, b) =>
  a.tier - b.tier ||
  a.at - b.at ||
  (b.issue.updated_at ?? '').localeCompare(a.issue.updated_at ?? '') ||
  a.issue.id.localeCompare(b.issue.id)

export function searchIssues(issues, query, limit = SEARCH_LIMIT) {
  const needle = (query ?? '').trim().toLowerCase()
  if (!needle) return []

  const hits = []
  for (const issue of issues) {
    const hit = match(issue, needle)
    if (hit) hits.push(hit)
  }

  return hits
    .sort(compare)
    .slice(0, limit)
    .map(({ issue, field, snippet }) => ({
      id: issue.id,
      title: issue.title,
      status: issue.status,
      type: issue.issue_type ?? 'task',
      field,
      snippet
    }))
}
