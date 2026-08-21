/* What a row in the file tree offers on a secondary click, and the three path
   questions the verbs behind it ask.

   The `projectMenu.js` / `taskMenu.js` / `branchChoice.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — no test in
   this repository can reach a `.vue`, so a rule left inside the component is a
   rule nothing checks. It is also one half of a pair nothing else joins: these
   `kind` strings are matched by hand in `DesktopApp.vue`, the same shape
   `newTabMenu.js` and `onNewTab` make, where a row renamed on one side draws
   perfectly and does nothing at all when pressed. The test pins this side.

   All eight items are live now. The three that write to disk — New file, New
   folder and Delete — were drawn greyed by the first half of this work and are
   given their behaviour by the second, which is why the shape of the panel has
   not moved: a menu that grows two rows in the middle a week after somebody
   learned it is a menu whose muscle memory was worth nothing.

   Delete is the one row in this app that asks a second time in place. The first
   pick redraws it as "Click again to confirm" and leaves the panel open — that
   is the `keepOpen` flag, which `PointerMenu.pick` reads — and the second one
   deletes. Which of the two labels a caller gets is `confirmingDelete`, held by
   `FileTree.vue` beside the path the menu is open on and cleared by the panel's
   `close`, the one event that arrives however the menu leaves. */

/* The path separator to write an absolute path with. Everything relative in
   `stores/files.js` uses `/` whatever the platform, and the project's own root
   arrives from Rust in the platform's form — so a path copied on Windows would
   read `C:\Users\you\dev\app/src/main.rs` if the two were simply joined. The
   root is the only evidence available here of which system this is, which is
   why the question is asked of it rather than of the navigator: a root holding
   a backslash and no forward slash is a Windows path and nothing else is. */
const separatorOf = (root) => (root.includes('\\') && !root.includes('/') ? '\\' : '/')

/* What the platform calls the thing that shows a file in its folder. A pure
   function of the user agent rather than `@tauri-apps/plugin-os`, which would be
   a command, a permission and an await for one noun in one label — and would
   answer nothing at all in `npm run dev`, where this menu is checked.

   The fall-back is a noun and not a guess: WebKitGTK covers every Linux and BSD
   this could run on, and "file manager" is true on all of them where "Nautilus"
   would be true on some. `revealItemInDir` itself works on all three. */
export function fileManagerName(userAgent = '') {
  if (/windows|win32|win64/i.test(userAgent)) return 'Explorer'
  if (/mac/i.test(userAgent)) return 'Finder'
  return 'file manager'
}

/* Why Attach to agent is refused, written into the label itself. `ContextMenu`
   clips a row rather than wrapping it and gives a row no tooltip and no
   `title`, so a reason kept anywhere else is a reason nobody reads — the same
   trade `projectMenu.js` and `taskMenu.js` make, and the reason a caller buys
   the width.

   Two sentences, because there are two reasons and only one of them is "there
   is no agent". The verb types into the agent the centre is *showing*, which is
   the selected one and never whichever happens to be newest — `DesktopApp.vue`
   records why a path delivered into a session nobody is looking at is the one
   failure this gesture cannot afford. So the row can be refused with a live
   agent on screen one column over, and that case is ordinary rather than exotic:
   an agent finishing while another still runs leaves the selection on the one
   that finished, and nothing moves it back.

   Between them the two cover every state the item can be off in — no agent in
   this project at all, one that has exited, one still being spawned (which reads
   as "none yet", true for the second it lasts), and a live agent that simply is
   not the one selected. The second sentence is the house form `projectMenu.js`
   uses for the same shape: say the way out, not just the fact. */
const NOTHING_TO_ATTACH_TO = 'no agent to type into'
const NOTHING_SELECTED = 'select an agent first'

/* The eight rows, in the order and the grouping the design was drawn in:
   two about opening, two about making, two about copying, then the one that
   reaches out of this window and the one that destroys.

   `target` is `'file'`, `'dir'` or `'root'`; the last is the menu the empty
   space below the tree opens, and it is the whole of the difference between the
   lists. Attach to agent and Delete are *absent* there rather than greyed: a
   greyed row says "not now", and neither verb has any meaning about a project's
   own root — there is no file to hand an agent and nothing anybody should be
   offered a way to delete.

   `canAttach` is deliberately not called `hasAgentSession`, which is a different
   question and a live export of `stores/terminals.js`: that one counts a start
   ticket and an exited session, because what hangs off it is whether the centre
   has an Agent tab at all. This one is whether the *selected* agent can be typed
   into right now, which excludes both. Two names, because a prop wired to the
   store's answer by somebody reading the name rather than the comment would
   light this row over a session that would swallow the path.

   `hasLiveAgent` decides nothing about whether the row is off — it only chooses
   which of the two sentences says so. It is the same population `canAttach` is
   narrowed out of, before the selection narrows it. */
export function fileMenuItems({
  target = 'file',
  canAttach = false,
  hasLiveAgent = false,
  confirmingDelete = false,
  userAgent = ''
} = {}) {
  const root = target === 'root'
  const items = [
    { kind: 'open-terminal', label: 'Open in terminal', icon: 'terminal' },
    { kind: 'reveal', label: `Reveal in ${fileManagerName(userAgent)}`, icon: 'folder-open' },
    { type: 'separator' },
    /* The two that make something. Neither opens a dialog: the row for what is
       about to exist appears in the tree where it will be, and the name is
       typed there — so these two verbs put a field on screen and nothing more.
       The glyphs are the plain page and folder with a plus, which is the whole
       of what the pair says. */
    { kind: 'new-file', label: 'New file', icon: 'file-plus' },
    { kind: 'new-folder', label: 'New folder', icon: 'folder-plus' },
    { type: 'separator' },
    { kind: 'copy-path', label: 'Copy path', icon: 'copy' },
    { kind: 'copy-relative-path', label: 'Copy relative path', icon: 'copy' }
  ]
  if (root) return items
  return [
    ...items,
    { type: 'separator' },
    {
      kind: 'attach',
      label: canAttach
        ? 'Attach to agent'
        : `Attach to agent — ${hasLiveAgent ? NOTHING_SELECTED : NOTHING_TO_ATTACH_TO}`,
      icon: 'paperclip',
      disabled: !canAttach
    },
    { type: 'separator' },
    {
      kind: 'delete',
      /* The question and the answer are the same row, which is the whole of the
         design: a modal for this would take the panel away, and with it the
         name of the thing being deleted. `keepOpen` is what makes the first
         pick a question — the panel stays up and redraws — and it is off on the
         armed row, so the second pick closes the way every other row does. */
      label: confirmingDelete ? 'Click again to confirm' : 'Delete',
      icon: 'trash-2',
      tone: 'danger',
      keepOpen: !confirmingDelete
    }
  ]
}

/* Which folder a row's verb acts in, as a path relative to the project root —
   `''` being the root itself, which is what `files_list` already calls it and
   what `resolve_within` reads as "no further".

   A folder answers with itself and a file with the folder holding it, which is
   the only reading that makes a verb mean the same thing on both: "where this
   is". Two verbs ask it and they are one function rather than two copies. Open
   in terminal: a shell started at a file's own path would be asking the PTY to
   `cd` into a file, and the failure would surface a second later as a shell that
   would not start. And the draft row: New file on a folder puts the field
   inside it, and on a file beside it — which is the same sentence.

   It was `shellFolder` while only one verb asked; the name moved when the
   second one did, since a shared rule under one caller's name is how a second
   copy gets written by somebody who read the name and not the body. */
export function folderOf({ path = '', target = 'file' } = {}) {
  if (target === 'root') return ''
  if (target === 'dir') return path
  return parentOf(path)
}

/* The folder a path is *in*, whatever the path is — which is a different
   question from the one above and has one caller: after something is deleted,
   the folder to re-read is the one it was in, and a folder deleted answers
   `folderOf` with itself. A top-level path answers `''`, the root, the way it
   does everywhere else here. */
export function parentOf(path = '') {
  const cut = path.lastIndexOf('/')
  return cut === -1 ? '' : path.slice(0, cut)
}

/* The absolute path, for the clipboard and for the file manager. The tree's
   paths are relative to the project and the two verbs that leave this window
   want the whole thing: a relative path handed to `revealItemInDir` names
   whatever happens to sit under the process's own working directory. */
export function absolutePath(root, path = '') {
  if (!root) return path
  const sep = separatorOf(root)
  const base = root.replace(/[/\\]+$/, '')
  if (!path) return base || root
  return `${base}${sep}${path.split('/').join(sep)}`
}

/* And the relative one, which the tree already holds — except at the root,
   where the tree's own name for it is the empty string. `.` instead: an empty
   clipboard is indistinguishable from a copy that failed, and `.` is what every
   shell and every tool means by "this folder". */
export function relativePath(path = '') {
  return path === '' ? '.' : path
}
