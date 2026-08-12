/* Which branch the run dialog opens on.

   Three steps in one order: what this project was left at last time, then the
   project's own `[defaults].target_branch`, then whatever the list puts first —
   which is the branch most recently worked on, because `target_branches` orders
   by the reflog, and across several repositories by the most recent touch in
   any of them.

   A name that is not in the list is skipped rather than offered: a branch
   deleted since it was remembered would otherwise sit in the field as an option
   that fails on the first merge. Skipping is silent for the same reason it is
   silent everywhere else here — the list is the truth, and a stale name in
   settings is not worth a warning.

   Pure and outside the component on purpose. A `.vue` file is the one thing no
   test in this repository can reach, and this is the whole of the rule the field
   is filled by; the defect it was written for (smetana-6gs, smetana-o8r) was not
   the rule being wrong but it running exactly once, against a list that had not
   arrived yet.

   What the list holds is `{ name, missing_in }` and no longer a bare string,
   because "does this branch exist" stopped having one answer per project: the
   worker asks every repository `[project].repos` names, and a branch can be in
   three of four. All three rules here read that same record, which is what
   keeps the field, the create flag and the fill from disagreeing about what a
   partial branch is. */
export function pickBranch(branches, remembered, configured) {
  const names = (Array.isArray(branches) ? branches : []).map((b) => b?.name).filter(Boolean)
  const named = [remembered, configured].find((name) => name && names.includes(name))
  return named ?? names[0] ?? ''
}

/* Whether choosing this name means a branch has to be cut somewhere.

   A name the list does not carry at all and a name it carries short of a
   repository are the same answer, and that is the whole of the defect this was
   written for: while "new" meant "not in the list", `develop` — present in all
   four repositories a run touches — came back as new because the list was the
   containing folder's, and the prompt then told the agent to cut a branch that
   already had its own history.

   An absent list answers yes. It is the honest direction: the run then carries
   permission it may not need, where the opposite is a `provisioning` STOP in
   the first batch of a run nobody is watching. */
export function needsCutting(branches, name) {
  const found = (Array.isArray(branches) ? branches : []).find((b) => b?.name === name)
  return !found || (found.missing_in?.length ?? 0) > 0
}

/* The options the field draws: the two groups under captions, each partial row
   saying which repositories it is short of.

   The order inside a group is the one it was given — the worker sorted it by
   `by_recency` across repositories, and a second ordering here could only
   disagree with that one. What the group split reads off is `missing_in` being
   empty, which is the same fact `needsCutting` reads.

   **No captions at all when nothing is partial**, which is every
   single-repository project and therefore the common case: a caption over the
   entire list names nothing, and this field looks exactly as it always has.

   **And never a caption over an empty group**, which is the same rule read the
   other way: a caption names a group, and where there is no group there is
   nothing to name. Repositories sitting on `main` and `master` with nothing else
   are exactly that — every branch is partial, because the list always carries
   each repository's own HEAD — and an `Everywhere` heading nothing followed was
   pruned again downstream by `Dropdown`'s own filter, leaving the field's rows
   and its cursor counting different lists.

   The note is always "not in …" and never "only in …", and that is forced
   rather than chosen: `BranchOption` carries the repositories a branch is
   missing from, and the front end never learns the full list, so "only in" is
   not derivable here. */
export function branchOptions(branches) {
  const list = Array.isArray(branches) ? branches : []
  const complete = list.filter((b) => !b.missing_in?.length)
  const partial = list.filter((b) => b.missing_in?.length)
  const plain = (b) => ({ value: b.name, label: b.name })
  if (!partial.length) return complete.map(plain)
  const rows = []
  if (complete.length) rows.push({ header: true, label: 'Everywhere' }, ...complete.map(plain))
  rows.push(
    { header: true, label: 'Not everywhere' },
    ...partial.map((b) => ({ ...plain(b), note: `not in ${b.missing_in.join(', ')}` }))
  )
  return rows
}
