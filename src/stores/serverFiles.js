/* Files that cross between this machine and a server that is not on it.

   One of the files in `src/` that know Tauri exists (the list is in CLAUDE.md). It is a store for
   the reason `app.js` is one: nothing here is state, it is what the front end asks of the desktop
   and of the server when a path would otherwise be meant for the wrong machine. Design:
   `docs/superpowers/specs/2026-10-25-headless-server-design.md`, "Things that name a path on the
   client".

   **Local keeps the old rule.** With the server on this machine a path travels and the bytes do
   not (`.claude/rules/attachments.md`), so every function here that could change that answers
   "nothing to do" before it touches anything: `pathsForServer` hands the paths back untouched.

   **Remote moves the bytes.** A file picked or dropped here is read by the shell
   (`client_file_read`, this machine's own command) and written to the server's attachment store
   with `attachment_write`, the command a phone's picture already uses; what comes back, and what is
   stored in a task or sent to an agent, is the server's path. The other direction is
   `server_file_read`, one command that reads one file from the server's attachment store or from a
   project's report folder and refuses everything else (`smetana_core::placement`): a run report is
   written to a temporary file here and opened in the person's own browser.

   The ceiling is Rust's. `client_file_read` refuses a file over `MAX_ATTACHMENT_BYTES` before reading
   it, so this file carries no copy of the number and cannot be more permissive than the side that
   decides. */
import { invoke } from '@tauri-apps/api/core'
import { connection, isRemote, refusalOf } from './connection.js'

/* Whether the server is on another machine, right now. The reactive answer where the connection
   store has been started (the app window); a fresh read of the shell's state where it has not (the
   dialog windows, which never start it) - an invoke to the shell itself, so cheap, and never stale. */
export async function serverIsRemote() {
  if (connection.ready) return isRemote.value
  try {
    const view = await invoke('connection_state')
    return view?.mode === 'split' && view.local === false
  } catch {
    return false
  }
}

/* One file of this machine, stored on the server. Answers the server's attachment record
   (`{ path, name, bytes, mime, data }`), the shape `attachment_write` and `attachment_import` give. */
export async function uploadClientFile(path) {
  const file = await invoke('client_file_read', { path })
  return invoke('attachment_write', { name: file.name, data: file.data })
}

/* The paths an agent on the server can open for these files: the same paths when the server is
   here, the stored copies' paths when it is not. In order, one at a time, and the first failure
   rejects with the file named - nothing half-sent is hidden behind a partial list. */
export async function pathsForServer(paths) {
  if (paths.length === 0 || !(await serverIsRemote())) return paths
  const stored = []
  for (const path of paths) {
    try {
      stored.push((await uploadClientFile(path)).path)
    } catch (err) {
      throw new Error(`${refusalOf(err)}`)
    }
  }
  return stored
}

/* One file from the server's attachment store or a project's report folder: `{ name, bytes, data }`
   with `data` base64. Rejects with the server's refusal, which for a path outside both is "outside
   the folders this server lets a client reach". */
export async function fetchServerFile(path) {
  try {
    return await invoke('server_file_read', { path })
  } catch (err) {
    throw new Error(refusalOf(err))
  }
}

/* A run's report, in the person's own browser. A server on this machine already has the file here;
   a remote one is fetched, written to a temporary file by the shell and opened from there. Rejects
   with words for a toast. */
export async function openReportInBrowser(path) {
  try {
    if (await serverIsRemote()) {
      const file = await fetchServerFile(path)
      await invoke('client_open_fetched', { name: file.name, data: file.data })
    } else {
      await invoke('client_open_document', { path })
    }
  } catch (err) {
    throw new Error(refusalOf(err))
  }
}
