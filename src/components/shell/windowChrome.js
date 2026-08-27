/* What the app window's own chrome is, and what the bar at the top of it has to
   draw because of that. Pulled out of `ScopeIndicator.vue` for the reason every
   rule in this repository is outside the component that draws it: a `.vue` file
   is the one thing no test here can reach.

   Three states rather than two, and the third is the important one. `none` is a
   browser — `npm run dev` and `?view=gallery` — where there is no window and no
   title bar, and it is the state every component in this repository is actually
   checked in. A bar that always kept room for traffic lights would show that
   room as a hole on the one screen anybody can inspect it on. */

/** A browser, or a window whose own decorations are still drawn by the system. */
export const CHROME_NONE = 'none'
/** macOS: the system draws the real traffic lights over our bar. */
export const CHROME_TRAFFIC_LIGHTS = 'traffic-lights'
/** Windows and Linux: no decorations at all, so the bar draws the buttons. */
export const CHROME_BUTTONS = 'buttons'

/** The closed list, in the order a reader would meet them. */
export const CHROME_STATES = [CHROME_NONE, CHROME_TRAFFIC_LIGHTS, CHROME_BUTTONS]

/** Where the state is written for the stylesheet: the document root, beside
 *  `data-theme` and `data-density`, and read by `tokens/space.css`. */
export const CHROME_ATTRIBUTE = 'data-window-chrome'

/**
 * What the back end's answer means.
 *
 * Anything unrecognised is `none` rather than an error. The commonest way to
 * get here with nothing is a browser, where the command does not exist at all,
 * and that is an ordinary mode of this app rather than a fault.
 */
export function chromeFromPlatform(platform) {
  return CHROME_STATES.includes(platform) && platform !== CHROME_NONE ? platform : CHROME_NONE
}

/**
 * The same state, corrected for a fullscreen window.
 *
 * macOS moves the traffic lights into an auto-hiding bar when the window goes
 * fullscreen, so an inset kept for them is an empty gap in the middle of the
 * one row that has to stay readable. Our own buttons are not moved by anything
 * and stay exactly where they are — on a window with no decorations they are
 * also the only way back out of fullscreen.
 */
export function chromeInFullscreen(chrome, fullscreen) {
  return fullscreen && chrome === CHROME_TRAFFIC_LIGHTS ? CHROME_NONE : chrome
}

/**
 * The three buttons, left to right, for a window drawing its own.
 *
 * The middle one is two buttons in one seat, which is why it alone carries a
 * second icon and a second label. All four glyph names are already registered
 * in `components/core/icons.js`.
 */
export const WINDOW_CONTROLS = [
  { action: 'minimize', icon: 'minus', label: 'Minimize' },
  {
    action: 'toggle-maximize',
    icon: 'square',
    label: 'Maximize',
    maximizedIcon: 'copy',
    maximizedLabel: 'Restore'
  },
  { action: 'close', icon: 'x', label: 'Close' }
]

/** The glyph a control wears right now. */
export function controlIcon(control, maximized) {
  return (maximized && control.maximizedIcon) || control.icon
}

/** What a control is called right now — its tooltip and its accessible name. */
export function controlLabel(control, maximized) {
  return (maximized && control.maximizedLabel) || control.label
}
