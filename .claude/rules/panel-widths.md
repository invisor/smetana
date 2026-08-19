---
paths:
  - "src/views/panelWidths.js"
  - "src/components/shell/Panel.vue"
  - "src/components/shell/Resizer.vue"
---

# Panel widths

Either side column is dragged by the `Resizer` between it and the board, and the rules for how wide
it may get live in `src/views/panelWidths.js` — pure, no Vue and no DOM, which is what makes them the
one part of this that a test can reach at all. A panel takes at most a third of the window and never
so much that the board drops below `CENTER_MIN`; the neighbour is part of that sum, costing its own
width open and a rail collapsed.

The stored width and the drawn width are different numbers, and conflating them would be the defect
here. What `settings.json` keeps is what a person dragged to; what `leftStyle` draws is that number
clamped against the window it is in now. Only a drag writes back — narrowing the window squeezes the
panel and widening it restores what was asked for, because a resized window must not silently rewrite
a preference. Every delta a `Resizer` emits is likewise measured from a width snapshotted at
`dragstart`, not from the previous frame: clamping against the last frame would make each clamped
move the new origin and the panel would drift away from the pointer.

Dragging a panel past `COLLAPSE_SLACK` below its minimum folds it into a 32px rail, keeping the
stored width so it comes back where it left; pulling out of the rail past `EXPAND_PULL` reopens it,
and so does the button inside the folded rail. The left panel's header button is no longer one of the
ways: it hides the project rail beside the panel and leaves the panel alone. Double click resets to the shipped 236/340.

`RAIL` is the one width in the app that does **not** grow with the app-wide font size, and it cannot:
these pure functions do arithmetic with it — a collapsed neighbour's cost, both drag thresholds, the
clamp against the stored width — so a scale-dependent rail would have to be threaded through every
one of them. What sits in it does grow, though, and that was a real defect: the expand button is an
`IconButton size="sm"` at `--control-h-sm`, which reaches 44px at the top of the range and hung over
the board beside it. So the button is capped rather than scaled — `min(var(--control-h-sm),
RAIL_CONTROL_MAX)`, which leaves both densities exactly as they are at the shipped size and stops the
growth at the rail's edge. `Panel.vue` takes both numbers from this file now; it used to write the 32
out a second time. When the window is too narrow to honour both a panel's minimum and the board's
floor, the panel keeps its minimum and the board takes the squeeze — the board's content scrolls, a
file tree at 90px does not.

`PROJECT_RAIL` is the second fixed width and the reasoning is `RAIL`'s exactly: the project rail is
44px, it does arithmetic here, and it does not grow with `--ui-scale`. It is *outside* the left
panel's own width — the column is the rail plus the panel — so it is subtracted in `maxWidth`, and
every caller that builds a geometry passes `railOpen` through. It joins the sum that protects the
board's floor and not the third-of-the-window cap, which is deliberate: the floor is a guarantee
about how much room the board actually has, which 44px genuinely takes away, while the fraction is a
rule about how much of a window one panel may dominate and a strip of chrome does not change the
answer to that. When the column is folded into its own 32px rail the project rail is not drawn at
all — two rails side by side would say nothing about which button belongs to which.

`Panel` emits **two** events where it used to emit one, and the split is load-bearing rather than
tidy. `toggle` is the header button, which in the left column is about the project rail beside the
panel and not about the panel; `expand` is the button inside the folded rail, which can only ever
mean this panel coming back. One event for both had the folded column's button hiding the projects
instead of unfolding. `subtitle` is the other addition: its presence switches the title from a
section header's uppercase micro-caps to a name in ordinary mono, and lets the header grow past
`--tab-h`, which two lines of type do not fit inside.

`Resizer` diverges from the design system in behaviour, not in styling — pointer capture, so a
release outside the window still ends the drag; `user-select: none` on the body for the duration; and
arrow keys, which its `role="separator"` had been promising with nothing behind them. Those belong
back upstream.
