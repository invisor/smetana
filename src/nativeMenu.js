/* The webview's own right-click menu, and why this app has none of it.

   What WKWebView offers on a secondary click is the platform's menu rather than
   this product's: Look Up, Translate, Search with Google, Share, Speech,
   Services, Inspect Element. Every entry there either reaches out of the app
   entirely or exposes the fact that the window is a browser, and it appears over
   any word on the screen — a branch name, a status, a caption — so the one thing
   it reliably says is that nobody designed this. The menus this app does want
   are its own, on the rows that have earned one (a project's tile on the rail
   and a branch row are the two),
   and those are ordinary components drawn from tokens like everything else.

   There is no Tauri setting for this: the webviews have no such switch, so
   refusing the event is the whole of the mechanism.

   **Capture, not bubble.** A component's own menu handler calls `preventDefault`
   itself and the two do not conflict — preventing twice is preventing once — but
   a handler that ever stops propagation would leave the event never reaching a
   listener on the way up, and the native menu would come back on exactly the row
   that has a menu of its own. Running first is what makes the rule hold whatever
   a component does with the event afterwards, and `preventDefault` stops none of
   that: a custom menu still opens.

   It returns its own undo, which is what a test needs and what a caller wanting
   the platform's menu back for a while would need too. Nothing calls it. */
export function suppressNativeMenus(target) {
  const onContextMenu = (event) => event.preventDefault()
  target.addEventListener('contextmenu', onContextMenu, true)
  return () => target.removeEventListener('contextmenu', onContextMenu, true)
}
