/* What the branch-review window's table holds, and what a press of Review
   turns it into.

   The `branchMenu.js` / `branchTree.js` / `changesFold.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — a `.vue` file
   is the one thing no test in this repository can reach, so a rule left inside
   the component is a rule nothing checks.

   **A row is a pair.** One repository, one reference branch and one branch to
   check, with each side saying whether it means the local branch or what
   `origin` has. What was asked for is that the number of bases always equals
   the number of branches under review, and that is a property of the shape
   rather than a rule checked on the way out: there is no arrangement of this
   table with four bases and three branches, because a base cannot be added
   without the branch beside it. A rule can be forgotten; a shape cannot.

   The table is built here and never in the component, which is what makes both
   doors into the window one piece of code. From a branch row's menu the name is
   known, and the table is every repository that has a branch of that name; from
   `New review` it is not, and the table is one empty row on the repository the
   Git panel is showing — after which picking a name on that row builds the rest
   of the table through this same function. The two doors differ only in what
   they start with. */
import { pickBranch } from '../run/branchChoice.js'

/* Which side of a pair a name is read on. `origin` and no other remote: there
   is no notion of a second one anywhere in this app, and inventing one here for
   a case nobody has asked about would be a second vocabulary. */
export const LOCAL = 'local'
export const ORIGIN = 'origin'

const list = (value) => (Array.isArray(value) ? value : [])

/* The table, and the repositories that had nothing to put in it.

   `repos` is `vcsState.repos` — `{ name, path }` apiece, where the name is the
   one `[project].repos` gives and therefore the same name `missing_in` speaks
   in. `branches` is `target_branches`' answer, `{ name, missing_in }` apiece,
   which is the command that already answers the whole multi-repository question
   — nothing here walks a project a second time.

   A repository the branch is missing from gets no row at all and is named in
   `without` instead. That is not an error and not a broken row: a repository
   without a branch of that name is a fact, said once under the table, and
   somebody who wants it in the review adds it by hand with the name the branch
   goes by there. A name no repository has is the same answer with nothing left
   over — an empty table rather than a table of rows that cannot be reviewed.

   Without a name there is one row, on the repository the panel has selected,
   with the base filled and the checked side empty. The base is
   `branchChoice.js`'s existing order — what this project was left at, then
   `[defaults].target_branch`, then the top of the list — because the run dialog
   answers exactly this question one screen over, and a second order would be a
   second answer to it. */
export function reviewRows(repos, branch, options = {}) {
  const { branches = [], remembered = null, configured = null, selected = null } = options
  const all = list(repos).filter((repo) => repo && repo.path)
  if (!all.length) return { rows: [], without: [] }

  const base = pickBranch(branches, remembered, configured)
  const name = typeof branch === 'string' ? branch.trim() : ''

  if (!name) {
    const repo = all.find((r) => r.path === selected) ?? all[0]
    return { rows: [row(repo, base, '')], without: [] }
  }

  const option = list(branches).find((b) => b?.name === name)
  /* A name `target_branches` has never heard of is missing from every
     repository it walked, which is the honest reading and the one that fills
     the caption: an empty table with nothing said under it would leave somebody
     looking at a window that had simply not worked. */
  const missing = option ? list(option.missing_in) : all.map((repo) => repo.name)
  const has = (repo) => !missing.includes(repo.name)
  return {
    rows: all.filter(has).map((repo) => row(repo, base, name)),
    without: all.filter((repo) => !has(repo)).map((repo) => repo.name)
  }
}

const row = (repo, base, head) => ({
  repo: repo.path,
  name: repo.name,
  base,
  baseSide: LOCAL,
  head,
  headSide: LOCAL
})

/* The local branch names one repository has, out of the project-wide answer.

   `target_branches` is asked once for the whole project and says which
   repositories each branch is short of, so a row's own list is that answer
   filtered by this repository's name rather than a second read per row. What
   `origin` has is the other list and comes from `vcs_remote_branches`, one
   repository at a time: the two are deliberately not merged, since a remote
   branch and a local one of the same name are different things to read a diff
   against, and a name in one and not the other is the ordinary case — a branch
   that lives only on the server, and a branch nobody has ever pushed. */
export function localNames(branches, repoName) {
  return list(branches)
    .filter((b) => b?.name && !list(b.missing_in).includes(repoName))
    .map((b) => b.name)
}

/* Whether there is anything to review. An empty table is the obvious half; a
   row with a side unanswered is the other, and it is the one that happens — a
   row added by hand starts with nothing on its checked side, and so does the
   only row `New review` opens with. Both sides of every row or nothing: a pair
   short of a reference is not a smaller review, it is a repository the report
   would have had to guess about. */
export function canReview(rows) {
  const all = list(rows)
  if (!all.length) return false
  return all.every((r) => Boolean(r?.base) && Boolean(r?.head))
}

/* A side of a row as git spells it: `main` for the local branch, `origin/main`
   for what the remote has. The whole of what `local` and `origin` mean, and
   deliberately nothing cleverer — the choice is resolved here, once, and
   nothing downstream re-reads a branch list or guesses at a remote. */
export function refOf(name, side) {
  if (!name) return ''
  return side === ORIGIN ? `${ORIGIN}/${name}` : name
}

/* The table as the intent carries it: one `ReviewPair` per row, refs resolved.
   `src-tauri/src/agents/mod.rs` is the other end of this shape. */
export function reviewPairs(rows) {
  return list(rows).map((r) => ({
    repo: r.repo,
    base: refOf(r.base, r.baseSide),
    head: refOf(r.head, r.headSide)
  }))
}

/* Which repositories have to be fetched before any of this is read.

   Any row with `origin` on either side: `origin/main` is only as current as the
   last fetch, and a review of a week-old commit drawn under the name of a
   branch somebody pushed this morning is the one way this feature fails with
   nothing on screen saying so. One entry per repository however many of its
   rows ask for it, and the order the rows are in. */
export function fetchTargets(rows) {
  const wanted = []
  for (const r of list(rows)) {
    const origin = r?.baseSide === ORIGIN || r?.headSide === ORIGIN
    if (origin && r?.repo && !wanted.includes(r.repo)) wanted.push(r.repo)
  }
  return wanted
}

const pad = (n) => String(n).padStart(2, '0')

/* Where the report goes, relative to the project and without an extension: the
   agent writes `<path>.md` and `<path>.html`, and the app composes the path
   itself so that the tab it opens afterwards is at somewhere it already knows.

   The date first, so a directory of them reads in the order they were made, and
   the branch last, so a person recognises one. The minute is in it because two
   reviews of one branch in one day is the ordinary case here.

   The name is reduced to `a-z0-9` and hyphens and nothing else, which is not
   tidiness: it lands in a path on somebody's disk, a branch name may hold a
   slash, a space, a hash or a word in another alphabet, and any of those is
   either a directory that was never meant or a filename an OS refuses. A name
   that reduces to nothing at all is `review` rather than an empty tail, since a
   path ending in the minute would be a file named after a clock. */
export function reportPath(branch, at) {
  const when = at instanceof Date && !Number.isNaN(at.getTime()) ? at : new Date()
  const stamp = [
    `${when.getFullYear()}-${pad(when.getMonth() + 1)}-${pad(when.getDate())}`,
    `${pad(when.getHours())}${pad(when.getMinutes())}`
  ].join('-')
  const slug = String(branch ?? '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return `.smetana/reviews/${stamp}-${slug || 'review'}`
}

/* The one line under the table naming the repositories that had no branch of
   that name, and '' when there are none.

   **One sentence for all of them rather than a row apiece**, which is the whole
   of the decision: a repository without such a branch is not a broken row and
   not an error, it is a fact — said once, in one place, where a row each would
   have turned an ordinary state into a table half full of things that cannot be
   reviewed. Somebody who wants one of them in the review adds it by hand, with
   the name the branch goes by there.

   The verb follows the count, because "shared do not have" is the sort of
   sentence that makes a person doubt everything else on the screen. */
export function withoutCaption(names, branch) {
  const all = list(names).filter(Boolean)
  if (!all.length || !branch) return ''
  const verb = all.length === 1 ? 'does not have' : 'do not have'
  return `${all.join(', ')} ${verb} a branch called ${branch}`
}

/* What the window says while it is fetching, and '' when it is not.

   It says it at all because the alternative is the silent failure this whole
   step exists to prevent: without a fetch `origin/main` is as old as whenever
   somebody last asked, and a review of that commit drawn under this morning's
   branch name would be wrong with nothing anywhere admitting it. A wait nobody
   is told about is the other half of the same problem. */
export function fetchingCaption(repos) {
  const n = list(repos).length
  if (!n) return ''
  return `Fetching origin for ${n} ${n === 1 ? 'repository' : 'repositories'}…`
}

/* And what it says when a fetch did not work.

   **A failed fetch does not cancel the review**, which is why this is a
   sentence and not a refusal: what `origin` holds on this machine is still
   readable, it is merely older than the remote, and stopping here would trade a
   review that is slightly behind for no review at all. The sentence is what
   keeps that honest. */
export function fetchFailedCaption(names) {
  const all = list(names).filter(Boolean)
  if (!all.length) return ''
  const verb = all.length === 1 ? 'was' : 'were'
  return `${all.join(', ')} ${verb} not reached — the review reads what origin was last known to have`
}
