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

   Nothing here deletes a file. Taking a thumbnail out forgets the path; the
   bytes stay in the app's data directory, which grows — tidying it is
   deliberately not part of this. */
import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'

/* What the picker offers. The list is the same four formats `sniff` in
   `attachments.rs` recognises — a filter is a convenience, and Rust is the one
   that decides, by looking at the bytes rather than at the name. */
const EXTENSIONS = ['png', 'jpg', 'jpeg', 'gif', 'webp']

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

function fail(err) {
  attachmentsState.lastError = messageOf(err)
  console.error('[attachments] attaching failed:', err)
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
  for (const path of paths) {
    try {
      add(await invoke('attachment_import', { path }))
      attachmentsState.lastError = null
    } catch (err) {
      fail(err)
    }
  }
}

/* The picker. Cancelling is not a failure and leaves everything as it was. */
export async function pickImages() {
  let picked = null
  try {
    picked = await open({
      multiple: true,
      title: 'Attach images',
      filters: [{ name: 'Images', extensions: EXTENSIONS }]
    })
  } catch (err) {
    fail(err)
    return
  }
  if (!picked) return
  await importPaths(Array.isArray(picked) ? picked : [picked])
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
  for (const file of files) {
    try {
      const bytes = new Uint8Array(await file.arrayBuffer())
      add(await invoke('attachment_write', { name: file.name || null, data: toBase64(bytes) }))
      attachmentsState.lastError = null
    } catch (err) {
      fail(err)
    }
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
