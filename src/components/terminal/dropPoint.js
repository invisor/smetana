/* Where a file was let go, in the one coordinate space the hit test can read.

   Tauri reports a drag against the window rather than against an element, and
   it types the point `PhysicalPosition` on every platform. On two of the three
   that is not what is in it. wry reads the point out of the toolkit and
   `tauri-runtime-wry` hands it on unscaled, so the units are the toolkit's:
   AppKit's `draggingLocation()` and `NSView.frame`, which are points — wry
   reads that same `frame` back as a `LogicalSize` one file over — and GTK's
   `drag-motion` widget coordinates, which are logical as well. Windows alone
   reports device pixels, from `ScreenToClient` on the client area.

   Points and CSS pixels are the same unit in a webview at zoom 1, so on macOS
   and Linux the position has already arrived in the space
   `document.elementFromPoint` reads, and dividing it by the device pixel ratio
   moves it. On a Retina Mac that division halved every point: a drag over the
   agent panel was hit-tested at half its distance from the top left corner, so
   the panel answered only where the halved point happened to land back inside
   it, and the person had to hunt for the working part of a panel that looks
   uniform. Nothing threw — `elementFromPoint` answers for any point on screen.

   Which of the two arrives is a fact about the build, so it comes from the back
   end (`drag_drop_space` in `src-tauri/src/window.rs`) rather than from a
   user-agent string, the same way `shell/windowChrome.js` learns which chrome
   the window has.

   Outside `TerminalView.vue` because a `.vue` file is the one thing no test in
   this repository can reach, and beside `dropPaths.js` for the same reason it
   is: that file decides the characters, this one decides the point. */

/** Points already in CSS pixels from the top left of the webview: macOS, Linux. */
export const DROP_SPACE_LOGICAL = 'logical'
/** Device pixels from the top left of the client area: Windows. */
export const DROP_SPACE_PHYSICAL = 'physical'

/** The closed list, and the whole of the contract with `drag_drop_space`. */
export const DROP_SPACES = [DROP_SPACE_LOGICAL, DROP_SPACE_PHYSICAL]

/**
 * What the back end's answer means.
 *
 * Anything unrecognised is `physical`, which is the reading every platform got
 * before this was measured: a name nobody knows costs the fix rather than the
 * gesture, and the commonest way to arrive with nothing is a browser, where
 * the command does not exist and no drop is ever reported anyway.
 */
export function dropSpaceFromPlatform(space) {
  return DROP_SPACES.includes(space) ? space : DROP_SPACE_PHYSICAL
}

/**
 * The reported point, in CSS pixels from the top left of the viewport.
 *
 * A ratio that is not a positive number is treated as 1 rather than divided by:
 * `devicePixelRatio` is 0 in no browser this app runs in, but the result of
 * getting it wrong here is a point at infinity, and a hit test cannot say that
 * something has gone wrong — it just refuses every drop.
 */
export function viewportPoint({ x, y }, space, ratio) {
  const divisor = space === DROP_SPACE_PHYSICAL && ratio > 0 ? ratio : 1
  return { x: x / divisor, y: y / divisor }
}
