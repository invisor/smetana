/* The centre's tabs: order, which one is temporary, which is active, the
   buffers and their dirtiness. This file knows nothing about Tauri — the disk
   is files.js's job.

   The split is by lifetime: the list of open tabs survives a restart and
   therefore lives in settings, the buffers do not and therefore live here. */
import { computed, reactive } from 'vue'
import { settings } from './settings.js'
import { basenameOf, fileErrorText, filesState, readFile, writeFile } from './files.js'
import { fileAtHead } from './vcs.js'
/* The centre's tab row is partly derived from the worker's sessions — the Agent
   tab and the terminal tabs below. Read only inside computeds and functions,
   never at module scope: `settings.js` imports this file and this file imports
   `settings.js` through `terminals.js`, so which of the two the bundler emits
   first is not this file's to assume (the same cycle `notifications.js` carries
   a note about). */
import { hasAgentSession, removeSession, shellSessions } from './terminals.js'
import { relativeTo } from '../paths.js'
/* The theme is read from the document root rather than from settings.js beside
   this file: a tab's icon is about what is painted, and the gallery paints a
   theme no store holds. */
import { fileIconUrl } from '../catppuccinIcon.js'
import { documentTheme } from '../documentTheme.js'

/* Pinned tabs are not stored in settings: they come first and cannot be closed.

   One of them, now. The board is what a project always has; the Agent tab is
   drawn from the sessions instead (`AGENT_TAB` below), because a project with
   nothing running had a tab whose entire content was an empty terminal. */
export const PINNED = [{ id: 'kanban', kind: 'pinned', label: 'Kanban' }]

/* The Agent tab, present exactly while the project has an agent session — live
   or still starting. Derived and never stored, which is what makes it right on
   its own in the two cases a stored flag would have to be told about: a project
   switch (the session list changes, the tab follows) and a restart (there are
   no sessions, so there is no tab).

   Its `kind` is `pinned` all the same: what that word means to `Tab.vue` is
   "sans-serif label, no close button", and both are still true — the way to
   close this tab is to take the last agent out of the panel, not a cross on the
   tab. The id and the label diverge deliberately: the label says the tab holds
   an agent, while the id 'terminal' sits in settings.json on people's disks and
   in ProjectState::validate's closed list, and renaming it would break both
   sides for the sake of one word. */
const AGENT_TAB = { id: 'terminal', kind: 'pinned', label: 'Agent' }

export const hasAgentTab = computed(() => hasAgentSession.value)

/* Path → { text, original, mtime, error, saveError, stale, loading }.
   text/original differ exactly when the tab is dirty.
   loading — the first read has not come back yet. The buffer is already
   rendered and the field is already on screen, and without this flag a
   character typed before the read returns would look like an edit on top of
   content nobody has seen: the text arriving from disk would go in the bin, its
   mtime would settle into the buffer as the buffer's own, and the very first
   Cmd+S would replace the whole file with that character. While the flag is
   set, the buffer is not edited (setText) and not written (saveTab), and the
   field is read-only — VS Code behaves the same way.
   error — a read refusal ({ kind, message }); there is no edit and text is
   usually empty — except on a tab whose file vanished while already open, where
   whatever was read stays.
   saveError — a write refusal ({ kind, message }). They are separate
   deliberately: error means "the file does not exist as text" and so locks the
   field (readOnly on the tab and in the editor), while a write refusal leaves
   both the text and the right to edit it — otherwise what was typed would be
   locked away at exactly the moment it could not be saved. It clears on the
   first successful write.
   stale — the file moved on disk under a dirty tab; the mtime itself is not
   needed here, only the fact, and keepMine or reloadTab will bring the fresh
   one. */
export const buffers = reactive(new Map())

/* The diff tabs, which are not files and are deliberately not remembered.

   They live here, in module scope beside `buffers`, for exactly the reason
   this file already gives for those: the list of open tabs survives a restart
   and therefore lives in settings, and what has no life beyond the session
   lives here. A diff has no file on disk behind it — `openTabs` is a list of
   paths relative to the project root, re-read on the next launch — so a
   synthetic entry in that list would come back as a tab trying to read a file
   by that name, and the read would refuse in a way nobody could act on.

   Each record is `{ id, repo, path, head, work, missingAtHead, error, loading,
   seq }`: `repo` is the repository's absolute path and `path` is relative to
   it, the pair `vcs_status` reports and `vcs_file_at_head` takes back. */
export const diffTabs = reactive([])

/* A diff tab's id, and it has to be a string no file path can ever be: it sits
   in the same tab row as the paths and lands in `project.activeTab` beside
   them. A zero byte is what makes that true rather than nearly true — no
   filesystem allows one in a name, the property `files.js`'s stub marker
   already leans on.

   Derived from the repository and the path rather than counted out, so
   clicking the same row twice reaches the tab already open instead of laying a
   second one beside it. Nothing ever reads the pieces back out of it: the
   record carries them. */
const DIFF = '\u0000diff:'
const diffId = (repo, path) => `${DIFF}${repo}\u0000${path}`

/* A shape rather than a lookup, deliberately: the one caller that matters asks
   about an `activeTab` restored from settings, at a moment when the list is
   empty by construction. */
export const isDiffTab = (id) => typeof id === 'string' && id.startsWith(DIFF)

export const diffTab = (id) => diffTabs.find((tab) => tab.id === id) ?? null

/* A terminal tab's id, built the same way and for the same reason: it sits in
   the tab row beside the file paths and can land in `project.activeTab` with
   them, so it has to be a string no path can be. The session's own id is what
   it is derived from, so the tab and the shell behind it cannot come apart. */
const TERM = '\u0000term:'
const termTabId = (session) => `${TERM}${session}`

export const isTerminalTab = (id) => typeof id === 'string' && id.startsWith(TERM)

/* The terminal tabs themselves — one per shell session, in the worker's own
   order. Derived and not a list beside `diffTabs`: the shells are already in
   `terminalState.sessions`, and a second copy would be one to hold in agreement
   with the first. What that buys is the whole of a project switch: the session
   list changes, the tabs go with it, and there is nothing to reset.

   The label is numbered by position among the shells rather than by session id,
   which is what a person counts: two open shells are Terminal 1 and Terminal 2
   however many agents have come and gone beside them. Closing the first
   renumbers the second, and that is the honest reading of a position.

   `session` rides on the record and not in the tab row's entry: `tabList`'s
   entries are handed to `Tab.vue` whole, and a field it has no prop for would
   land on the DOM node as an attribute. Same split `diffTabs` keeps. */
const terminalTabs = computed(() =>
  shellSessions.value.map((session, at) => ({
    id: termTabId(session.id),
    session: session.id,
    label: `Terminal ${at + 1}`
  }))
)

export const terminalTab = (id) => terminalTabs.value.find((tab) => tab.id === id) ?? null

/* The same record, found by the session instead of by the tab's own id. Its one
   caller has just been handed a session by the worker and wants to open the tab
   that appeared with it — asking this way rather than building the id is what
   makes "there is no such tab" a possible answer, which is the honest one for a
   shell that was gone again before anybody could look at it. */
export const terminalTabFor = (session) =>
  terminalTabs.value.find((tab) => tab.session === session) ?? null

const project = () => settings.project

export const isDirty = (path) => {
  const buffer = buffers.get(path)
  /* An error does not make a buffer clean. For a file that could not be
     opened, text and original are both empty and the comparison gives false on
     its own; but text a person managed to type before the read refused has to
     count as unsaved — otherwise the tab is closed without asking and it
     disappears silently.
     A loading buffer is clean for the same reason: its text and original are
     both empty, and it cannot be edited before the read returns anyway. */
  return !!buffer && buffer.text !== buffer.original
}

export const dirtyPaths = computed(() => project().openTabs.filter(isDirty))

/* The lock on a tab says "this cannot be edited", and it has to name its own
   reason: the app asks agents nothing yet, and what locks a tab is a read
   refusal — a binary file, not UTF-8, too large, gone. So what travels outwards
   is text, not a bare flag.

   The first read gets no lock, even though the field is not editable for its
   duration either: it lasts milliseconds, and a lock blinking on every file
   open would lie about a permanent property of the tab. */
const readOnlyHint = (buffer) => (buffer?.error ? fileErrorText(buffer.error) : null)

export const tabList = computed(() => [
  /* Before the board, which is where it has always been drawn. */
  ...(hasAgentTab.value ? [AGENT_TAB] : []),
  ...PINNED,
  ...project().openTabs.map((path) => {
    const hint = readOnlyHint(buffers.get(path))
    return {
      id: path,
      kind: path === project().previewTab ? 'preview' : 'file',
      label: basenameOf(path),
      /* The same icon the file tree drew on the row this tab was opened from.
         `iconUrl` and not `icon`: a diff tab below still names a lucide glyph,
         since there the mark says what kind of tab it is rather than what kind
         of file, so `Tab.vue` has to draw both kinds. */
      iconUrl: fileIconUrl(path, documentTheme.value),
      dirty: isDirty(path),
      readOnly: !!hint,
      readOnlyHint: hint ?? undefined
    }
  }),
  /* After the files rather than among them: their order is the person's, kept
     in settings, and a tab nothing remembers has no place inside it. The kind
     is its own — `Tab.vue` draws anything that is not `pinned` the same way, so
     what this buys is the one thing a diff must be told apart for: it has no
     file on disk, and the editor-state sweep in DesktopApp.vue would otherwise
     build a key out of its id. The glyph says what the tab is; there is no lock
     beside it, since two labelled columns and no cursor say "read-only" better
     than a padlock does. */
  ...diffTabs.map((tab) => ({
    id: tab.id,
    kind: 'diff',
    label: basenameOf(tab.path),
    icon: 'git-compare'
  })),
  /* After the diffs, for the reason the diffs are after the files, and it
     applies twice over here: the order of the file tabs is the person's own and
     lives in settings, and a tab nobody remembers has no place inside it. Its
     own kind, so `Tab.vue` can tell it apart from a file — the label is prose
     rather than a path, and the editor-state sweep in DesktopApp.vue must not
     build a key out of an id with no file behind it. */
  ...terminalTabs.value.map((tab) => ({
    id: tab.id,
    kind: 'terminal',
    label: tab.label,
    icon: 'terminal'
  }))
])

export const activeBuffer = computed(() => buffers.get(project().activeTab) ?? null)

async function load(path, { force = false } = {}) {
  buffers.set(path, {
    text: '',
    original: '',
    mtime: 0,
    error: null,
    saveError: null,
    stale: false,
    loading: true
  })
  try {
    const file = await readFile(path)
    /* While the file was being read, the tab may have been closed or the
       project switched. Putting content into a buffer that is already gone
       would resurrect the tab. */
    if (!buffers.has(path)) return
    const current = buffers.get(path)
    /* This branch guards a re-read, not the first load: an edit typed during
       the first read cannot exist — `loading` keeps it out of the buffer. What
       lands here is a buffer that already lived and was dirty when it was told
       to read again. Content from disk must not erase what was typed: we take
       only the timestamp, the text stays with the person and the tab stays
       dirty. force comes from reloadTab — there the overwrite is exactly what
       was asked for. */
    if (!force && current.text !== current.original) {
      buffers.set(path, { ...current, mtime: file.mtime, loading: false })
      return
    }
    buffers.set(path, {
      text: file.text,
      original: file.text,
      mtime: file.mtime,
      error: null,
      saveError: null,
      stale: false,
      loading: false
    })
  } catch (error) {
    if (!buffers.has(path)) return
    const current = buffers.get(path)
    /* The same case as in the success branch, guarding the same thing — a
       re-read. A read refusal is no reason to throw away what was typed by
       hand, so the text stays and the error is simply attached to it. force
       comes from reloadTab: there the person asked to forget their edits
       themselves. */
    if (!force && current.text !== current.original) {
      buffers.set(path, { ...current, error, loading: false })
      return
    }
    buffers.set(path, {
      text: '',
      original: '',
      mtime: 0,
      error,
      saveError: null,
      stale: false,
      loading: false
    })
  }
}

/* Opening a file from the tree. All of VS Code's mechanics live here.

   A single click opens a preview tab; the next single click on another file
   replaces it in place rather than growing the row. A double click (permanent)
   opens a permanent tab straight away — and makes permanent the one already
   open as a preview. */
export function openFile(path, { permanent = false } = {}) {
  const state = project()
  const at = state.openTabs.indexOf(path)

  if (at !== -1) {
    if (permanent && state.previewTab === path) state.previewTab = null
    state.activeTab = path
    return
  }

  /* Only another preview tab takes a preview tab's place. A double click opens
     a permanent tab next to it: the preview slot belongs to the file currently
     being previewed, and a click on a third file must not evict it. */
  const previewAt =
    !permanent && state.previewTab ? state.openTabs.indexOf(state.previewTab) : -1
  if (previewAt !== -1) {
    /* Replacement in place: the row must not rearrange itself just because a
       person is previewing files one after another. A preview tab is never
       dirty, so there is nothing to ask about. */
    buffers.delete(state.openTabs[previewAt])
    state.openTabs.splice(previewAt, 1, path)
  } else {
    state.openTabs.push(path)
  }

  /* A permanent tab does not cancel somebody else's preview: if another
     file's tab was the preview, it stays one. Clearing `previewTab` here would
     take the italics off a tab the person never touched. */
  if (!permanent) state.previewTab = path
  state.activeTab = path
  load(path)
}

/* A double click on a tab. */
export function promote(path) {
  const state = project()
  if (state.previewTab === path) state.previewTab = null
  state.activeTab = path
}

export function closeTab(path) {
  const state = project()
  const at = state.openTabs.indexOf(path)
  if (at === -1) return
  state.openTabs.splice(at, 1)
  buffers.delete(path)
  if (state.previewTab === path) state.previewTab = null
  if (state.activeTab === path) {
    /* The neighbour on the right becomes active, or the one on the left for
       the last tab; with no tabs left, the board. removeProject behaves the
       same way with the project list. */
    state.activeTab = state.openTabs[at] ?? state.openTabs[at - 1] ?? 'kanban'
  }
}

/* A changed file from the Git panel, opened as a diff: HEAD on the left, the
   working tree on the right.

   Always permanent and never a preview: the preview slot belongs to the file
   tree's single click, and a diff evicting the file somebody is reading would
   be two mechanics fighting over one slot. Clicking the same row again lands on
   the tab already open and re-reads it, which is the only freshness this has —
   the panel has no watcher and neither does this. */
export function openDiff(repo, path) {
  const id = diffId(repo, path)
  if (!diffTabs.some((tab) => tab.id === id)) {
    diffTabs.push({
      id,
      repo,
      path,
      head: '',
      work: '',
      missingAtHead: false,
      error: null,
      loading: true,
      seq: 0
    })
  }
  project().activeTab = id
  loadDiff(id)
}

/* Both sides at once, and neither of them is the other's business: HEAD comes
   from git through `vcs.js`, the working tree from the disk through `files.js`
   — the same one `openFile` reads, so a file open in a tab and the right-hand
   side of its diff can never be two different reads of one file.

   The guard is `seq` and it plays `generation`'s part: two reads of one tab can
   be in flight with no ordering guarantee, and without it the last *response*
   would win rather than the last *call* — the same defect `terminals.js` and
   `git.js` guard against, here landing as one repository's text under another
   repository's name. */
async function loadDiff(id) {
  const tab = diffTab(id)
  if (!tab) return
  const seq = tab.seq + 1
  tab.seq = seq
  tab.loading = true

  const { repo, path } = tab
  /* `files_read` takes a path relative to the project root and refuses anything
     outside it, while a repository can sit anywhere `[project].repos` names. So
     "outside the project" is answered here rather than by a refusal from a
     command that would be right to refuse it. */
  const rel = relativeTo(filesState.root, `${repo}/${path}`)
  const [head, work] = await Promise.all([
    fileAtHead(repo, path).then(
      (text) => ({ text, error: null }),
      (error) => ({ text: null, error: asError(error) })
    ),
    rel === null
      ? Promise.resolve({ text: '', error: { kind: 'outside' } })
      : readFile(rel).then(
          (file) => ({ text: file.text, error: null }),
          /* A deleted file has nothing in the working tree, and that is the
             whole content of the right-hand side rather than a failure — the
             diff of a deletion is exactly an empty side. Every other refusal is
             one. */
          (error) =>
            error?.kind === 'notFound'
              ? { text: '', error: null }
              : { text: '', error: asError(error) }
        )
  ])

  const current = diffTab(id)
  if (!current || current.seq !== seq) return
  current.head = head.text ?? ''
  current.work = work.text ?? ''
  /* `null` from `vcs_file_at_head` is a file HEAD does not have — added,
     untracked, or every file of a repository with no commit yet. The empty left
     side is the truth, and the flag is what lets the view say which of the two
     empties it is looking at. */
  current.missingAtHead = head.error === null && head.text === null
  current.error = head.error ?? work.error
  current.loading = false
}

/* Both stores answer a refusal in Rust's own `{ kind, message }` shape and a
   transport failure as anything at all; one shape here means the view has one
   thing to draw. The kinds are `FilesError`'s on both sides, so `fileErrorText`
   already has the words for them. */
function asError(error) {
  if (error && typeof error === 'object' && typeof error.kind === 'string') return error
  return { kind: 'io', message: String(error?.message ?? error) }
}

export function closeDiff(id) {
  const at = diffTabs.findIndex((tab) => tab.id === id)
  if (at === -1) return
  diffTabs.splice(at, 1)
  const state = project()
  if (state.activeTab !== id) return
  /* The neighbour on the right, then the one on the left, then back to the
     files, then the board — `closeTab`'s rule, extended over the seam between
     the two lists rather than restarted at it. */
  state.activeTab =
    diffTabs[at]?.id ?? diffTabs[at - 1]?.id ?? state.openTabs[state.openTabs.length - 1] ?? 'kanban'
}

/* Closing a terminal tab, which is the same act as killing the shell in it —
   one gesture, exactly as closing a terminal window is. A tab that only hid a
   live shell would leave a process nobody can see and nobody will remember to
   stop, and the tab is derived from the session anyway: there is nothing to
   close but the session.

   The kill is `removeSession`'s → `terminal_remove`, which is the one path there
   is, and what that path does is `Pty::kill`: the process is killed outright and
   the session leaves the worker's map. It is the same end the remove button in
   the agents panel gives an agent, and deliberately not the app's own exit path
   — `kill_all` in `service.rs` hangs up the process group and waits, and that
   one runs when the window closes. Nothing here signals anything itself, and
   nothing here is a second way to end a session.

   The neighbour is worked out before the await, against the list as it stands,
   and the selection is only moved if the removal actually took: a refusal leaves
   the shell running and its tab in the row, and moving away from a tab that is
   still there would be a second failure on top of the first. */
export async function closeTerminalTab(id) {
  const tab = terminalTab(id)
  if (!tab) return
  const before = terminalTabs.value.map((entry) => entry.id)
  const at = before.indexOf(id)
  await removeSession(tab.session)
  const state = project()
  if (state.activeTab !== id || terminalTab(id)) return
  /* The neighbour on the right, then the one on the left, then back to the
     files, then the board — `closeDiff`'s rule, which is `closeTab`'s extended
     over the seams between the lists rather than restarted at each. */
  state.activeTab =
    before[at + 1] ?? before[at - 1] ?? state.openTabs[state.openTabs.length - 1] ?? 'kanban'
}

/* The last agent has gone, so the Agent tab has gone with it. Nothing else in
   the centre changes: the tab a person is on is left alone unless it is the one
   that just disappeared, in which case the board is where they land — the same
   fallback `closeTab` and `closeDiff` already use.

   A function rather than a watcher in here, and the caller is `DesktopApp.vue`:
   a module-scope `watch` would read `terminals.js` at evaluation time, and this
   module is one half of an import cycle where that is not safe (see the note on
   the import). */
export function dropAgentTab() {
  const state = project()
  if (state.activeTab === 'terminal') state.activeTab = 'kanban'
}

/* An edit. It also drops the temporary flag — the second of the two ways to
   make a tab permanent, and the one that makes the "a preview tab is never
   dirty" invariant true. */
export function setText(path, text) {
  const buffer = buffers.get(path)
  /* Until the first read comes back there is nothing to edit: the buffer is
     empty not because the file is empty. An edit into it would be an edit on
     top of the invisible. */
  if (!buffer || buffer.error || buffer.loading) return
  buffers.set(path, { ...buffer, text })
  const state = project()
  if (state.previewTab === path) state.previewTab = null
}

/* Writes are chained: there is never more than one in flight at a time.
   Otherwise two Cmd+S in a row would race for order, and the second write could
   land on disk before the first. The same trick as in settings.js. */
let chain = Promise.resolve()

export function saveTab(path) {
  const buffer = buffers.get(path)
  /* loading: there is nothing to write and no reason to write content nobody
     has read. */
  if (!buffer || buffer.error || buffer.loading || !isDirty(path)) return chain
  const text = buffer.text
  chain = chain
    .then(async () => {
      /* The mtime is taken at execution time, not when the write is queued:
         the first of two consecutive writes has already moved it on disk, and
         an mtime captured at queue time would produce a `stale` on our own
         save. The text, by contrast, is captured at queue time — we save what
         the person asked for. */
      const before = buffers.get(path)
      if (!before || before.error || before.loading) return
      try {
        const mtime = await writeFile(path, text, before.mtime)
        const current = buffers.get(path)
        if (!current) return
        /* original is set to what was written, not to the current text: the
           person may have kept typing while the write was in flight, and their
           new edits have to stay dirty. */
        buffers.set(path, { ...current, original: text, mtime, saveError: null, stale: false })
      } catch (error) {
        const current = buffers.get(path)
        if (!current) return
        if (error.kind === 'stale') {
          /* Nothing was written and nothing was lost. We show the strip and
             wait for a decision: reload, or keep mine. */
          buffers.set(path, { ...current, stale: true })
        } else {
          /* A write refusal is not a read refusal. `error` holds "the file
             could not be opened" and makes the field read-only; for a failed
             save that would lock away the typed text: neither editable nor
             saveable again. */
          buffers.set(path, { ...current, saveError: error })
        }
      }
    })
    /* A rejected promise left in the chain would poison every write that
       follows: their `.then` would silently skip its body, and Cmd+S would stop
       doing anything until a restart. The write's own refusal is handled above;
       only the unexpected lands here — and even that has no right to stop the
       queue. */
    .catch((err) => {
      console.error('[tabs] write failed:', err)
    })
  return chain
}

/* The file moved under a dirty tab — the window-focus sweep noticed it. The
   decision is the person's, so the tab gets a strip with buttons.

   The read refusal is cleared rather than kept: buffers that had one arrive
   here too — the file was deleted and put straight back, the permissions were
   fixed. The file is there, the typed text is there, and leaving the field
   locked would freeze it forever: nothing else leads out of `error`. */
export function markStale(path) {
  const buffer = buffers.get(path)
  if (!buffer || buffer.loading) return
  buffers.set(path, { ...buffer, error: null, stale: true })
}

/* The file vanished under a clean tab. There is nothing to lose and nothing to
   re-read, but showing the contents of something that does not exist for hours
   is not on either. The read refusal here is not an invention but the truth:
   the next read would return exactly it — and with it come the explanation
   under the strip, the lock on the tab and the locked field. A file that comes
   back lifts the tab back up — that is the focus sweep's job. */
export function markGone(path) {
  const buffer = buffers.get(path)
  if (!buffer || buffer.loading || buffer.error) return
  buffers.set(path, { ...buffer, error: { kind: 'notFound' }, stale: false })
}

/* "Reload": the content from disk wins, the edits go. */
export async function reloadTab(path) {
  if (!buffers.has(path)) return
  await load(path, { force: true })
}

/* "Keep mine": the strip goes away, but the buffer's mtime catches up with the
   disk — otherwise the next Cmd+S would refuse with `stale` again, and there
   would be no way out of it. */
export async function keepMine(path) {
  const buffer = buffers.get(path)
  if (!buffer) return
  try {
    const file = await readFile(path)
    buffers.set(path, { ...buffer, mtime: file.mtime, stale: false })
  } catch (error) {
    buffers.set(path, { ...buffer, error })
  }
}

export function saveTabs(paths) {
  return Promise.all(paths.filter(isDirty).map(saveTab))
}

/* Discarding the unsaved content of the listed tabs — the "Don't save" answer.
   The list is mandatory: if one tab was asked about, only that one is touched. */
export function discardTabs(paths) {
  for (const path of paths) {
    const buffer = buffers.get(path)
    if (buffer) buffers.set(path, { ...buffer, original: buffer.text })
  }
}

/* Moving to another project: the old project's buffers must not outlive it by
   a single frame. The tab list is left alone — it will come from the new
   project's settings. The diffs go with the buffers rather than with the list:
   they name a repository of the project being left, and nothing brings them
   back.

   The Agent tab and the terminal tabs are not here and need nothing: they are
   derived from the session list, which `loadSessions` replaces with the new
   project's on the same move. Deriving them is what makes a project switch free
   — there is no third list to remember to clear. */
export function resetTabs() {
  buffers.clear()
  diffTabs.length = 0
}

/* Restoring after a restart or a project switch. We read everything that was
   open; a path whose file does not read falls out of the list silently —
   exactly as an issue that is no longer in the tracker falls out. */
export async function restoreTabs() {
  const state = project()
  /* `activeTab` is remembered and a diff tab is not, so a restart can arrive
     with the one naming the other: the list is empty here by construction and
     the tab it points at will never appear. Rust already refuses an `activeTab`
     that is neither pinned nor one of the open files, so on the app's own path
     this has been dealt with a moment earlier — it is repeated here because
     `settings.json` is not the only way this state is reached (the browser mock
     answers `settings_load` itself), and the cost of the two disagreeing is a
     centre column drawing nothing at all. */
  if (isDiffTab(state.activeTab) || isTerminalTab(state.activeTab)) {
    state.activeTab = state.openTabs[0] ?? 'kanban'
  }
  /* The same repair, for the tab that is derived rather than remembered.
     `activeTab: "terminal"` is written whenever somebody was last watching an
     agent, and sessions deliberately do not survive a restart — so on the next
     launch that value names a tab which cannot exist yet. Rust leaves it alone
     on purpose (`ProjectState::validate` does not know how many sessions there
     are, and it is the only place the `chat` migration can land), so the repair
     is here, and it is guarded rather than unconditional: on a project *switch*
     the new project may well have agents running, and taking a person to the
     board then would be a repair of something that was not broken. */
  if (state.activeTab === 'terminal' && !hasAgentTab.value) state.activeTab = 'kanban'
  const paths = [...state.openTabs]
  await Promise.all(paths.map(load))

  const gone = paths.filter((path) => buffers.get(path)?.error?.kind === 'notFound')
  if (!gone.length) return
  for (const path of gone) {
    const at = state.openTabs.indexOf(path)
    if (at !== -1) state.openTabs.splice(at, 1)
    buffers.delete(path)
    if (state.previewTab === path) state.previewTab = null
    if (state.activeTab === path) state.activeTab = 'kanban'
  }
}

/* Who asks about unsaved work.
 *
 * The view asks the question (the modal lives in DesktopApp.vue), and there are
 * three occasions for it: closing a tab, switching project and closing the
 * window. The last two come from stores that know nothing about the interface —
 * so the view leaves its function here and the stores call it.
 *
 * The contract is this: returning true means "carry on", false means "the
 * person changed their mind". If nobody registered a handler, we carry on:
 * losing edits silently is bad, but locking up the app because of an
 * unregistered view is worse.
 */
let ask = null

export function onUnsaved(handler) {
  ask = handler
}

export async function confirmUnsaved(paths = dirtyPaths.value) {
  if (!paths.length) return true
  if (!ask) return true
  return ask(paths)
}
