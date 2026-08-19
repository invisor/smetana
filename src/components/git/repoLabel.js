/* What a repository row is called on screen.

   Pure, with no Vue and no DOM in it — the family `changeStatus.js`,
   `branchName.js` and `gitActions.js` belong to, and for the reason that family
   exists: a `.vue` file is the one thing no test in this repository can reach,
   so the rule lives outside the component that draws it.

   The rule is display-only and deliberately does not reach into Rust.
   `vcs::repos::discover` answers with how a repository is *named* — an entry
   from `[project].repos` in `.smetana/project.toml` (`backend`, `../shared`),
   or the result of the one-level walk — and in both arms the project root
   itself is named `"."`. That is the correct answer to "what is it called in
   the config" and a useless one to put in front of a reader: for a project made
   of a single repository the whole row is a dot and a branch, and nothing on it
   says which folder is meant.

   So exactly one name is replaced, and it is the only one that tells a reader
   nothing. Every other name a person wrote themselves, and drawing the path
   instead would be erasing what they called their repository — which is also
   why the path is not drawn for all rows: two repositories from different
   places can share their last path segment. */
import { basename } from '../../paths.js'

/* `basename` is the one copy of "what is this path called" in the tree
   (`src/paths.js`), and it answers a root path with the path itself rather than
   with an empty string — which is what keeps this row from going blank for a
   project opened at `/`. */
export function repoLabel(repo) {
  return repo.name === '.' ? basename(repo.path) : repo.name
}
