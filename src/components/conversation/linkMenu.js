/* The rows of the menu a right-click on a local link in the journal opens, and
   the one path rule they need.

   Pure, with no Vue and no DOM in it, because a `.vue` file is the one thing no
   test in this repository can reach.

   Open and Reveal are *emitted* by `ConversationView.vue` and never performed
   there, and Copy path goes out as `copy-path`: the phone
   draws the same view, and its build aliases `stores/app.js` to a module with no
   `revealInFileManager` and no clipboard helper, so a static import in the view
   would fail that build. `DesktopApp.vue` is the one listener.

   The `kind`s are matched by hand in the view's `pickLink`; a kind renamed on
   one side draws perfectly and does nothing when pressed. */
import { absolutePath } from '../../paths.js'
import { fileManagerName } from '../files/fileMenu.js'
import { isAbsoluteLinkPath } from './localLinkTarget.js'

/* A ceiling for the panel, in pixels, as `PointerMenu`'s `width` prop takes it. */
export const LINK_MENU_W = 240

/* "Reveal", not "Show": the file tree and the Git panel already call this very
   row that, and one action keeps one sentence. */
export function linkMenuItems({ userAgent } = {}) {
  return [
    { kind: 'open', label: 'Open', icon: 'file-code' },
    { kind: 'reveal', label: `Reveal in ${fileManagerName(userAgent)}`, icon: 'folder-open' },
    { type: 'separator' },
    { kind: 'copy-path', label: 'Copy path', icon: 'copy' }
  ]
}

/* The whole path a link names. `absolutePath` joins whatever it is given onto
   the root, so an absolute link — which the journal reads as an ordinary local
   one — is answered as it is: the file manager and the clipboard do not care
   whether it sits inside the project. */
export function linkAbsolutePath(root, path) {
  return isAbsoluteLinkPath(path) ? path : absolutePath(root, path)
}
