/* The left column's chrome closes in steps, and this is the whole of the rule.
   Nothing here touches Vue or the DOM — a `.vue` file is the one thing no test
   in this repository can reach, and a cycle of three states is exactly what is
   worth holding a test against.

   Beside `panelWidths.js` rather than inside it: that file is about how wide the
   column may be, this one about how much of it is drawn at all. They meet only
   in `DesktopApp.vue`, which owns both flags.

   The three stops, and the one button that walks them:

     A  rail and panel      header button  ->  B   (`Hide projects`)
     B  panel, no rail      header button  ->  C   (`Collapse sidebar`)
     C  a 32px strip        strip button   ->  A   (`Show sidebar and projects`)

   Two flags rather than a step number, and both are already in `settings.json`
   (`layout.railOpen`, `layout.leftCollapsed`), so the cycle survives a restart
   with no new field in the file and nothing to migrate.

   Two entry points rather than one, because the two buttons are two buttons:
   the header's exists only while the column is open, the strip's only while it
   is folded, and `Panel` emits them as separate events for a reason its own
   comment records. The labels are derived here beside the steps they name, so a
   button cannot end up saying one thing and doing another. */

/** The header button while the project rail is drawn: the next step hides it. */
export const HEADER_HIDE_RAIL = 'Hide projects'
/** The header button once the rail is hidden: the next step folds the column. */
export const HEADER_COLLAPSE = 'Collapse sidebar'
/** The button inside the folded strip. It says both nouns because it returns
 *  both parts — see `nextFromRail`. */
export const RAIL_EXPAND = 'Show sidebar and projects'

/**
 * Where the header button leaves the column: one step further closed.
 *
 * The header is drawn only while the column is open, so a collapsed state has
 * no such button to press and is answered with itself rather than with a step
 * that no gesture can reach.
 */
export function nextFromHeader({ railOpen, leftCollapsed }) {
  if (leftCollapsed) return { railOpen, leftCollapsed }
  if (railOpen) return { railOpen: false, leftCollapsed: false }
  return { railOpen: false, leftCollapsed: true }
}

/**
 * Where the folded strip's button leaves the column: everything back, whatever
 * folded it.
 *
 * It takes no state on purpose. A column folded by dragging the separator while
 * the rail was already hidden opens to the same place as one folded by the
 * second step of the cycle — the alternative, remembering whether the rail was
 * up before the fold, adds stored state and creates a "reopened it and the
 * projects are gone" case the cycle is not supposed to have. The cost is
 * accepted and known: somebody deliberately keeping the rail hidden loses that
 * preference every time the column folds and comes back.
 */
export function nextFromRail() {
  return { railOpen: true, leftCollapsed: false }
}

/**
 * What the header button says right now. The icon does not change with it —
 * both open steps are `panel-left-close`, since the button reads as one
 * direction and the label is what says how far the next press goes.
 */
export function headerLabel({ railOpen }) {
  return railOpen ? HEADER_HIDE_RAIL : HEADER_COLLAPSE
}
