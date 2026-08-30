/* Whether the Changes section of the Git panel is drawn open, which is not the
   same question as whether somebody folded it away.

   Pure, with no Vue and no DOM in it — the family `sectionHeights.js`,
   `gitActions.js` and `changeStatus.js` belong to, and for the reason that
   family exists: a `.vue` file is the one thing no test in this repository can
   reach, so the whole of a rule lives outside the component that draws it.

   **What somebody folded is a preference and lives in `settings.json`; what
   arriving on the tab does to it is neither, and lives nowhere.** Coming back
   to the Git tab with uncommitted work in the tree opens the section, because
   that list is what anybody came for — but saying so by writing
   `changesOpen: true` into the settings would put the stored `false` out of
   reach: it would survive only a clean tree, and "I folded this away" would
   stop meaning anything as a preference. So the opening is an override held
   for the length of one visit, over a stored value it never touches.

   A visit is `{ override, armed }`, and both halves earn their keep.
   `override` is `true` for "this visit has forced the section open" and `null`
   for "follow what is stored" — never `false`, because a visit is only ever
   allowed to open. `armed` is "this visit is owed an opening and git has not
   answered yet", and it exists because the working tree is read asynchronously:
   the tab is very often on screen before `vcs_status` comes back, so a rule
   that looked only at the moment of arrival would be answering from a tree
   nobody had read.

   What counts as arriving is one predicate with no exceptions, and it is the
   caller's half: any move of `project.sideTab` onto `'git'`, by a press or set
   from code, plus the app starting and the project changing with Git already
   the open tab. Those last two are what make the predicate worth stating at
   all — no line in this app sets that tab to `'git'` today, every programmatic
   switch there is goes to `'agents'` — and a rule that told a press apart from
   an assignment would be one visible nowhere on screen. */

/** Before any visit, and what a visit spent on a clean tree comes back to: the
 *  stored fold, drawn exactly as it is stored. */
export const NO_VISIT = Object.freeze({ override: null, armed: false })

/**
 * What the panel is handed as `changesOpen`: the visit's override where there
 * is one, and the stored fold otherwise.
 *
 * The stored value is passed through as it was given rather than coerced, so a
 * caller with a hole where its settings should be — every gallery frame — still
 * reaches `GitPanel`'s own default instead of being folded shut by this file.
 */
export function changesVisible(stored, visit) {
  return visit?.override ?? stored
}

/**
 * The count a visit may be decided from, or `null` for "nothing about here".
 *
 * `store` is three fields of `stores/vcs.js`, read at the moment of arrival:
 * `{ project, loading, count }`, where `count` is that store's own
 * `dirtyCount`. `path` is the project being arrived at.
 *
 * **The count is handed in and never derived here, and that is deliberate.**
 * `dirtyCount` in the store against the changes caption in `GitPanel.vue` are
 * already two spellings of one rule, held to the promise that the number in the
 * status footer is the rows of the panel's list; a `tree.changes.length` in this
 * file would be a third, and the first to drift. So this function filters a
 * count rather than computing one, and `tree` is not among its fields at all —
 * `dirtyCount` is `null` exactly when `tree` is, so an unread tree already
 * arrives here as an unread count and nothing is lost by not looking.
 *
 * What it is guarding against is a count about **somewhere else**. `moveTo`
 * sets the active project synchronously and only reaches `loadRepos` after an
 * awaited layout read, and `loadRepos` deliberately does not clear the tree, so
 * a switch reaches this with the departing project's changes still in the
 * store. `project` catches that window. `loading` catches the narrower one
 * behind it: `loadRepos` claims the new project before its first `await` and
 * holds `loading` across `vcs_repos` → `selectRepo` → `loadStatus`, so between
 * those two moments the project matches while the tree in hand is still the
 * previous one. Both were measured against the real defect rather than
 * imagined — the project term alone leaves a section drawn open over an empty
 * list.
 *
 * Answering `null` is always the safe way to be wrong: it arms the visit, which
 * costs a wait. Answering with somebody else's number spends it, and a visit
 * spent cannot be given back until the person leaves the tab and returns.
 */
export function answeredCount({ project, loading, count }, path) {
  if (project !== path || loading) return null
  return count ?? null
}

/**
 * What arriving on the Git tab leaves behind.
 *
 * `dirty` is how many uncommitted files the selected repository has — the
 * `dirtyCount` the status footer already reads out of `stores/vcs.js`, `null` for a
 * tree that has not been read or could not be. Taking that number rather than
 * the tree itself is deliberate: "is the tree known, and is there anything in
 * it" is the whole of what this file needs to know, it is already spelled once
 * in the store, and a second spelling of one rule is the half that drifts.
 *
 * **`null` also means a count about somewhere else**, which is why the caller
 * reaches this through `answeredCount` above rather than handing over whatever
 * the store happens to hold: a count about another project is not a late answer
 * but a wrong one.
 *
 * The answer is very often already in by the time somebody arrives — the tab
 * was open a minute ago, or this is a second visit — which is why this is the
 * arming and the first answer in one call rather than a rule of its own.
 */
export function enterGitTab(dirty) {
  return gitAnswered({ override: null, armed: true }, dirty)
}

/**
 * What git answering does to the visit.
 *
 * **The first known answer of a visit settles it and nothing after it counts.**
 * A tree with changes turns the override on; a clean one leaves the stored fold
 * exactly as it was; either way the visit is spent. That is what keeps a
 * refresh — the window regaining focus, or the panel's own refresh button,
 * pressed by somebody already sitting on the tab — from unfolding a section
 * they folded a moment ago, and it is the same rule that leaves a tree which
 * goes dirty mid-visit drawing nothing new.
 *
 * An answer that is not one leaves the visit armed: `null` is a tree that could
 * not be read, and not knowing what is in it is not evidence that nothing is —
 * the opposition `dirtyCount` keeps by being `null` and never `0`.
 */
export function gitAnswered(visit, dirty) {
  if (!visit?.armed) return visit ?? NO_VISIT
  if (dirty === null || dirty === undefined) return visit
  return dirty > 0 ? { override: true, armed: false } : NO_VISIT
}

/**
 * What a press on the Changes caption leaves behind: the fold to store, and the
 * visit that outlives the press.
 *
 * **It stores the inverse of what is on screen, not the inverse of what is
 * stored**, which are the same thing everywhere except the one case this
 * feature created: a stored `false` under a section the visit has forced open.
 * Inverting the stored value there writes `true` and folds nothing — the
 * section is open because of the override, which is still standing — so the
 * first press would do nothing visible and the second would finally fold it.
 * Somebody folding what they can see gets it on the first press.
 *
 * The press ends the visit's claim in both directions: the override is dropped,
 * and so is a pending arm. A visit may open the section on the way in; it may
 * not reopen it over a decision taken since, which is what an in-flight
 * `vcs_status` landing a moment after the press would otherwise do.
 */
export function toggleChanges(stored, visit) {
  return { changesOpen: !changesVisible(stored, visit), visit: NO_VISIT }
}
