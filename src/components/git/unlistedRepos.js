/* What the Git panel says about a repository sitting in the project that
   `.smetana/project.toml` does not name.

   Pure, with no Vue and no DOM in it — the family `sectionHeights.js`,
   `changesFold.js` and `gitActions.js` belong to, and for the reason that
   family exists: a `.vue` file is the one thing no test in this repository can
   reach, so the whole of a rule lives outside the component that draws it. The
   one defect the changes fold shipped with lived in exactly the half that was
   left inside the component.

   **The block is drawn only when there is something to say.** With nothing
   unlisted there is no caption, no row and no change to the panel at all: a
   permanent line saying "this list comes from a file" would be decoration on a
   panel that is quiet by design, where a line naming `newrepo` is the answer to
   the only question anybody asks here. That is what `null` means below, and it
   is the whole of the first of this rule's three cases.

   What it deliberately does not offer is a way to fix anything. The
   configured list is the truth about a project for the runs machinery as much
   as for this panel, and this file writes no verb of its own: the door is the
   setup agent, which is the only thing in this app that writes that file. */

/** The file the caption is about. An identifier, so whatever draws it draws it
 *  in mono — which is why it is its own field rather than a sentence with a
 *  path buried in it. */
export const CONFIG_FILE = '.smetana/project.toml'

/** The prose half of the caption, sentence case like every other caption here. */
export const CAPTION_LEAD = 'Not in'

/** The way out, worded exactly as the project row's own menu words it
 *  (`shell/projectMenu.js`). One act with two doors must not have two names:
 *  somebody who has seen the menu item has to recognise this button as the same
 *  thing, and a second spelling here is the half that drifts. */
export const SETUP_LABEL = 'Set up again'

/**
 * What to draw for these names, or `null` for "say nothing at all".
 *
 * `names` is `vcsState.unlisted` as `vcs_repos` answers it: the folders one
 * level below the project root that git can see and the configuration does not
 * name, in the backend's own order — which is the listing's, sorted, so two
 * machines looking at one folder draw one list.
 *
 * The order is kept rather than re-sorted here, the rule the branch list keeps
 * one section down: an order that arrives meaning something is not this file's
 * to have an opinion about.
 *
 * Blanks and repeats are dropped rather than drawn. Neither can come out of
 * `repos.rs` today; both would draw a row that is not about anything, and a
 * list is cheap to make honest at the one place it is read.
 */
export function unlistedBlock(names) {
  const clean = []
  for (const name of names ?? []) {
    if (typeof name !== 'string') continue
    const trimmed = name.trim()
    if (!trimmed || clean.includes(trimmed)) continue
    clean.push(trimmed)
  }
  if (clean.length === 0) return null
  return { lead: CAPTION_LEAD, file: CONFIG_FILE, names: clean, summary: summary(clean.length) }
}

/**
 * The other two cases — one name, or several — said in a whole sentence.
 *
 * The caption itself is the same two pieces however many names follow it: the
 * names are on the rows underneath, in the panel's smallest voice, and a
 * caption that counted them would be saying twice what a person can see once.
 * So the count lives where it costs no room at all — the accessible name of the
 * block, which is what somebody reading this panel aloud gets instead of a
 * caption and a stack of bare folder names with no relation stated between
 * them.
 *
 * Numerals in both, deliberately: "One repository" beside "3 repositories" is
 * two idioms for one fact, and this sentence is read one at a time.
 */
function summary(count) {
  const noun = count === 1 ? 'repository is' : 'repositories are'
  return `${count} ${noun} in this project but not in ${CONFIG_FILE}`
}
