/* Branch names as a tree of folders, the way GitLens draws one: everything
   before a slash is a heading, and `feature/holiday-curb-y5bt.8-drop-depot-columns`
   is a row called `holiday-curb-y5bt.8-drop-depot-columns` under a row called
   `feature`.

   Pure, with no Vue and no DOM in it — the family `gitActions.js`,
   `changeStatus.js` and `sectionHeights.js` belong to, and for the reason that
   family exists: a `.vue` file is the one thing no test in this repository can
   reach, so the whole of a rule lives outside the component that draws it.

   **The order is the load-bearing decision.** `BranchList` opens by saying the
   list arrives in `git::by_recency`'s order and is drawn exactly as it arrives,
   because the branch somebody merges into every day is nowhere in particular
   alphabetically. Grouping is a re-sort, so it is done the one way that keeps
   that promise: a folder takes the position of the most recent branch under it,
   and the branches inside it keep the order they came in. What was worked on
   last is still at the top, whether it is a row or a heading.

   **The current branch is lifted out of the tree and drawn first**, whatever
   the reflog says and whatever folder its name puts it in. It is the row the
   section's own Pull and Push are about and the one fact somebody opens the
   panel to read, and a folded `feature/` heading could hide it altogether —
   which is the state this rule exists for. It draws its whole name rather than
   its leaf, since there is no heading above it carrying the prefix, and it is
   left out of the tree below so the list never holds it twice; a folder that
   held nothing else is then not drawn at all.

   **The branches somebody marked are lifted the same way and sit under it.**
   The top block is therefore two groups and not one: the current branch, then
   the favourites, then the tree. Both groups are taken out of what the tree is
   built from, so a marked branch is not drawn twice, its folder's count comes
   down by one, and a folder it was the whole of is not drawn at all — every one
   of those is the current branch's rule, applied to a second reason for being
   lifted. A branch that is both current and marked is one row, the first, with
   the star on it.

   **The order inside the favourites is the order the list arrived in**, which
   is `by_recency`'s, and deliberately not the order they were marked in. This
   panel promises one ordering and this would be a second one inside it — and
   the second would be invisible, since nothing on a row says when it was
   pinned.

   **The top block is two surfaces rather than one hairline.** A lifted row
   carries `block` — `'current'` for the branch the repository is on and
   `'favourite'` for each marked one — and a row of the tree carries none. What
   those are drawn as is `BranchList`'s and not this file's: the current branch
   on `--surface-selected` between two rules, the marked ones on `--surface`
   with a hairline under the last of them, the tree on the canvas under both.
   Three surfaces rather than one line under the last row, because with several
   marked branches the two groups are two things and a single rule under the
   pair of them said only where the tree started.

   The tree is flattened to a single list, exactly as `FileTree.vue` flattens
   its own — one `v-for` over rows carrying their own depth, rather than a
   component recursing into itself. */

const SEPARATOR = '/'

/* Empty segments are dropped rather than honoured, which covers a leading
   slash, a trailing one and the doubled slash in `feature//one` in one line. A
   folder with no name is a heading nobody could point at, and the alternative
   to dropping it is drawing one. */
const segments = (name) => String(name ?? '').split(SEPARATOR).filter(Boolean)

/* The tree, in insertion order at every level.
 *
 * A folder is created the first time a branch passes through it, which is what
 * puts it where its most recent branch was. `folders` is keyed by the whole
 * path and not by the segment: `fix/legacy` and `feature/legacy` are two
 * different headings, and one map keyed by `legacy` would merge them.
 *
 * A branch whose name is also a folder's — git holds a ref in a file and a
 * folder in a directory, so it refuses to have both — is drawn as an ordinary
 * branch row beside the heading. It cannot arrive, and if it does the panel
 * still draws, which is the whole of what this case is for.
 */
function build(branches) {
  const root = []
  const folders = new Map()
  for (const branch of branches ?? []) {
    const parts = segments(branch?.name)
    if (parts.length === 0) continue
    let siblings = root
    let path = ''
    for (let depth = 0; depth < parts.length - 1; depth += 1) {
      path = path ? `${path}${SEPARATOR}${parts[depth]}` : parts[depth]
      let folder = folders.get(path)
      if (!folder) {
        folder = { kind: 'folder', path, label: parts[depth], depth, count: 0, children: [] }
        folders.set(path, folder)
        siblings.push(folder)
      }
      /* Every branch passing through, however deep, and not the immediate
         children: the count is what somebody decides whether to unfold on, and
         a `1` over a heading hiding four would be worse than no count. */
      folder.count += 1
      siblings = folder.children
    }
    siblings.push({
      ...branch,
      kind: 'branch',
      label: parts[parts.length - 1],
      depth: parts.length - 1
    })
  }
  return root
}

/**
 * The rows to draw, top to bottom.
 *
 * `branches` is `vcs_branches`' own list, `expanded` the folder paths that are
 * open and `favorites` the names somebody has pinned, as
 * `settings.project.favoriteBranches` keeps them. A folder row is
 * `{ kind: 'folder', path, label, depth, count, expanded }`; a branch row is
 * the branch itself with `kind`, `label` and `depth` added — the whole `name`
 * travels, because that is what a checkout, a merge and a rebase are given,
 * while `label` is the leaf and all that is drawn.
 *
 * **Three groups.** The current branch, then the branches marked as favourites
 * in the order the list arrived in, then the tree. Both of the first two are at
 * depth 0, draw their whole name as the label and carry `pinned` — they are
 * lifted out of what the tree is built from, so nothing draws them twice. A row
 * whose name is in `favorites` also carries `favorite`, including the current
 * branch when it is marked, which is one row and not two.
 *
 * Every lifted row carries `block`, which says which of those two groups it is
 * in — `'current'` or `'favourite'` — and a row of the tree carries none. The
 * component draws a surface per block from it; the fact being stated is that
 * the branch the repository is on, the branches somebody marked and the list
 * proper are three things and not one.
 *
 * A folded folder leaves its branches out of the list altogether rather than
 * hiding them, which is both the height this buys back and what makes the count
 * on the heading the only thing saying they are there.
 */
export function branchRows(branches, expanded, favorites) {
  const list = branches ?? []
  /* A branch with no name at all is dropped here for the reason `build` drops
     it below — there is no row to draw for it — rather than being lifted to the
     top as an empty one. */
  const named = (branch) => segments(branch?.name).length > 0
  const current = list.find((branch) => branch?.current && named(branch))
  const marked = new Set(favorites ?? [])
  /* Read off the branch list rather than off the stored names, which is what
     keeps the group in `by_recency`'s order and what makes a name the selected
     repository has never heard of draw nothing at all. */
  const pinnedFavorites = list.filter(
    (branch) => branch !== current && named(branch) && marked.has(branch.name)
  )
  const open = new Set(expanded ?? [])
  const rows = []
  const lift = (branch, block) => {
    rows.push({
      ...branch,
      kind: 'branch',
      label: branch.name,
      depth: 0,
      pinned: true,
      block,
      favorite: marked.has(branch.name)
    })
  }
  if (current) lift(current, 'current')
  for (const branch of pinnedFavorites) lift(branch, 'favourite')
  const walk = (nodes) => {
    for (const node of nodes) {
      if (node.kind === 'branch') {
        rows.push(node)
        continue
      }
      const { children, ...folder } = node
      const isOpen = open.has(node.path)
      rows.push({ ...folder, expanded: isOpen })
      if (isOpen) walk(children)
    }
  }
  /* `liftedOut` and never a second copy of its test written out here. The tree
     is by definition what the lifting left over, so the two are one rule: a
     third reason for lifting added to that function has to reach this line
     without anybody remembering to come here, or a row would be drawn twice
     and a folded heading would carry a mark for a row already on screen. */
  walk(build(list.filter((branch) => !liftedOut(branch, favorites))))
  return rows
}

/**
 * Whether a branch is drawn above the tree rather than in it — the current one,
 * or one somebody marked.
 *
 * Here rather than written out at each call site, because two of them are the
 * same question asked about a fold: `tracking.js` has to know which rows a
 * folded heading is *not* hiding, and getting that wrong puts a mark on a
 * heading standing in for a row already on screen.
 */
export function liftedOut(branch, favorites) {
  return Boolean(branch?.current) || (favorites ?? []).includes(branch?.name)
}

/**
 * How many branches may be marked, and it is **`MAX_FAVORITE_BRANCHES` in
 * `src-tauri/src/settings/model.rs`, written out again on this side**.
 *
 * The pair has to be kept in step by hand, and the direction of a divergence is
 * what matters: this side may be stricter than Rust and must never be more
 * permissive. Rust trims a list past its ceiling from the **tail**, so a front
 * end that let a 51st star be added would draw it, persist it for the session,
 * and lose it on the next load with nothing on screen saying why — a mark
 * somebody made, gone, silently. Refusing it here means the star simply does
 * not appear, which is a press that did nothing rather than a press that lied.
 */
export const MAX_FAVORITES = 50

/**
 * The list a press on `Add to favourites` / `Remove from favourites` leaves
 * behind.
 *
 * Pure and here beside `toggleFolder`, for that function's reason: the panel is
 * told what the list became rather than working it out, so the one rule lives
 * where a test can reach it. Always a new array — the caller assigns it into
 * `settings.json`, and a list mutated in place gives the store's watcher
 * nothing to notice.
 *
 * Adding puts the name on the end, which decides nothing about where the row is
 * drawn: `branchRows` reads this as a set and takes its order from the branch
 * list. What the position does say is which name falls off first if the file is
 * ever trimmed at the ceiling — which is also why the ceiling is enforced here
 * rather than left to Rust. Unmarking is never refused: a list already over the
 * ceiling, from a hand-edited file, still has to be reachable to shorten.
 */
export function toggleFavorite(stored, name) {
  const marked = stored ?? []
  if (!name) return [...marked]
  if (marked.includes(name)) return marked.filter((one) => one !== name)
  if (marked.length >= MAX_FAVORITES) return [...marked]
  return [...marked, name]
}

/**
 * The folders the current branch sits in, outermost first.
 *
 * The whole chain and not the innermost folder alone: unfolding `fix/legacy`
 * while `fix` stays folded would leave the current branch inside a heading that
 * is not on screen.
 */
export function currentChain(branches) {
  const current = (branches ?? []).find((branch) => branch?.current)
  const parts = segments(current?.name)
  return parts.slice(0, -1).map((_, at) => parts.slice(0, at + 1).join(SEPARATOR))
}

/**
 * Which folders are open, given what `settings.json` holds.
 *
 * **`null` and `[]` are different states**, the distinction `sectionHeights.js`
 * keeps one file over for a height nobody has dragged. `null` is "nobody has
 * chosen here" and opens the folder the repository's current branch is in;
 * `[]` is somebody having folded them all, and stays folded.
 *
 * That seed used to be about the tick — the current branch was inside the tree
 * and a fold could take it off the screen. It is the first row now whatever is
 * folded, so what is left of the argument is the rest of that folder: the
 * branches beside the one being worked on are the ones most likely to be
 * wanted next. Where a folder held nothing but the current branch it is not
 * drawn at all, and the seed names a heading that is not there — harmless, and
 * the alternative is a second rule saying which folders still exist.
 *
 * After that the stored list rules absolutely, and a checkout does not reopen
 * anything: the only way to press a branch row is to see it, so a branch
 * checked out from this panel was in a folder that was open at the time. A
 * branch switched to in a terminal can land inside a folded heading, and that
 * heading is folded because somebody folded it.
 */
export function expandedFolders(stored, branches) {
  return stored ?? currentChain(branches)
}

/**
 * The list a press on one folder leaves behind — resolved here rather than in
 * the component, so the seed above is written out whole on the first press and
 * `[]` can actually be reached.
 *
 * Always a new array: the caller assigns it into `settings.json`, and a list
 * mutated in place gives the store's watcher nothing to notice.
 */
export function toggleFolder(stored, branches, path) {
  const open = expandedFolders(stored, branches)
  return open.includes(path) ? open.filter((folder) => folder !== path) : [...open, path]
}

/* The two sides this section is drawn as, by id, and the one a project starts
   on.

   **A closed list duplicated across the IPC boundary**, exactly as `SIDE_TABS`
   and `RIGHT_TABS` are one column over: `BRANCH_TABS` in
   `src-tauri/src/settings/model.rs` holds the same two words, and
   `ProjectState::validate` rewrites anything else back to `local` with nothing
   logged and nothing on screen. A third tab added only here would work all
   session and come back as Local after a restart. The default is written out on
   both sides too, and a third time in `src/stores/settings.js`'s project
   defaults — a key missing from that object is a key the defaults layer cannot
   clear, so one project's choice would follow somebody into the next project's
   panel.

   Here rather than in `GitPanel.vue`, where the labelled rows that draw them
   live, for this family's reason: a `.vue` file is the one thing no test in this
   repository can reach, and the closed list is exactly the half worth pinning.
*/
export const BRANCH_TABS = ['local', 'origin']
export const DEFAULT_BRANCH_TAB = 'local'

/**
 * The branches of the Origin tab as entries, before anything is done with them.
 *
 * One list, two readers: `originBranchRows` builds its tree out of it, and the
 * filter runs over it directly. Both need the same two facts about a name, and
 * both would otherwise work them out for themselves — `hasLocal` is what
 * chooses the row's glyph, its menu and the verb behind its double click, so a
 * second copy of it would be a filtered row that draws `cloud` and checks out
 * as an ordinary switch, or the other way about.
 *
 * `current` rides beside it for the tick, and it can only ever be true of an
 * entry `hasLocal` is true of: the repository is standing on a branch it has.
 */
export function originBranches(remote, local) {
  const held = new Set((local ?? []).map((branch) => branch?.name).filter(Boolean))
  const current = (local ?? []).find((branch) => branch?.current)?.name ?? null
  return (remote ?? [])
    .filter(Boolean)
    .map((name) => ({ name, hasLocal: held.has(name), current: name === current }))
}

/**
 * The rows of the Origin tab, top to bottom, and `[]` when `origin` has nothing.
 *
 * `remote` is the plain names `vcs_remote_branches` answered — alphabetical,
 * with `origin/` and `origin/HEAD` already off — `local` is `vcs_branches`' own
 * list, and `expanded` is `settings.project.remoteBranchFolders`.
 *
 * **The whole of `origin`, including the branches this repository already
 * has.** That is a change of mind about `remoteBranchRows`, which this replaces,
 * and it follows from the shape rather than from taste. A group at the foot of
 * the local list was obliged to answer the narrower question — what is on the
 * server that is not here yet — or it would have been a duplicate of the list
 * above it. A tab of its own is called `Origin` and shows `origin`; a list
 * missing half its branches would be lying in its own title. What that group
 * cost is what the tab exists to undo: in a repository with 346 local branches
 * the heading sat 346 rows down the scroller, and in one where `origin` holds
 * nothing extra it was not drawn at all — which on screen is indistinguishable
 * from broken.
 *
 * **The paths carry no group prefix**, because there is no heading above them
 * any more: a folder here is `feature`, not `origin/feature`. The field stays
 * separate from `branchFolders` for the reason it was made separate — `feature`
 * on this tab and `feature` on the other are two different rows — and no
 * migration is needed for the entries the old shape left behind: an entry that
 * matches nothing means a folder that is folded, which is the default anyway.
 *
 * **The order is left exactly as it arrived**, which is alphabetical. The Local
 * tab is ordered by reflog, because what somebody worked on here most recently
 * is what they are about to want; a remote-tracking ref's reflog says when this
 * machine last *fetched* it, which is a fact about the fetch and not about
 * anybody's work. Nothing is lifted to the top either — no current branch, no
 * favourites — so the alphabet holds all the way down.
 *
 * A row carries `hasLocal`, and that is the whole of what the two gestures are
 * chosen by: a name this repository already has is an ordinary `vcs_checkout`,
 * and one it does not is `vcs_checkout_remote`, which creates the local branch
 * and sets its upstream. The rule answers it rather than the component, for this
 * family's reason. `current` rides beside it for the tick — the repository can
 * only ever be on a branch it has, so `current` implies `hasLocal`.
 */
export function originBranchRows(remote, local, expanded) {
  const open = new Set(expanded ?? [])
  const rows = []
  const walk = (nodes) => {
    for (const node of nodes) {
      if (node.kind === 'folder') {
        const { children, ...folder } = node
        const isOpen = open.has(node.path)
        rows.push({ ...folder, expanded: isOpen })
        if (isOpen) walk(children)
        continue
      }
      rows.push({
        kind: 'branch',
        name: node.name,
        label: node.label,
        depth: node.depth,
        hasLocal: Boolean(node.hasLocal),
        current: Boolean(node.current)
      })
    }
  }
  /* The two facts a row of this tab is drawn by are `originBranches`' and are
     already on the entry `build` spread into the leaf — the filter reads that
     same list, and one answer for both is what keeps a filtered row's glyph and
     its double click agreeing with an unfiltered one's. */
  walk(build(originBranches(remote, local)))
  return rows
}

/**
 * The list a press on one folder of the Origin tab leaves behind.
 *
 * Simpler than `toggleFolder` above and deliberately so: there is no seed to
 * write out whole, because an empty list already means what it says — every
 * folder on this tab is folded. That is the one place this field parts company
 * with `branchFolders`, whose `null` unfolds the current branch's folder; the
 * branch a repository is on is one row among the alphabet here, with nothing
 * about it worth opening a folder for.
 *
 * Named after the settings field it resolves, `remoteBranchFolders`, rather
 * than after the tab that draws it — the field kept its name through the change
 * of shape, and two names for one list is one more thing to hold in agreement.
 *
 * Always a new array: the caller assigns it into `settings.json`, and a list
 * mutated in place gives the store's watcher nothing to notice.
 */
export function toggleRemoteFolder(stored, path) {
  const open = stored ?? []
  return open.includes(path) ? open.filter((folder) => folder !== path) : [...open, path]
}

/**
 * The rows a name filter leaves, flat, in the order the list arrived in.
 *
 * **A case-insensitive substring over the whole name, prefix and all**, so
 * `feat/nxc` and `nxc` both find `feat/nxc-204-…` and `feat/kick` finds
 * nothing at all. Deliberately not fuzzy: the names in a branch list of a
 * naming convention differ by a few characters over a shared prefix, and a
 * fuzzy match over 346 of them answers with most of them.
 *
 * **Nothing is lifted and nothing is re-sorted.** The order is the tab's own —
 * `by_recency`'s on Local, the alphabet on Origin — because a result list
 * ranked some other way would be a second ordering inside a panel that
 * promises one, and the rows a filter is drawn on are already few enough to
 * read. The current branch and the marked ones keep their glyph and their tick
 * and lose their block: three surfaces say where three groups end, and a flat
 * list of hits has one group.
 *
 * A hit is the branch itself with `kind`, `label` (the whole name, since there
 * is no heading above it), `depth: 0` and `pinned` added, plus the three things
 * the drawing needs: `prefix` — everything up to and including the last slash,
 * or `''` — `tail`, and `match`, the half-open range of the match in `name`.
 * Anything else on the branch travels through the spread, which is how a hit of
 * the Origin tab carries `hasLocal`.
 *
 * An empty or blank query answers `[]` rather than the whole list: what asks
 * this question is a field somebody has opened and not yet typed into, and a
 * filter that matched everything would flatten the list for no reason.
 */
export function filterBranches(branches, query, { favorites = [], current = null } = {}) {
  const needle = String(query ?? '').trim().toLowerCase()
  if (!needle) return []
  const marked = new Set(favorites ?? [])
  const hits = []
  for (const branch of branches ?? []) {
    const name = String(branch?.name ?? '')
    const at = name.toLowerCase().indexOf(needle)
    if (at < 0) continue
    const cut = name.lastIndexOf(SEPARATOR)
    hits.push({
      ...branch,
      kind: 'branch',
      name,
      label: name,
      depth: 0,
      pinned: true,
      prefix: cut < 0 ? '' : name.slice(0, cut + 1),
      tail: cut < 0 ? name : name.slice(cut + 1),
      match: { start: at, end: at + needle.length },
      favorite: marked.has(name),
      current: Boolean(branch?.current) || name === current
    })
  }
  return hits
}

/* The word each tab is called when there is no filter on. Here and not in
   `GitPanel.vue` for this file's reason: the labels are half of what the filter
   says about itself, and the half that is prose is the half worth pinning. */
const TAB_WORD = { local: 'Local', origin: 'Origin' }

/**
 * What the two tabs are called, given the filter.
 *
 * With no filter they are the two words and nothing else — the caption above
 * carries the count. With one, the caption is the field and the counts move
 * here: the active tab reads `3 of 346`, and **the other tab reports its own
 * hits** — `Origin 9`. That second number is the whole reason this rule exists.
 * A filter that found nothing on the tab showing, over a list where the other
 * side holds nine matches, would otherwise draw an empty state that is telling
 * the truth about the wrong half of the repository.
 *
 * **Two strings and not one**, `{ id, label, count }`, and the split is where
 * the face changes rather than a convenience for the caller: `SegmentedTabs`
 * sets a label in sans, because it is prose, and a count in mono, because it is
 * a figure — the same face the caption above carries that number in before the
 * filter moves it down here. So the active tab hands over its whole `3 of 346`
 * as the figure with no word beside it, and the other hands over its word and
 * its own hit count separately. Neither field is ever absent: an empty string
 * is a segment drawing nothing there, which is what the unfiltered row is.
 *
 * Both totals and both hit counts are asked for whichever tab is showing, so
 * the caller cannot accidentally label a tab with the other one's numbers.
 */
export function branchTabLabels({
  tab,
  query,
  localTotal = 0,
  originTotal = 0,
  localHits = 0,
  originHits = 0
} = {}) {
  const on = String(query ?? '').trim().length > 0
  return BRANCH_TABS.map((id) => {
    if (!on) return { id, label: TAB_WORD[id], count: '' }
    const total = id === 'local' ? localTotal : originTotal
    const hits = id === 'local' ? localHits : originHits
    return id === tab
      ? { id, label: '', count: `${hits} of ${total}` }
      : { id, label: TAB_WORD[id], count: String(hits) }
  })
}

/**
 * How long the field waits before the rule sees what was typed, in
 * milliseconds, given whatever `--dur-fast` came back as.
 *
 * The delay is the stylesheet's own step and not a number this app invents
 * twice: the same token times every surface change in the system, and
 * `motion.css` zeroes it under `prefers-reduced-motion`, which lands here as a
 * filter that answers on the keystroke — correct, and for free.
 *
 * The parse is here rather than in the component because `getPropertyValue`
 * hands back a **string in whatever unit the stylesheet was written in**, and
 * nothing normalises it: `90ms` today, `.09s` after an edit nobody would think
 * of as behavioural. Surrounding whitespace is tolerated rather than expected —
 * a custom property's value is whatever was written after the colon, and this
 * is not the place to find out which. A unit this does not recognise, an
 * unreadable value and a negative one all fall back to `FILTER_DELAY_MS`, which
 * is `--dur-fast`'s own value written out — a filter that never fires, or one
 * that fires 90 times a second, is worse than one that ignores the token.
 */
export const FILTER_DELAY_MS = 90

export function filterDelay(raw) {
  const text = String(raw ?? '').trim()
  const value = Number.parseFloat(text)
  if (!Number.isFinite(value) || value < 0) return FILTER_DELAY_MS
  if (text.endsWith('ms')) return value
  if (text.endsWith('s')) return value * 1000
  return FILTER_DELAY_MS
}
