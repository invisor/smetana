/* The project's files in the front end. One of the files in src/ that know
   about Tauri — along with tracker.js, settings.js and projects.js. The tabs
   store (tabs.js) goes to the disk through this one and knows nothing about
   Tauri itself.

   The truth here is outside, on disk, as it is for the tracker — but there is
   nothing to catch up with it: the tree deliberately has no watcher. Freshness
   comes from the window-focus sweep (see DesktopApp.vue) and the refresh
   button. */
import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { STUB_MARK } from '../paths.js'

export const filesState = reactive({
  /* The active project's absolute path. Every other path in this store is
     relative, and their separator is always "/". */
  root: null,
  /* A directory's path → { entries, truncated }. The empty string is the root.
     Filled lazily: a directory appears here only after it has been expanded. */
  dirs: new Map(),
  /* The directories currently being read. A second read of the same directory
     does not start: expand-collapse-expand must not produce three requests. */
  loading: new Set(),
  lastError: null
})

/* The same function `projects.js` exports as `basename`, kept under this store's
   own name so its importers do not move. It used to be a second implementation
   splitting on `/` alone, which meant `setupFor` — a project path, passed
   through here — lost nothing on macOS and everything on Windows. One module
   now: src/paths.js, where the trade it makes is written down. */
export { basename as basenameOf } from '../paths.js'

/* The two sentences the making verbs are refused with, written once because
   they are read from two tables. A name a person typed is the subject of both,
   which is what makes them the same words wherever the refusal surfaces. */
const NAME_TAKEN = 'Something with that name is already there.'
const NAME_REFUSED = 'That name cannot be used.'

/* Back-end errors are diagnostics: their text speaks the filesystem's language
   and is addressed to whoever fixes things. The person is shown a short phrase
   chosen by the error's machine-readable kind, and the full text stays in the
   console. The same trick as in tracker.js. */
const ERRORS = {
  notFound: 'This file is gone from disk.',
  denied: 'No permission to read this file.',
  notAFile: 'This is not a file.',
  binary: 'Binary file — not shown.',
  tooLarge: 'File is too large to open here.',
  notUtf8: 'Not UTF-8 text — not shown.',
  outside: 'That path is outside the project.',
  stale: 'This file changed on disk since it was opened.',
  /* The one kind here that no file read can produce. It arrives from the other
     side of this table: a diff's left-hand side is `vcs_file_at_head`, which is
     three git calls, and every git call has a ceiling now (`run.rs`). Without
     this line a git stopped on its ceiling would draw "Could not read this
     file." — silently, since the fallback is what an unknown kind gets. */
  timeout: 'Git took too long and was stopped.',
  /* The other two kinds no read produces, and they are here for the reason
     `timeout` is: this table is the fallback every kind that crosses the IPC
     lands in, and one that is missing from it falls back silently to a sentence
     about reading a file. They are made by `files_create` and `files_mkdir` —
     see MAKE_ERRORS, which is where a person actually meets them. */
  alreadyExists: NAME_TAKEN,
  badName: NAME_REFUSED,
  io: 'Could not read this file.'
}

export function fileErrorText(error) {
  return ERRORS[error?.kind] ?? ERRORS.io
}

/* The same error kinds, but for writes. A separate map rather than a shared
   one: "No permission to read this file." after a refused Cmd+S describes
   something other than what happened, and the person looks for the cause in the
   wrong place. There is deliberately no `stale` key here — a stale mtime is
   handled by its own branch with buttons. */
const SAVE_ERRORS = {
  notFound: 'This file is gone from disk — nothing was written.',
  denied: 'No permission to write this file.',
  notAFile: 'This is not a file — nothing was written.',
  outside: 'That path is outside the project — nothing was written.',
  io: 'Could not save this file.'
}

export function saveErrorText(error) {
  return SAVE_ERRORS[error?.kind] ?? SAVE_ERRORS.io
}

/* And a third map, for directories. A person sees a directory read refusal as
   a toast, and "This file is gone from disk." under a folder's name describes
   something other than what happened. */
const DIR_ERRORS = {
  notFound: 'This folder is gone from disk.',
  denied: 'No permission to read this folder.',
  notAFile: 'This is not a folder.',
  outside: 'That path is outside the project.',
  io: 'Could not read this folder.'
}

export function dirErrorText(error) {
  return DIR_ERRORS[error?.kind] ?? DIR_ERRORS.io
}

/* A fourth, for making a file or a folder. The two kinds above it are the whole
   reason it exists — nothing else in this store can be refused because a name
   is taken — and the rest of it says "nothing was created", which is true of
   every refusal here: `resolve_new_within` decides before anything is opened,
   and `create_new` is the one call that could have raced, so there is no
   half-made state to describe. */
const MAKE_ERRORS = {
  alreadyExists: NAME_TAKEN,
  badName: NAME_REFUSED,
  notFound: 'That folder is gone from disk — nothing was created.',
  denied: 'No permission to write into that folder.',
  notAFile: 'That is not a folder — nothing was created.',
  outside: 'That path is outside the project — nothing was created.',
  io: 'Could not create it.'
}

export function makeErrorText(error) {
  return MAKE_ERRORS[error?.kind] ?? MAKE_ERRORS.io
}

/* And a fifth, for the one verb that destroys. `badName` covers two things here
   and neither is a name somebody typed, which is why it does not borrow the
   making table's sentence: the project's own root, by whichever spelling
   reached the command, and a last segment Rust will not take as a name. The
   second is rarer than it sounds and reachable all the same — a file whose own
   name holds a backslash, or one spelled like a drive (`C:notes.txt`), both
   perfectly legal on macOS and Linux and both listed by `files_list`. Neither
   is split or repaired: splitting the first would delete a different file, so
   the row is refused instead, and the toast over an ordinary-looking row is
   this. */
const TRASH_ERRORS = {
  badName: 'That name cannot be deleted.',
  notFound: 'It is already gone from disk.',
  denied: 'No permission to delete this.',
  outside: 'That path is outside the project — nothing was deleted.',
  io: 'Could not move it to the trash.'
}

export function trashErrorText(error) {
  return TRASH_ERRORS[error?.kind] ?? TRASH_ERRORS.io
}

/* The marker for the "…N more" stub row in tree paths, and the test for one.
   Both live in src/paths.js — the file tree's context menu has to recognise a
   stub as well, and a component cannot import a store. Re-exported under the
   name it has always had here, so no importer of this store moved. */
export { isStubPath } from '../paths.js'

/* An error from Tauri arrives as a { kind, message } object; a delivery error
   (the mock threw an Error, the IPC did not come up) arrives as anything at
   all. We reduce both to one shape so callers do not have to handle two
   cases. */
function normalize(error) {
  if (error && typeof error === 'object' && typeof error.kind === 'string') return error
  return { kind: 'io', message: String(error?.message ?? error) }
}

/* A directory read refusal is visible to the person: at that moment the tree
   shows whatever it managed to read, and with no words it simply looks like an
   empty folder. The full error text stays in the console; a short phrase
   travels outwards — the toast in DesktopApp.vue shows it. */
function report(where, error) {
  console.error(`[files] ${where}:`, error)
  filesState.lastError = dirErrorText(error)
}

/* Moving to another project. The tree is reset entirely: showing the old
   project's directories under the new one's name is not on for a second. */
export function setRoot(path) {
  filesState.root = path
  filesState.dirs = new Map()
  /* The instance is deliberately not replaced: otherwise the finally of a read
     already in flight would clear the mark of somebody else's just-started
     request on the new Set. */
  filesState.loading.clear()
  filesState.lastError = null
}

export async function listDir(dir = '') {
  if (!filesState.root || filesState.loading.has(dir)) return
  const root = filesState.root
  filesState.loading.add(dir)
  try {
    const listing = await invoke('files_list', { root, dir })
    /* While the directory was being read, the project may have been switched:
       the answer belongs to the previous root and must not go into the new
       tree. The last move wins, not the last answer. */
    if (filesState.root !== root) return
    filesState.dirs.set(listing.dir, {
      entries: listing.entries,
      truncated: listing.truncated
    })
  } catch (err) {
    report(`could not read the directory ${dir || '(root)'}`, normalize(err))
  } finally {
    filesState.loading.delete(dir)
  }
}

/* Re-reading directories that are already known — the window-focus sweep and
   the refresh button. Directories absent from the map are not read: nobody
   asked to expand them. */
export async function refreshDirs(dirs) {
  const known = dirs.filter((dir) => filesState.dirs.has(dir))
  await Promise.all(known.map((dir) => listDir(dir)))
}

export async function readFile(path) {
  try {
    return await invoke('files_read', { root: filesState.root, path })
  } catch (err) {
    const error = normalize(err)
    console.error(`[files] could not read ${path}:`, error)
    throw error
  }
}

export async function writeFile(path, text, expectedMtime) {
  try {
    return await invoke('files_write', {
      root: filesState.root,
      path,
      text,
      expectedMtime
    })
  } catch (err) {
    const error = normalize(err)
    console.error(`[files] could not write ${path}:`, error)
    throw error
  }
}

/* The three verbs that change what is on disk outside a file's own text. Each
   throws its normalized error rather than reporting it into `lastError`: a
   refused write is not the tree's own state the way a refused directory read
   is — somebody asked for it just now and is owed a toast, which is
   `DesktopApp.vue`'s to raise. None of them touches `filesState`; re-reading
   the parent directory afterwards is the caller's, because only the caller
   knows what else has to happen in the same breath (a tab to open, a folder to
   expand, tabs to close).

   `dir` and `name` rather than a path, in both makers, because that split is
   what the check in `resolve_new_within` is made of — see `files/fs.rs`. */
export async function createFile(dir, name) {
  try {
    return await invoke('files_create', { root: filesState.root, dir, name })
  } catch (err) {
    const error = normalize(err)
    console.error(`[files] could not create ${dir || '(root)'}/${name}:`, error)
    throw error
  }
}

export async function createDir(dir, name) {
  try {
    return await invoke('files_mkdir', { root: filesState.root, dir, name })
  } catch (err) {
    const error = normalize(err)
    console.error(`[files] could not create the folder ${dir || '(root)'}/${name}:`, error)
    throw error
  }
}

/* Into the system trash, where it can be got back from. The name says so: a
   `deleteFile` here would read as gone for good at every call site. */
export async function trashPath(path) {
  try {
    await invoke('files_trash', { root: filesState.root, path })
  } catch (err) {
    const error = normalize(err)
    console.error(`[files] could not move ${path} to the trash:`, error)
    throw error
  }
}

/* Timestamps in a batch. A refusal here means nothing: the focus sweep is a
   convenience, and there is no reason to drop the interface over it. */
export async function statFiles(paths) {
  if (!filesState.root || !paths.length) return []
  try {
    return await invoke('files_stat', { root: filesState.root, paths })
  } catch (err) {
    console.error('[files] could not check the timestamps:', normalize(err))
    return []
  }
}

/* FileTree expects nested nodes with children, while the store holds a flat
   map of directories. We build the tree on the fly and descend only into the
   expanded ones: a node whose children have not been read yet returns
   children: undefined, and FileTree simply does not go deeper.

   A truncated directory gets one extra stub entry: silent truncation would read
   as "there are no more files here". Its kind is "file", because a stub row has
   no kind of its own in the tree, and its path is marked with a zero byte: no
   filesystem lets that character into a file name, and `isStubPath` recognises
   a stub by it. The click handlers in DesktopApp.vue filter it out — the row
   itself knows nothing about being one. */
export function treeNodes(expandedSet) {
  const build = (dir) => {
    const listing = filesState.dirs.get(dir)
    if (!listing) return undefined
    const nodes = listing.entries.map((entry) => ({
      path: entry.path,
      name: entry.name,
      kind: entry.kind,
      children: entry.kind === 'dir' && expandedSet.has(entry.path) ? build(entry.path) : undefined
    }))
    if (listing.truncated > 0) {
      nodes.push({
        path: `${dir}${STUB_MARK}more`,
        name: `…${listing.truncated} more`,
        kind: 'file'
      })
    }
    return nodes
  }
  return build('') ?? []
}
