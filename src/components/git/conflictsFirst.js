/* Where a conflicted file sits in the change list: first, above everything
   else.

   Pure, with no Vue and no DOM in it — the family `changeStatus.js` and
   `changesFold.js` belong to, and for the reason that family exists: a `.vue`
   file is the one thing no test in this repository can reach, so the whole of a
   rule lives outside the component that draws it.

   A merge that stops on a conflict leaves a row nobody may miss, and until this
   file existed the list drew it wherever `git status --porcelain=v2` happened
   to put it — last, on the screenshot this rule was written from, under three
   ordinary `M` rows and carrying one coloured letter to say so. Position is one
   of the three things that make it loud; `ChangeList.vue` supplies the other
   two, the `!` mark and the colour taken by the whole row rather than by the
   letter alone.

   **It is a stable partition and not a sort.** Within each of the two groups
   git's own order is kept, because that order is what the list showed before
   this rule existed and it is worth something — sorting by name would throw it
   away silently while looking like a tidier answer. There is no grouping
   heading either: two conflicts and six ordinary rows do not need a caption to
   be told apart once the first two are coloured throughout.

   The array handed in is left alone. The caller is a computed reading a store's
   `tree.changes`, and a rule that reordered its input would be rewriting what
   the store holds on the way past. */

/**
 * The same changes, conflicted ones first, in a new array.
 *
 * A kind this does not recognise is simply not a conflict — the same way
 * `changeStatus.js` lets an unheard-of kind fall through to a row rather than
 * throwing.
 */
export function conflictsFirst(changes) {
  const rows = changes ?? []
  const conflicted = rows.filter((change) => change?.kind === 'conflicted')
  if (conflicted.length === 0) return [...rows]
  return [...conflicted, ...rows.filter((change) => change?.kind !== 'conflicted')]
}
