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

export const basenameOf = (path) => path.split('/').filter(Boolean).pop() ?? path

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

/* The marker for the "…N more" stub row in tree paths. A zero byte never
   appears in a file name on any filesystem, so a real path will not collide
   with it. */
const STUB_MARK = '\u0000'

export const isStubPath = (path) => typeof path === 'string' && path.includes(STUB_MARK)

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
