/* The phone's `stores/app.js`: the three names the conversation panel imports from it, and
   nothing more. See `phoneStores` in vite.config.js for why the desktop's file is not used.

   The desktop's one reaches for windows, the clipboard, the build's version and a mock
   backend, none of which the phone has or wants. */
import { openUrl } from '@tauri-apps/plugin-opener'

/* A link in a message opens in the person's own browser. A link that cannot be opened is
   said in the console and nowhere else: there is no toast corner on this screen. */
export async function openExternal(url) {
  try {
    await openUrl(url)
  } catch (err) {
    console.warn('[companion] could not open the link:', err)
  }
}

/* A picture named in a message is a path on the desktop, and the bridge carries no file
   reads. Nothing opens. */
export function openImageWindow() {}

/* Attachments are paths on the desktop and `session_send` refuses a non-empty list, so the
   panel draws no paperclip here and this is never pressed. An empty answer is what a
   cancelled dialog gives, which the panel already treats as "nothing to attach". */
export async function pickFiles() {
  return []
}

/* Files named by the panel are on the machine the panel runs on; the phone attaches none (see
   `pickFiles`), so there is nothing to send anywhere and the list comes back as it was. The desktop's
   version uploads them when its server is on another machine. */
export async function pathsForServer(paths) {
  return { paths, failures: [] }
}

export function failureSentence() {
  return ''
}
