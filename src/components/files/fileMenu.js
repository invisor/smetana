/* What a row in the file tree offers on a secondary click, and the three path
   questions the verbs behind it ask.

   The `projectMenu.js` / `taskMenu.js` / `branchChoice.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — no test in
   this repository can reach a `.vue`, so a rule left inside the component is a
   rule nothing checks. It is also one half of a pair nothing else joins: these
   `kind` strings are matched by hand in `DesktopApp.vue`, the same shape
   `newTabMenu.js` and `onNewTab` make, where a row renamed on one side draws
   perfectly and does nothing at all when pressed. The test pins this side.

   Three of the eight items are drawn and greyed. New file, New folder and
   Delete are the second half of this work — everything on this menu that writes
   to disk — and they are here rather than added later because the shape of a
   menu is one of the things a person learns once: a panel that grows two rows
   in the middle a week from now is a panel whose muscle memory was worth
   nothing. They carry no icon either, and that is the same decision as their
   labels: the glyphs are chosen by whoever makes them do something. */

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
  userAgent = ''
} = {}) {
  const root = target === 'root'
  const items = [
    { kind: 'open-terminal', label: 'Open in terminal', icon: 'terminal' },
    { kind: 'reveal', label: `Reveal in ${fileManagerName(userAgent)}`, icon: 'folder-open' },
    { type: 'separator' },
    /* Disabled here, and doing nothing is the whole of what they do — see the
       note at the top of this file. */
    { kind: 'new-file', label: 'New file', disabled: true },
    { kind: 'new-folder', label: 'New folder', disabled: true },
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
    { kind: 'delete', label: 'Delete', icon: 'trash-2', tone: 'danger', disabled: true }
  ]
}

/* Where a shell opened from this row starts, as a path relative to the project
   root — `''` being the root itself, which is what `files_list` already calls
   it and what `resolve_within` reads as "no further".

   A folder opens in itself and a file in the folder holding it, which is the
   only reading that makes the verb mean the same thing on both: "put me where
   this is". Opening a file's own path would be asking the PTY to `cd` into a
   file, and the failure would surface a second later as a shell that would not
   start. */
export function shellFolder({ path = '', target = 'file' } = {}) {
  if (target === 'root') return ''
  if (target === 'dir') return path
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
