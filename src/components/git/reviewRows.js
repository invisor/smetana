/* What the branch-review window holds, and what a press of Review turns it into.

   The `branchPicker.js` / `branchMenu.js` / `branchTree.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — a `.vue` file
   is the one thing no test in this repository can reach, so a rule left inside
   the component is a rule nothing checks.

   **One pair for the project, and a row that differs keeps its own.** This used
   to be a table where a row *was* a pair: one repository, its own reference
   branch and its own branch to check, four controls apiece. That shape held the
   thing that matters — the number of bases is always the number of branches
   under review — and paid for it with a wall of controls that grew with the
   project, four dropdowns deep on every row of five. The shape here holds the
   same invariant for the same reason and spends one pair of controls on the
   whole window:

     base:      { ref, remote }
     head:      { ref, remote } | null
     repoIds:   string[]                        the rows, in the order shown
     overrides: { [repoId]: { base, head } }     a row that differs, whole
     manual:    string[]                        the rows somebody named

   **A pair is still indivisible.** There is no arrangement of this form with
   four bases and three branches to check, because neither the rule nor an
   override can be half set: both are an object of two sides, and an override is
   made by copying the rule rather than by starting an empty one. That is a
   property of the shape rather than a check on the way out, and the difference
   is the same as it always was — a rule can be forgotten and a shape cannot.

   `manual` is the one field that is neither the pair nor the rows: which of
   those rows somebody put there by hand. It was derived at first — a row
   without the rule's branch could only have been named by a person — and that
   answer changes under a foot it should not, since the rule's branch changes.
   A repository added by hand and then reviewed against another branch lost its
   badge and, with it, the `x` that is the only way a row ever leaves this
   table. Provenance is a fact about what somebody did, so it is recorded rather
   than inferred from a state that moves.

   `remote` is the whole of what `local` and `origin` mean here, and it is a
   flag on a side rather than a control of its own: the window draws it as the
   muted `origin/` in front of the name, and `refOf` below is where it becomes
   the ref git is handed. `origin` and no other remote — there is no notion of a
   second one anywhere in this app, and inventing one for a case nobody has
   asked about would be a second vocabulary.

   **The exit is unchanged.** `reviewPairs` answers `{ repo, base, head }`
   apiece, which is `src-tauri/src/agents/mod.rs`' `ReviewPair`, with the same
   fields and the same reading of `origin/`. What changed is where a person's
   answer is kept on the way to it. */
import { pickBranch } from '../run/branchChoice.js'
import { shortAge } from './branchPicker.js'

/* The one remote this app knows, and the prefix a side wears when it means
   that remote rather than the local branch. */
export const ORIGIN = 'origin'

const list = (value) => (Array.isArray(value) ? value : [])
const has = (object, key) =>
  Boolean(object) && Object.prototype.hasOwnProperty.call(object, key)

/* One side of a pair as git spells it: `main` for the local branch,
   `origin/main` for what the remote has. Resolved here, once, so that nothing
   downstream re-reads a branch list or guesses at a remote. */
export function refOf(side) {
  const name = typeof side?.ref === 'string' ? side.ref.trim() : ''
  if (!name) return ''
  return side.remote ? `${ORIGIN}/${name}` : name
}

/* The same side split for drawing: the prefix apart from the name, because the
   window sets the prefix in the muted colour and the name in the ordinary one.
   A side nobody has answered is two empty strings rather than null, so the
   caller has nothing to branch on. */
export function sideLabel(side) {
  const name = typeof side?.ref === 'string' ? side.ref.trim() : ''
  if (!name) return { prefix: '', ref: '' }
  return { prefix: side.remote ? `${ORIGIN}/` : '', ref: name }
}

/* And a whole pair, for the one cell that draws one: an override's own
   `main → infra/4nsa-remote-branches`. */
export function pairLabel(pair) {
  return { base: sideLabel(pair?.base), head: sideLabel(pair?.head) }
}

/* Whether a repository has the branch a side names.

   `branches` is `target_branches`' answer — `{ name, missing_in }` apiece, said
   once for the whole project — and reading the absence rather than the presence
   is that command's own shape: an empty `missing_in` is the ordinary case and
   there is nothing to do with it.

   A side meaning `origin` is asked of the remote list first, because a branch
   that lives only on the server is exactly the case where the local answer is
   wrong. `remote` is keyed by repository path and is filled one repository at a
   time as the reads land, so a repository whose list has not arrived yet has no
   entry at all — and that falls through to the local answer rather than to
   "no". A branch missing from a list nobody has read is not a fact about the
   repository. */
export function hasBranch(repo, head, context = {}) {
  const { branches = [], remote = {} } = context
  const name = typeof head?.ref === 'string' ? head.ref.trim() : ''
  if (!repo?.path || !name) return false
  if (head.remote) {
    const known = remote?.[repo.path]
    if (Array.isArray(known)) return known.includes(name)
  }
  const option = list(branches).find((branch) => branch?.name === name)
  return Boolean(option) && !list(option.missing_in).includes(repo.name)
}

/* The rows a branch fills the table with: every repository of the project that
   has it, by path, in the order the project lists them. */
export function repoIdsWith(repos, head, context = {}) {
  return list(repos)
    .filter((repo) => hasBranch(repo, head, context))
    .map((repo) => repo.path)
}

/* The form a window opens with.

   From a branch row's menu the name is known, and the table is every repository
   that has a branch of that name. From `New review` it is not: the head is
   `null`, there are no rows at all, and `Review` is refused until somebody picks
   one — at which point `withPick` below fills the table with the very same rule.
   The two doors differ in what they start with and in nothing else.

   The base is `branchChoice.js`'s existing order — what this project was left
   at, then `[defaults].target_branch`, then the top of the list — because the
   run dialog answers exactly this question one screen over, and a second order
   would be a second answer to it. It starts local: what a person was looking at
   is the branch on this machine, and `origin` is the deliberate choice. */
export function reviewForm(repos, branch, options = {}) {
  const { branches = [], remote = {}, remembered = null, configured = null } = options
  const base = { ref: pickBranch(branches, remembered, configured), remote: false }
  const name = typeof branch === 'string' ? branch.trim() : ''
  const head = name ? { ref: name, remote: false } : null
  return {
    base,
    head,
    repoIds: head ? repoIdsWith(repos, head, { branches, remote }) : [],
    overrides: {},
    manual: []
  }
}

/* The pair a row is actually reviewed with: its own if it has one, the
   project's rule otherwise. The one line the whole model rests on. */
export function pairOf(form, repoId) {
  if (has(form?.overrides, repoId)) return form.overrides[repoId]
  return { base: form?.base ?? null, head: form?.head ?? null }
}

export function isOverride(form, repoId) {
  return has(form?.overrides, repoId)
}

/* The rows that differ, in the order the table draws them rather than in
   whatever order the object was written in. */
export function overrideIds(form) {
  return list(form?.repoIds).filter((id) => isOverride(form, id))
}

/* Whether a row is one somebody put there by hand, which is what earns it the
   `man` badge and the `x` that takes it out again.

   Read off the form and deliberately not worked out from the branch. It was
   worked out at first — a row without the rule's branch could only have been
   named by a person — and the answer moved the moment the rule's head did: a
   hand-added row silently became an ordinary one, lost its `x`, and could not
   be taken out of the review at all. What a person did does not stop being true
   when they change their mind about the branch. */
export function isManual(form, repoId) {
  return list(form?.manual).includes(repoId)
}

/* ---- editing the form --------------------------------------------------- */
/* Every one of these answers a new form rather than changing the one it was
   given: the component holds it in a `ref`, and a reactive object edited in
   place is the version where half a change is on screen. */

const withOverrides = (form, overrides) => ({ ...form, overrides })

/* A branch picked, for whichever side of whichever pair the list was opened
   for. `picker` is the component's own `{ side, repoId }` — `repoId` null for
   the project's rule, a path for one row's override.

   The one thing that happens here beyond writing a side: **a branch on the
   rule's checked side rebuilds the rule's rows.** That is the `New review` door
   at its first pick, and it is the same movement every time afterwards, which
   is what stops the table from claiming things about itself that stopped being
   true. It is here rather than in the component because it is the same rule
   `reviewForm` opens the other door with.

   What the rebuild leaves alone is the whole of its manners: a row that differs
   carries its own pair, and a row somebody added by hand is a decision of
   theirs — the rule's head does not reach either, so neither moves. Everything
   else is the rule's own, and a rule-following row in a repository that has no
   such branch is not a smaller review, it is a pair git would refuse. It leaves
   the table and `reviewNotes` names it, which is the sentence this window
   already promises: `No such branch in extension, docs. They are left out of
   the review.`

   The order is the reading order: rows that stay keep their place, and rows the
   new branch brings in arrive at the end rather than shuffling the table under
   somebody's eye. */
export function withPick(form, picker, value, context = {}) {
  const side = picker?.side === 'base' ? 'base' : 'head'
  const repoId = picker?.repoId ?? null
  if (repoId != null) {
    const pair = pairOf(form, repoId)
    return withOverrides(form, { ...form?.overrides, [repoId]: { ...pair, [side]: value } })
  }
  const next = { ...form, [side]: value }
  if (side === 'head') next.repoIds = refill(form, value, context)
  return next
}

/* Which of the rows the form already holds a rule with this checked side still
   reaches, in the order they were in.

   **This is the one decision about whether a row may follow the rule**, and it
   is a function of its own because two different acts ask it: a branch picked
   on the rule's checked side, and a row put back on the rule with `undo-2`.
   The second is where it was missing — the override went and the row stayed,
   claiming `follows the rule` in a repository that has no such branch, with the
   notes block silent about it because a row in the table is not "left out" and
   `reviewPairs` sending the agent a head that does not exist there. A rule the
   repository cannot follow is not a smaller review; it is a pair git refuses,
   inside a terminal, with nothing in the window having said so.

   A row stays if it is not the rule's to decide — an override, or one somebody
   added by hand — or if the repository has the branch. */
function reached(form, found) {
  return list(form?.repoIds).filter(
    (id) => isOverride(form, id) || isManual(form, id) || found.includes(id)
  )
}

/* The rows the rule's new branch leaves: the ones it still reaches, and then
   the ones it brings in. Only a change to the rule's own checked side adds
   rows — `undo-2` spends the removing half of this alone, because taking one
   row back to the rule must not reach a neighbour. */
function refill(form, head, context = {}) {
  const found = repoIdsWith(context.repos, head, context)
  const kept = reached(form, found)
  return [...kept, ...found.filter((id) => !kept.includes(id))]
}

/* A row told to differ: the rule frozen into it, so that changing the rule
   afterwards leaves this row where it was. A copy of the pair and never half of
   one — this is the operation the invariant would otherwise be broken by.

   **A row differs on its head and never on its base.** The whole pair is
   copied, so an override *carries* a base and a caller could set one; nothing
   in the window ever opens the list on a row's base, and both of the calls that
   name a repository — the pencil, and a repository added by hand — ask for the
   checked side. That is the shape rather than an omission: a row differs on
   what is being checked in it, not on what it is checked against, and the
   window is a form for one comparison rather than for a table of unrelated
   ones. */
export function withOverride(form, repoId) {
  if (!repoId || form?.head == null || isOverride(form, repoId)) return form
  const pair = pairOf(form, repoId)
  return withOverrides(form, { ...form?.overrides, [repoId]: { ...pair } })
}

/* The pair a row kept of its own, dropped. Only this row: the object is rebuilt
   without the one key, so no neighbour is touched. */
const dropOverride = (form, repoId) => {
  if (!has(form?.overrides, repoId)) return form
  const overrides = {}
  for (const [id, pair] of Object.entries(form.overrides)) {
    if (id !== repoId) overrides[id] = pair
  }
  return withOverrides(form, overrides)
}

/* And back to the rule — **if the rule reaches it**. A row whose repository has
   no such branch leaves the table with its override, and `reviewNotes` names it
   under the table like any other repository the review is not in. Refusing the
   action instead was the version thrown away: it leaves a row an override with
   no way back, and it puts the decision in the template, which is the one place
   this whole family exists to keep it out of.

   **A row somebody added by hand has no rule to go back to**, and this answers
   with the form untouched rather than leaving that to the order of three icon
   buttons in a template. The reason it was in the table at all is that the
   rule's branch was not in its repository, so `follows the rule` is a thing it
   can never truthfully say; the only way out of the table for one of those is
   the `x`, which is `withoutRepo` below. Making it unrepresentable here is what
   keeps it from coming back the next time somebody reorders those buttons. */
export function withoutOverride(form, repoId, context = {}) {
  if (!isOverride(form, repoId) || isManual(form, repoId)) return form
  const next = dropOverride(form, repoId)
  /* With nothing to ask about the project, the override goes and the table
     stays as it is. A caller that names no repositories has not said that no
     repository has the branch — it has said nothing — and reading the silence
     as "nowhere" would empty the whole table of rule-following rows. The same
     reading `reviewForm` takes of a branch list that has not landed. */
  if (!list(context.repos).length) return next
  return {
    ...next,
    repoIds: reached(next, repoIdsWith(context.repos, next.head, context))
  }
}

/* A repository added by hand, at the end of the table.

   It arrives as an override, and that is not a formality: the reason it was not
   in the table is that the rule's branch is not in it, so following the rule is
   the one thing this row cannot do. The pair it starts from is the rule's, which
   is a whole pair — the window then opens the branch list on its checked side so
   the name it goes by here can be given. */
export function withRepo(form, repoId) {
  if (!repoId || form?.head == null || list(form?.repoIds).includes(repoId)) return form
  const added = {
    ...form,
    repoIds: [...list(form.repoIds), repoId],
    manual: [...list(form.manual), repoId]
  }
  return withOverride(added, repoId)
}

/* And out again, taking its override and its provenance with it: either left
   behind for a row that is not in the table would come back the next time the
   same repository was added — a pair somebody had abandoned, or a badge on a
   row the rule put there. */
export function withoutRepo(form, repoId) {
  const dropped = {
    ...form,
    repoIds: list(form?.repoIds).filter((id) => id !== repoId),
    manual: list(form?.manual).filter((id) => id !== repoId)
  }
  /* `dropOverride` and not `withoutOverride`: the row is already gone from the
     table, so there is nothing here to ask the rule about, and the guarded door
     would refuse this one anyway — a hand-added row is exactly what it turns
     away. */
  return dropOverride(dropped, repoId)
}

/* ---- what the form answers ---------------------------------------------- */

/* Whether there is anything to review. A branch to check, and at least one row
   to check it in. Nothing here asks whether a pair is whole, and that is the
   point of the shape: there is no way to build a half one. */
export function canReview(form) {
  return Boolean(form?.head) && list(form?.repoIds).length > 0
}

/* The form as the intent carries it: one `ReviewPair` per row, refs resolved.
   `src-tauri/src/agents/mod.rs` is the other end of this shape, and it has not
   moved. */
export function reviewPairs(form) {
  return list(form?.repoIds).map((repoId) => {
    const pair = pairOf(form, repoId)
    return { repo: repoId, base: refOf(pair.base), head: refOf(pair.head) }
  })
}

/* **The base is not checked against any repository, and that is a decision.**
   Everything in this module that asks whether a branch is there — `hasBranch`,
   `repoIdsWith`, `reached`, `missingRepos` — asks it about the *head*, and so
   does every sentence the window draws: `No such branch in extension, docs`,
   `follows the rule`, `using origin from 2h ago`. A base that exists in one
   repository of a project is sent as the base for all of them, and a repository
   without it is not named anywhere.

   The head is what a review is *about* — somebody picked that branch, it is
   what the report is titled after, and its absence takes the whole row with it
   — where the base is what the difference is read against and is in practice a
   long-lived branch every repository has. The window's copy is head-only by
   design (the spec's own three sentences say nothing about a base), and pairs
   go to git either way: a base git cannot resolve fails in the agent's terminal
   in git's own words, which is the same answer the review would give about any
   ref that moved between the window closing and the session starting.

   Extending the clause to the base is a change to what this window says, not a
   defect in what it does. It is recorded here so the gap is a decision somebody
   can re-open rather than an omission somebody has to discover.

   Which repositories have to be fetched before any of this is read.

   Any row with `origin` on either side of its effective pair: `origin/main` is
   only as current as the last fetch, and a review of a week-old commit drawn
   under the name of a branch somebody pushed this morning is the one way this
   feature fails with nothing on screen saying so. One entry per repository, in
   the order the rows are in. */
export function fetchTargets(form) {
  const wanted = []
  for (const repoId of list(form?.repoIds)) {
    const pair = pairOf(form, repoId)
    const origin = Boolean(pair?.base?.remote) || Boolean(pair?.head?.remote)
    if (origin && !wanted.includes(repoId)) wanted.push(repoId)
  }
  return wanted
}

/* Which of the repositories that were fetched did not answer.

   `reached` is what `Promise.all` handed back over `targets`, one verdict
   apiece and in that order, and joining the two is the whole of this function.
   It is a rule rather than a line in the view because **two different readers
   are drawn from what it answers**: the note in the window and the toast behind
   it, and the list that rides into the intent so the report can say so about
   itself. One list read twice cannot disagree with itself; two walks of the
   same array could, and the disagreement would be invisible — a review whose
   report says origin was current when the window had just said it was not.

   It answers in **paths**, which is what a row is keyed by, what a `ReviewPair`
   names a repository by and what the prompt lists them in. The names the window
   draws are that same list mapped through the project's repositories, which is
   a rendering of this answer rather than a second one. */
export function fetchFailures(targets, reached) {
  const verdicts = list(reached)
  return list(targets).filter((_, at) => !verdicts[at])
}

/* The branches of one repository, out of the project-wide answer.

   `target_branches` is asked once for the whole project and says which
   repositories each branch is short of, so a row's own list is that answer
   filtered by this repository's name rather than a second read per row. The
   records travel whole rather than as names, because the list this fills draws
   an age and a repository count off them.

   **`missing_in` is emptied on the way out, and that is the scope rather than a
   loss of information.** The field says where a branch is absent *across the
   project*, and `branchPicker.js` turns it into `local · 4 repos` by subtracting
   it from however many repositories the list is drawn against. A list drawn for
   one repository is drawn against one, so a branch absent from two others came
   out as `local · 0 repos` — the list flatly denying that any repository has the
   branch, directly under a field saying that branch is what is being checked.
   Inside a list about one repository that has the branch, the honest answer is
   that it is absent from nowhere. */
export function branchesIn(branches, repoName) {
  return list(branches)
    .filter((branch) => branch?.name && !list(branch.missing_in).includes(repoName))
    .map((branch) => ({ ...branch, missing_in: [] }))
}

/* How fresh the whole project's idea of `origin` is: the oldest of the fetch
   times, in the epoch seconds `vcs_last_fetch` answers in.

   The branch list opened for the project's rule draws one age against every
   `origin` row, and one number has to stand for several repositories. The
   oldest is the only one that cannot mislead — a pair set for every repository
   is as stale as the least recently fetched of them — and the direction is the
   safe one: an age that is too old says "ask again", where one that is too
   fresh promises refs are newer than they are.

   A repository nobody has ever fetched into takes the answer away entirely
   rather than being skipped. There is no honest number for a project holding
   one, and the list then says `origin` with nothing after it, which is what a
   single repository in that state already draws. */
export function oldestFetch(paths, fetchedAt = {}) {
  let oldest = null
  for (const path of list(paths)) {
    const at = fetchedAt?.[path]
    if (!Number.isFinite(at)) return null
    if (oldest === null || at < oldest) oldest = at
  }
  return oldest
}

/* The repositories that are in this project and have no such branch — the ones
   the review leaves out. Anything already in the table is not among them, which
   is what keeps a note from being about a row directly above it.

   Nothing at all until there is a branch to be missing. Without that clause the
   `New review` door would open by naming every repository of the project under
   a sentence about a branch nobody has chosen yet. */
export function missingRepos(repos, form, context = {}) {
  if (!form?.head) return []
  const rows = list(form?.repoIds)
  return list(repos).filter(
    (repo) => repo?.path && !rows.includes(repo.path) && !hasBranch(repo, form?.head, context)
  )
}

/* What a row is not in this review because of, for the panel `Add a repository`
   opens: it has the branch and simply is not in the table, or it has no such
   branch and whoever adds it will have to say what the branch is called there.
   Both are ordinary, which is why neither is drawn as a refusal. */
export const NOT_IN_REVIEW = 'not in this review'
export const NO_SUCH_BRANCH = 'no such branch — name it by hand'

export function addableRepos(repos, form, context = {}) {
  const rows = list(form?.repoIds)
  return list(repos)
    .filter((repo) => repo?.path && !rows.includes(repo.path))
    .map((repo) => ({
      repo,
      note: hasBranch(repo, form?.head, context) ? NOT_IN_REVIEW : NO_SUCH_BRANCH
    }))
}

/* ---- the words ---------------------------------------------------------- */

/* What the checked side says while it is empty, which is the `New review` door
   at the moment it opens. Lower case and in the muted colour: it is a field
   waiting to be answered rather than a value. */
export const PICK_HEAD = 'pick a branch to check'
/* And what stands where the table will be, for the same door. */
export const WAITING_FOR_BRANCH = 'waiting for a branch'

/* The line under the pair. It names the two fields rather than repeating what
   is in them — they are directly above it — and says what a choice there
   reaches. Without a branch to check it says what will happen instead, because
   an empty table under an unanswered field otherwise reads as a window that
   failed to fill. */
export const RULE_CAPTION = 'base → to check · applies to every repository below'
export const RULE_CAPTION_EMPTY = 'the repositories that have it will fill in below'

export function ruleCaption(form) {
  return form?.head ? RULE_CAPTION : RULE_CAPTION_EMPTY
}

/* The right-hand end of the branch list's footer: what picking there applies
   to. It is the one thing that tells a list opened for the whole project from
   one opened for a single row, and those two do very different things. */
export function pickerScope(repoName = null) {
  return repoName ? `this repository only · ${repoName}` : 'sets every repository below'
}

/* A number and the word that agrees with it. The verb follows the count, always
   and everywhere in this window, because `1 follow the rule` over a row saying
   `follows the rule` is the sort of sentence that makes a person doubt
   everything else on the screen. */
const count = (n, one, many) => `${n} ${n === 1 ? one : many}`

/* The right-hand end of the table's heading: how many rows there are and how
   many of them are following the rule. `all follow the rule` rather than
   `6 follow the rule · 0 differ`, because a count of nothing is a fact nobody
   needs and the sentence is what a person is checking. */
export function tableSummary(form) {
  const rows = list(form?.repoIds).length
  const differ = overrideIds(form).length
  if (!differ) return `${rows} · all follow the rule`
  const following = rows - differ
  return [
    String(rows),
    count(following, 'follows the rule', 'follow the rule'),
    count(differ, 'differs', 'differ')
  ].join(' · ')
}

/* The left-hand end of the footer: what pressing Review would start.

   In busy it is what is happening instead, because the button that said so has
   just gone quiet and the sentence is the only thing left saying why. With no
   branch to check it is `0 pairs`, which is the honest reading of a form nobody
   has filled in — and the number the disabled button is about. */
export function footerSummary(form, options = {}) {
  const { busy = false, notes = 0 } = options
  const rows = list(form?.repoIds).length
  if (busy) return `starting the review session · ${count(rows, 'pair', 'pairs')}`
  if (!form?.head) return '0 pairs'
  const parts = [count(rows, 'pair', 'pairs')]
  const differ = overrideIds(form).length
  if (differ) parts.push(count(differ, 'override', 'overrides'))
  if (notes) parts.push(count(notes, 'note', 'notes'))
  return parts.join(' · ')
}

/* What one row says about itself, on the right of its pair.

   Four states and only one of them is about this window's own doing. A row that
   follows the rule says so, since the alternative — nothing at all — reads as a
   row that has not finished loading. A row that differs says nothing here: its
   pair is drawn in the cell beside this one, which is the whole answer.

   The fetch is the other two. `fetching origin` turns while it runs, and the
   one that matters is the third: a fetch that did not reach the remote leaves
   the review reading the copy of origin already on this disk, which is a fact
   about how old the answer is and **not an error**. It is drawn in the muted
   colour with a triangle and never in red — the saturated range belongs to
   status, and a review that goes ahead over slightly older refs is not a
   failure to colour.

   Without a stamp it says `from before`, matching the note under the table: a
   repository nobody has ever fetched into has no age to give, and `from  ago`
   with a hole in it would be worse than the shorter sentence. */
export const FOLLOWS_THE_RULE = 'follows the rule'
export const FETCHING_ORIGIN = 'fetching origin'

export function rowStatus(state = {}) {
  const { override = false, fetching = false, stale = false, at = null, now = null } = state
  if (fetching) return { text: FETCHING_ORIGIN, icon: 'loader-circle', spin: true }
  if (stale) {
    const age = shortAge(at, now)
    return {
      text: age ? `using origin from ${age} ago` : 'using origin from before',
      icon: 'triangle-alert',
      spin: false
    }
  }
  if (override) return { text: '', icon: '', spin: false }
  return { text: FOLLOWS_THE_RULE, icon: '', spin: false }
}

/* The service messages, as one block of lines rather than three sentences
   loose under the table.

   Each is a glyph, a sentence and the identifiers inside it in mono — the
   sentence is prose and a repository's name is not, and the block is unreadable
   if the two are drawn alike. `parts` is what makes that possible without the
   component knowing any of the words: a run of pieces, each either prose or an
   identifier.

   **None of the three is an error.** A fetch in flight is a wait, a fetch that
   failed is a review going ahead over an older copy of origin, and a repository
   without such a branch is an ordinary fact about a project made of several. So
   the whole block is drawn in the quiet idiom, with no red anywhere in it, and
   `Review` is refused by none of them. */
const prose = (text) => ({ text, mono: false })
const ident = (text) => ({ text, mono: true })

export function reviewNotes(state = {}) {
  const { fetching = [], failed = [], missing = [] } = state
  const notes = []
  const waiting = list(fetching).filter(Boolean)
  if (waiting.length) {
    notes.push({
      key: 'fetching',
      icon: 'loader-circle',
      spin: true,
      parts: [prose(`Fetching origin for ${count(waiting.length, 'repository', 'repositories')}.`)]
    })
  }
  const missed = list(failed).filter(Boolean)
  if (missed.length) {
    notes.push({
      key: 'failed',
      icon: 'triangle-alert',
      spin: false,
      parts: [
        prose('Fetch failed for '),
        ident(missed.join(', ')),
        prose('. The review still runs and reads the copy of origin from before.')
      ]
    })
  }
  const absent = list(missing).filter(Boolean)
  if (absent.length) {
    notes.push({
      key: 'missing',
      icon: 'circle-dashed',
      spin: false,
      parts: [
        prose('No such branch in '),
        ident(absent.join(', ')),
        prose(`. ${absent.length === 1 ? 'It is' : 'They are'} left out of the review.`)
      ]
    })
  }
  return notes
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
