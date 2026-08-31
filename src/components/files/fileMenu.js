/* What a row in the file tree offers on a secondary click, and the three path
   questions the verbs behind it ask.

   The `projectMenu.js` / `taskMenu.js` / `branchChoice.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — no test in
   this repository can reach a `.vue`, so a rule left inside the component is a
   rule nothing checks. It is also one half of a pair nothing else joins: these
   `kind` strings are matched by hand in `DesktopApp.vue`, the same shape
   `newTabMenu.js` and `onNewTab` make, where a row renamed on one side draws
   perfectly and does nothing at all when pressed. The test pins this side.

   Every item is live. The panel has grown once since, by the five rows of the
   clipboard group, and that is the one shape change this menu has had: the
   three that write to disk — New file, New folder and Delete — were drawn
   greyed by the first half of that work and given their behaviour by the
   second, deliberately without moving anything, because a menu that grows two
   rows in the middle a week after somebody learned it is a menu whose muscle
   memory was worth nothing.

   Delete is the one row in this app that asks a second time in place. The first
   pick redraws it as "Click again to confirm" and leaves the panel open — that
   is the `keepOpen` flag, which `PointerMenu.pick` reads — and the second one
   deletes. Which of the two labels a caller gets is `confirmingDelete`, held by
   `FileTree.vue` beside the path the menu is open on and cleared by the panel's
   `close`, the one event that arrives however the menu leaves. */

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

/* And why Paste is refused, in the same form and for the same reason. Two
   causes, two sentences: nothing has been copied yet, or something has and the
   folder under the pointer is inside it. The second is the check
   `fileClipboard.js` makes and `files/fs.rs` makes again — this row is where a
   person meets it, before the call rather than after it, which is the whole
   point of greying rather than refusing. */
const NOTHING_COPIED = 'nothing copied yet'
const PASTE_INTO_SELF = 'cannot paste a folder into itself'

/* The thirteen rows, in the order and the grouping the design was drawn in:
   two about opening, two about making, five about the clipboard, two about
   copying a path, then the one that reaches out of this window and the one that
   destroys.

   `target` is `'file'`, `'dir'` or `'root'`; the last is the menu the empty
   space below the tree opens, and it is the whole of the difference between the
   lists. Attach to agent and Delete are *absent* there rather than greyed: a
   greyed row says "not now", and neither verb has any meaning about a project's
   own root — there is no file to hand an agent and nothing anybody should be
   offered a way to delete.

   The clipboard group is the same choice made a third time. Cut, Copy,
   Duplicate and Rename say nothing at all about a project's own root, so the
   root menu keeps Paste alone out of the five — a paste into the root is
   ordinary, and it is the one of the group that has somewhere to land. Paste is
   the row that is greyed instead of absent wherever it appears, because "not
   now" is exactly what it means: copy something, or pick a folder that is not
   inside what was copied, and it lights.

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
  /* Whether the clipboard has something the folder this menu is about can take,
     and, when it has not, which of the two reasons that is. Both come from
     `canPasteInto` in `fileClipboard.js` through `FileTree.vue` — the rule is
     one function and this file only puts its answer into words. A caller that
     passes neither gets the greyed row with the commonest of the two reasons,
     which is also the true one before anything has ever been copied. */
  canPaste = false,
  pasteReason = 'empty',
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
    /* The clipboard group, between the making rows and the two that copy a
       path — the making verbs and these five are all about entries on disk,
       while Copy path is about a string, and the separator is where the subject
       changes.

       Four of the five are absent on the root and Paste is not; see the header.
       Duplicate takes no part in the clipboard at all — it is a copy into the
       folder the entry is already in, so it neither reads the record nor
       replaces it, which is why nothing about it is ever greyed. */
    ...(root ? [] : [{ kind: 'cut', label: 'Cut', icon: 'scissors' }]),
    ...(root ? [] : [{ kind: 'copy', label: 'Copy', icon: 'copy' }]),
    {
      kind: 'paste',
      label: canPaste
        ? 'Paste'
        : `Paste — ${pasteReason === 'intoSelf' ? PASTE_INTO_SELF : NOTHING_COPIED}`,
      icon: 'clipboard-paste',
      disabled: !canPaste
    },
    ...(root
      ? []
      : [
          { kind: 'duplicate', label: 'Duplicate', icon: 'copy-plus' },
          { kind: 'rename', label: 'Rename', icon: 'pencil' }
        ]),
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

/* How wide the panel of these rows may get, exported for `taskMenu.js`'s
   reason and living here for the same one: the ceiling is a fact about the
   longest **label**, and the labels are this module's. `FileTree.vue` opens the
   panel with it and `Gallery.vue` draws the same rows at the same width, where
   the number was written out by hand in both.

   It is a ceiling and not a width. `ContextMenu` sizes itself by its widest row
   and clips anything past this with an ellipsis, giving a row no tooltip and no
   `title` to recover the rest from — so what it has to hold is the two rows
   that carry their refusal in the label. "Attach to agent — no agent to type
   into" measured 292px at the default type scale in comfortable density and set
   this number when it was 300. "Paste — cannot paste a folder into itself" is
   two characters longer, so it went to 320 rather than clipping the last word
   off the one sentence that says why a row is off. Room over the longest
   sentence costs nothing: the panel never grows to reach it.

   Prefixed rather than `MENU_W`, the way `sessionMenu.js` prefixes its own:
   `Gallery.vue` imports the board's `MENU_W` already, and two menus sharing one
   number is how a panel gets moved by a rewording somewhere else. */
export const FILE_MENU_W = 320

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

/* The absolute path, for the clipboard and for the file manager — this
   module's own callers, plus `stores/files.js`, which is what moved it up to
   `src/paths.js`. Re-exported under the name it has always had here, the way
   `stores/files.js` re-exports `isStubPath`, so no importer of this module
   moved. */
export { absolutePath } from '../../paths.js'

/* And the relative one, which the tree already holds — except at the root,
   where the tree's own name for it is the empty string. `.` instead: an empty
   clipboard is indistinguishable from a copy that failed, and `.` is what every
   shell and every tool means by "this folder". */
export function relativePath(path = '') {
  return path === '' ? '.' : path
}
