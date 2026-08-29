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

   **The hairline is under the last row of the top block**, not under the
   current branch. It says one thing — the real list starts below — and that is
   a fact about the bottom of the block rather than about which branch the
   repository is on. `divider` is the row that carries it.

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
 * The last row of that top block carries `divider`, and the component draws the
 * hairline under it: the fact being stated is that the real list starts below.
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
  const lift = (branch) => {
    rows.push({
      ...branch,
      kind: 'branch',
      label: branch.name,
      depth: 0,
      pinned: true,
      favorite: marked.has(branch.name)
    })
  }
  if (current) lift(current)
  for (const branch of pinnedFavorites) lift(branch)
  if (rows.length > 0) rows[rows.length - 1].divider = true
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
  walk(build(list.filter((branch) => branch !== current && !marked.has(branch?.name))))
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
 * list. What the position does say is which name falls off first when the file
 * is trimmed at its ceiling.
 */
export function toggleFavorite(stored, name) {
  const marked = stored ?? []
  if (!name) return [...marked]
  return marked.includes(name) ? marked.filter((one) => one !== name) : [...marked, name]
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
