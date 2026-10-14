/* What a tab in the centre row offers on a secondary click, and the four rules
   that say which tabs a batch verb reaches.

   The `fileMenu.js` / `newTabMenu.js` family: pure, no Vue and no DOM, which is
   the whole reason it is a file of its own — no test in this repository can
   reach a `.vue`, so a rule left inside the component is a rule nothing checks.
   It is also one half of a pair nothing else joins: these `kind` strings are
   matched by hand in `DesktopApp.vue`'s `onTabAction`, where a row renamed on
   one side draws perfectly and does nothing at all when pressed. The test pins
   this side.

   The rows for a path — Copy path, Copy relative path, Reveal, Attach to agent —
   are taken from `fileMenu.js` rather than written out again: they are the same
   four verbs with the same labels and the same two refusal reasons, and a second
   copy of those words is how the tree and the tab come to disagree about why a
   row is off.

   Present and greyed are different statements, and the rule is `fileMenu.js`'s:
   greyed is "not now", absent is "never". A diff, a shell and the Reports page
   have no path on disk, so the path rows are absent from their menus; the two
   pinned tabs cannot be closed by a cross, so Close and Close to the right are
   absent from theirs. Close all but terminals is greyed when there is no
   terminal, because that changes the moment one is opened.

   Every set below is read off the **drawn** row (`tabList`) and never off
   `openTabs`: the row is dragged, its order is stored apart from the list of
   files, and "to the right" means to the right on the screen. And every set
   excludes the pinned tabs, which are not closed by anything here. */
import { fileMenuItems, FILE_MENU_W } from '../files/fileMenu.js'

/* A ceiling, and the same one the tree's panel has: the longest label on this
   menu is the reason-carrying Attach to agent row, which is the tree's own
   string. Re-exported under this module's name so a caller does not import a
   file's constant for a different menu, and so a test can pin it. */
export const TAB_MENU_W = FILE_MENU_W

/* The kinds whose tab is a file on disk. */
const isFileKind = (kind) => kind === 'file' || kind === 'preview'

const closable = (tab) => tab.kind !== 'pinned'

/* Every tab but this one, among those that can close. */
export function othersOf(tabs = [], id) {
  return tabs.filter((tab) => closable(tab) && tab.id !== id)
}

/* Every tab drawn after this one. An id that is not in the row answers nothing
   rather than everything: a menu opened on a tab that has since closed must not
   turn into "close the lot". */
export function rightOf(tabs = [], id) {
  const at = tabs.findIndex((tab) => tab.id === id)
  if (at === -1) return []
  return tabs.slice(at + 1).filter(closable)
}

/* What "saved" means for each kind: a file with no unsaved edits, and a diff or
   the Reports page, which hold nothing of anybody's. A shell is not in it, and
   neither is a file somebody is still editing. `isDirty` takes a tab's id, the
   way the store's own does. */
export function savedOf(tabs = [], isDirty = () => false) {
  return tabs.filter((tab) => {
    if (isFileKind(tab.kind)) return !isDirty(tab.id)
    return tab.kind === 'diff' || tab.kind === 'reports'
  })
}

/* Everything that can close except the shells. */
export function butTerminals(tabs = []) {
  return tabs.filter((tab) => closable(tab) && tab.kind !== 'terminal')
}

/* Every tab that can close at all — the set Close all reaches. */
export function allOf(tabs = []) {
  return tabs.filter(closable)
}

/* The shells in a set. A batch that holds one asks before it closes, because
   closing a terminal's tab ends its shell. */
export function terminalsIn(set = []) {
  return set.filter((tab) => tab.kind === 'terminal')
}

/* The words of the question a batch holding shells asks first. One sentence per
   count, since "1 terminals" and "its shells" are the two ways a plural goes
   wrong; owned here rather than in the template so the app and the gallery draw
   the same sentence and a test can read it. */
export const CLOSE_TERMINALS_TITLE = 'Close terminals too?'
export function closeTerminalsText(count) {
  return count === 1
    ? '1 terminal will be closed and its shell ended.'
    : `${count} terminals will be closed and their shells ended.`
}

/* The rows, for the tab of this `kind`: `file`, `preview`, `diff`, `terminal`,
   `reports` or `pinned`. `hasTerminals` is whether any shell is open — it is
   the one thing the menu needs to know about the rest of the row. `canAttach`,
   `hasLiveAgent` and `userAgent` are the tree's three, for the same reasons. */
export function tabMenuItems({
  kind = 'file',
  hasTerminals = false,
  canAttach = false,
  hasLiveAgent = false,
  userAgent = ''
} = {}) {
  const pinned = kind === 'pinned'
  const closeGroup = [
    ...(pinned
      ? []
      : [{ kind: 'close', label: 'Close', icon: 'x' }]),
    { kind: 'close-others', label: 'Close others', icon: 'x' },
    ...(pinned ? [] : [{ kind: 'close-right', label: 'Close to the right', icon: 'x' }]),
    ...(pinned ? [] : [{ kind: 'close-saved', label: 'Close saved', icon: 'x' }]),
    {
      kind: 'close-but-terminals',
      label: 'Close all but terminals',
      icon: 'x',
      disabled: !hasTerminals
    },
    { kind: 'close-all', label: 'Close all', icon: 'x' }
  ]
  if (!isFileKind(kind)) return closeGroup

  const path = fileMenuItems({ target: 'file', canAttach, hasLiveAgent, userAgent }).filter((item) =>
    ['copy-path', 'copy-relative-path', 'reveal', 'attach'].includes(item.kind)
  )
  const pick = (...kinds) => kinds.map((k) => path.find((item) => item.kind === k))
  return [
    ...closeGroup,
    { type: 'separator' },
    ...pick('copy-path', 'copy-relative-path'),
    { type: 'separator' },
    ...pick('reveal', 'attach'),
    { type: 'separator' },
    /* Greyed on a permanent tab, which has nothing to keep open. The label
       needs no reason: a tab that is not italic is the reason. */
    { kind: 'keep-open', label: 'Keep open', icon: 'pin', disabled: kind !== 'preview' }
  ]
}
