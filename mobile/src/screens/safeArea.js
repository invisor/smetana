/* The one module of the phone's layout that names the device's safe area.

   `env(safe-area-inset-*)` is the notch, the rounded corners and the home indicator, and it is
   a CSS function with no token behind it: the design system has spacing and no knowledge of a
   particular phone. It is allowed here and nowhere else in the tree (CLAUDE.md, mobile), and
   always as a floor under a token - `max(<token>, env(...))` - so that on a device without
   insets, and in a desktop browser, the page looks exactly as the tokens say. */

export const inset = (side, floor) => `max(${floor}, env(safe-area-inset-${side}))`

/* `top right bottom left`, each a floor under its inset. */
export const around = (floor) =>
  ['top', 'right', 'bottom', 'left'].map((side) => inset(side, floor)).join(' ')
