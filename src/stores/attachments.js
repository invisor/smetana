/* Images attached to a task that has not been filed yet. Seventh of the files
   in this directory that know Tauri exists; components see a reactive object
   and a few functions.

   Three gestures put a picture in this list — the picker, Cmd+V and a drop on
   the window — and they arrive as only two kinds of thing. A file already on
   disk arrives as a path, and Rust copies it (`attachment_import`); the
   clipboard exists inside the page and nowhere this process can reach, so a
   paste arrives as bytes and travels back down (`attachment_write`). Both
   answer with the same record, so nothing above this file has to know which
   gesture produced a thumbnail.

   The list is here rather than inside the dialog because a drop is not the
   dialog's event to hear: Tauri intercepts file drops before the webview sees
   them, and what comes back is a window-level event with absolute paths in it.

   Taking a thumbnail out forgets the path and leaves the file, and so does
   closing the dialog — nothing about handling a picture here ever deletes one.
   The two functions at the bottom are the exception that proves it: they are
   the settings window asking what the store weighs and, when a person presses
   the button, telling Rust to sweep the active project's folder. Which project
   that is and which files are still wanted are both decided in Rust, off the
   tracker worker's own answer; nothing about the deleting is decided here, and
   no path travels from this file into it. */
import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { downloadDir } from '@tauri-apps/api/path'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { dirname } from '../paths.js'

/* What the picker offers. The list is the same four formats `sniff` in
   `attachments.rs` recognises — a filter is a convenience, and Rust is the one
   that decides, by looking at the bytes rather than at the name. */
const EXTENSIONS = ['png', 'jpg', 'jpeg', 'gif', 'webp']

/* A second copy of `MAX_IMAGE_BYTES` from `attachments.rs`, and the same bargain
   `defaults()` in settings.js makes with the Rust schema: Rust holds the
   authority and refuses every oversized payload on arrival, whatever this number
   says. It exists here only so a file that is certainly going to be refused is
   not first read into an ArrayBuffer, turned into a base64 string a third larger
   again, and carried across the boundary to be told no.

   Drift is not symmetrical, and that is the thing to keep in mind if either
   number ever moves. Above Rust's is harmless: the extra files are encoded for
   nothing and refused a moment later by the side that decides. Below Rust's is
   not: every file between the two is refused here, by a message Rust would never
   have sent, and there is no way to attach it at all — the same false refusal
   `decoded_at_least` on the Rust side is deliberately a lower bound to avoid. So
   this number must never be the smaller of the two. */
const MAX_IMAGE_BYTES = 8 * 1024 * 1024

export const attachmentsState = reactive({
  /* { path, name, bytes, url } — `url` is a data URL built from what Rust
     stored, which is how a thumbnail is drawn without an asset protocol. */
  items: [],
  /* What the last attempt was refused with, in one readable line. Shown inside
     the dialog rather than as a toast: the dialog is on top of everything, and
     the refusal is about the thing the person just handed over. */
  lastError: null,
  /* Something is being dragged over the window while the dialog is open. */
  dragging: false
})

/* Rust's errors arrive as { kind, message }; anything else is a broken
   channel rather than a refusal, and says so with whatever it has. */
function messageOf(err) {
  if (err && typeof err === 'object' && typeof err.message === 'string') return err.message
  return String(err)
}

/* Every refusal is logged; the first one of a batch is the one kept on screen.
   The rest are usually the same refusal about the same handful of files, and a
   line that keeps being rewritten reads as the newest thing that happened
   rather than as the thing that did not. */
function fail(err) {
  console.error('[attachments] attaching failed:', err)
  if (attachmentsState.lastError === null) attachmentsState.lastError = messageOf(err)
}

/* A batch begins. The refusal is cleared exactly here — once, in front of the
   loop — and never after a success inside it: clearing on success would let the
   second of two files wipe the message the first one earned, and a person who
   attached [huge.png, small.png] would be looking at one thumbnail, no message,
   and nothing at all to say the other file never arrived. A write that failed
   and looked like it worked is the one thing this app refuses everywhere. */
function begin() {
  attachmentsState.lastError = null
}

function add(attachment) {
  attachmentsState.items.push({
    path: attachment.path,
    name: attachment.name,
    bytes: attachment.bytes,
    url: `data:${attachment.mime};base64,${attachment.data}`
  })
}

/* Files already on disk: the picker's answer and a drop's paths.

   One at a time and in order, not Promise.all: a person who picked four files
   expects them in the order they picked, and the first refusal is the one
   worth showing — four toasts about the same folder of holiday photos say
   nothing the first does not. */
export async function importPaths(paths) {
  begin()
  for (const path of paths) {
    try {
      add(await invoke('attachment_import', { path }))
    } catch (err) {
      fail(err)
    }
  }
}

/* Where the panel opens, and why that is this store's business rather than the
   panel's own. Handed no `defaultPath`, macOS opens where it was left last, and
   with no such memory yet it opens in Recents — every application's files at
   once: what lies inside the Photos library, inside the Music library, inside
   other applications' containers and on network mounts. The panel draws its
   sidebar, asks Spotlight and builds a QuickLook preview for every visible row,
   and since this app is not sandboxed macOS charges each of those touches to the
   responsible process, which is the app. The person gets four consent prompts in
   a row from a development tool that wants none of it — the panel wanted it, in
   the app's name.

   Handing over the same fixed directory on every open would stop that and break
   something better, because `defaultPath` overrides the panel's own memory: a
   person who walked to another folder would be sent back to the start every
   time. So the folder of the last choice is kept here and handed over instead,
   and only the first open after the app starts falls back — to ~/Downloads,
   which is the case this dialog is mostly about by `attachments/mod.rs`'s own
   comment: a screenshot somebody throws away in a week. That folder is protected
   too and the first open there may ask for it; that is accepted knowingly, since
   the question names Downloads and follows the button the person just pressed,
   unlike a question about their music.

   Not in `settings.json`: falling back to a sensible directory when the app
   restarts is an acceptable answer, and a field in the settings file is one more
   thing to carry afterwards. */
let lastPickedDir = null

/* Never a reason for the picker not to open. With nothing to resolve, the
   dialog is opened with no `defaultPath` at all, which is exactly what it did
   before this — and it is the ordinary answer away from the app, where there is
   no path plugin behind the transport: a browser under `npm run dev` and a test
   both land here. */
async function pickerDir() {
  if (lastPickedDir) return lastPickedDir
  try {
    return await downloadDir()
  } catch {
    return undefined
  }
}

/* The picker. Cancelling is not a failure and leaves everything as it was —
   including a refusal already on screen. Opening the picker is not a batch;
   `begin()` therefore belongs to the two outcomes that are, and not to the top
   of this function: clearing there would let a person paste a 12 MB screenshot,
   read why it was refused, click Attach, change their mind, and watch the
   explanation disappear with nothing having happened. */
export async function pickImages() {
  /* Resolved before the try, which is about the picker: a directory that could
     not be worked out is not a picker that failed, and must not be reported as
     one. */
  const defaultPath = await pickerDir()
  let picked = null
  try {
    picked = await open({
      multiple: true,
      title: 'Attach images',
      defaultPath,
      filters: [{ name: 'Images', extensions: EXTENSIONS }]
    })
  } catch (err) {
    /* The picker itself failing is a batch of one that went wrong, and its own
       message has to win over whatever an earlier attempt left behind. */
    begin()
    fail(err)
    return
  }
  if (!picked) return
  const paths = Array.isArray(picked) ? picked : [picked]
  /* Where the next open starts, remembered here and nowhere near the cancelled
     branch above: a person who opened the panel, walked somewhere and thought
     better of it has chosen nothing, and the panel's memory is the one thing
     that should still be deciding then. Of a multiple selection it is the first
     path: they are siblings in practice, and the first is the one that was
     clicked. `?? lastPickedDir` is for the path with no folder above it, which
     keeps the last real answer rather than dropping back to ~/Downloads. */
  lastPickedDir = dirname(paths[0]) ?? lastPickedDir
  /* `importPaths` opens the batch — one `begin()`, in the one place that knows
     something is actually about to be attached. */
  await importPaths(paths)
}

/* base64 in 32 KB slices. `String.fromCharCode(...bytes)` in one call is what
   the fixture in mockBackend.js can afford and a screenshot cannot: spreading
   two million arguments overflows the stack. */
const CHUNK = 0x8000
function toBase64(bytes) {
  let binary = ''
  for (let at = 0; at < bytes.length; at += CHUNK) {
    binary += String.fromCharCode(...bytes.subarray(at, at + CHUNK))
  }
  return btoa(binary)
}

/* Bytes the page is holding, which means a paste and nothing else: a drop
   never reaches the webview at all. `File` and `Blob` both answer
   arrayBuffer(), and a pasted screenshot is one with no name — Rust invents
   one for it. */
export async function attachFiles(files) {
  begin()
  for (const file of files) {
    try {
      /* Measured before it is encoded, which is this route's version of the
         `metadata().len()` read the picker's route gets in Rust. Encoding first
         would build a base64 string a third larger than the file, hand it
         across the boundary and have it refused there — for a video somebody
         dropped by mistake that is hundreds of megabytes of string built to be
         thrown away. Rust refuses it again on arrival; that check is the
         authority and this one only keeps the cost down. */
      if (file.size > MAX_IMAGE_BYTES) {
        throw { kind: 'tooLarge', message: `${file.name || 'the pasted image'} is ${file.size} bytes; the ceiling is ${MAX_IMAGE_BYTES} bytes` }
      }
      const bytes = new Uint8Array(await file.arrayBuffer())
      add(await invoke('attachment_write', { name: file.name || null, data: toBase64(bytes) }))
    } catch (err) {
      fail(err)
    }
  }
}

/* What the store holds, for the settings window's Storage tab: the whole
   directory's size, and how much of the active project's share of it no open
   task refers to any more. A read, so a browser answers it from a fixture and
   the section can be looked at under `npm run dev`.

   The answer travels whole, in Rust's shape, and is handed to the pure module
   that turns it into sentences (`components/settings/storage.js`) — unpacking
   it into flags here would put half the rule in a store and half in a
   component. */
export async function surveyStorage() {
  try {
    return await invoke('attachments_survey')
  } catch (err) {
    console.error('[attachments] the storage could not be read:', err)
    throw new Error(messageOf(err))
  }
}

/* The one call in the app that deletes somebody's pictures, and it exists only
   at the end of a person's press. What goes is Rust's decision, made against
   the active project's board and inside that project's own folder — this
   function names no file and no directory, which is what keeps the button from
   ever reaching another project's images or the store's own root. */
export async function clearStorage() {
  try {
    return await invoke('attachments_clean')
  } catch (err) {
    console.error('[attachments] the storage was not cleared:', err)
    throw new Error(messageOf(err))
  }
}

/* Out of the list, not off the disk — see the note at the top. */
export function removeAttachment(path) {
  attachmentsState.items = attachmentsState.items.filter((item) => item.path !== path)
}

export function clearAttachments() {
  attachmentsState.items = []
  attachmentsState.lastError = null
  attachmentsState.dragging = false
}

/* Drops on the window.

   The webview never sees a file drop: Tauri handles it and reports it here,
   against the window rather than against an element. So while the dialog is
   open the whole window is the drop target, and narrowing that to the
   dialog's rectangle would mean doing its layout arithmetic here in order to
   refuse a gesture nobody makes with a modal on the screen.

   `accepting` is a function rather than a flag this store keeps: whether
   anything is collecting images is the view's business, and asking it is what
   keeps a drop from landing in a list nobody is looking at.

   In a browser there is no webview to ask, and getCurrentWebview throws
   before the subscription — a normal mode, the same one settings.js reads a
   throw from getCurrentWindow as, so it is logged at debug and nothing else
   happens. */
export function watchDrops(accepting) {
  let webview
  try {
    webview = getCurrentWebview()
  } catch {
    console.debug('[attachments] no webview: drops are a Tauri-only gesture')
    return () => {}
  }
  let stop = null
  let stopped = false
  webview
    .onDragDropEvent(({ payload }) => {
      if (!accepting()) return
      if (payload.type === 'enter' || payload.type === 'over') {
        attachmentsState.dragging = true
        return
      }
      attachmentsState.dragging = false
      if (payload.type === 'drop') importPaths(payload.paths)
    })
    .then((unlisten) => {
      stop = unlisten
      /* The view unmounted while the subscription was still on its way. */
      if (stopped) stop()
    })
    .catch((err) => console.error('[attachments] listening for drops failed:', err))

  return () => {
    stopped = true
    if (stop) stop()
  }
}
