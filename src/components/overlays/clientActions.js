/* The menu rows that act on the machine the person sits at, and the one rule for hiding them.

   Another of the `branchChoice.js` family: the whole of one rule, pure, with no Vue, no DOM and
   no Tauri. A `.vue` file is the one thing no test here can reach, so the list and the filtering
   live in this file and the two menus that draw rows (`PointerMenu`, `MenuButton`) only call it.

   **Why hidden and not drawn disabled.** With a server on another machine a path in this app is a
   path on *that* machine, and the operating system's file manager, the system's "open this file",
   a transcript handed to the OS, all belong to the machine in front of the person. A greyed
   "Reveal in Finder" would be a puzzle with no answer the person could act on
   (docs/superpowers/specs/2026-10-25-headless-server-design.md, "Things that name a path on the
   client"), so the row is not there. Copying a path stays: a path on the server is still a thing a
   person wants to paste into a terminal tab.

   The rows are named by the `kind` the menu builders give them, so a new menu that adds one of
   these kinds is covered without being touched; a new kind of the same sort is added here, and the
   test beside this file holds the list against the builders that produce them. */

export const CLIENT_MACHINE_KINDS = ['reveal', 'reveal-log', 'open-log', 'open-cwd']

/* The items without those rows, and without the separators they leave behind: none at either end
   and never two in a row, so hiding a group does not leave a menu with a bar over nothing. */
export function withoutClientActions(items) {
  const kept = items.filter((item) => !CLIENT_MACHINE_KINDS.includes(item?.kind))
  const clean = []
  for (const item of kept) {
    const bar = item?.type === 'separator'
    if (bar && (clean.length === 0 || clean[clean.length - 1].type === 'separator')) continue
    clean.push(item)
  }
  while (clean.length && clean[clean.length - 1].type === 'separator') clean.pop()
  return clean
}

/* The items a menu draws: all of them, or the ones that remain once this machine's own are gone. */
export function visibleItems(items, hideClientActions) {
  return hideClientActions ? withoutClientActions(items) : items
}
