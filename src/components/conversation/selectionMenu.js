/* The rows of the menu a right-click on a selection in the journal opens, and
   nothing else.

   Pure, with no Vue and no DOM in it, because a `.vue` file is the one thing no
   test in this repository can reach.

   Two of the three verbs are *emitted* by `ConversationView.vue` and never
   performed there: the phone draws the same view, and its build aliases
   `stores/app.js` to a module exporting `openExternal`, `openImageWindow` and
   `pickFiles` only, so a static `import { copyText }` in the view would fail
   that build. `DesktopApp.vue` is the one listener.

   The `kind`s are matched by hand in the view's `pickSelection`; a kind renamed
   on one side draws perfectly and does nothing when pressed. */

/* A ceiling for the panel, in pixels, as `PointerMenu`'s `width` prop takes it. */
export const SELECTION_MENU_W = 240

export function selectionMenuItems({ canQuote, canFile }) {
  return [
    { kind: 'quote', label: 'Quote', icon: 'quote', disabled: !canQuote },
    { kind: 'copy', label: 'Copy', icon: 'copy' },
    { type: 'separator' },
    { kind: 'new-task', label: 'New task from selection', icon: 'plus', disabled: !canFile }
  ]
}
