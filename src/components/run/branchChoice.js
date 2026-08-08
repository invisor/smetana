/* Which branch the run dialog opens on.

   Three steps in one order: what this project was left at last time, then the
   project's own `[defaults].target_branch`, then whatever the list puts first —
   which is the branch most recently worked on, because `git_branches` orders by
   the reflog rather than alphabetically.

   A name that is not in the list is skipped rather than offered: a branch
   deleted since it was remembered would otherwise sit in the field as an option
   that fails on the first merge. Skipping is silent for the same reason it is
   silent everywhere else here — the list is the truth, and a stale name in
   settings is not worth a warning.

   Pure and outside the component on purpose. A `.vue` file is the one thing no
   test in this repository can reach, and this is the whole of the rule the field
   is filled by; the defect it was written for (smetana-6gs, smetana-o8r) was not
   the rule being wrong but it running exactly once, against a list that had not
   arrived yet. */
export function pickBranch(branches, remembered, configured) {
  const list = Array.isArray(branches) ? branches : []
  const named = [remembered, configured].find((name) => name && list.includes(name))
  return named ?? list[0] ?? ''
}
