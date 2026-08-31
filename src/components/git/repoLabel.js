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
import { basename, relativeTo } from '../../paths.js'

/* `basename` is the one copy of "what is this path called" in the tree
   (`src/paths.js`), and it answers a root path with the path itself rather than
   with an empty string — which is what keeps this row from going blank for a
   project opened at `/`. */
export function repoLabel(repo) {
  return repo.name === '.' ? basename(repo.path) : repo.name
}

/* Where a repository is, for the column that draws it beside the name above.

   The name alone is not enough there and the reason is in the name's own rule:
   exactly one name is replaced, and every other one is whatever somebody wrote
   in `[project].repos`. Two repositories from different places can share their
   last path segment, and an entry like `../shared` says where it is not without
   saying where it is — so the review window's table, which lists every
   repository of a project at once, needs the second half of the answer.

   Five shapes, in the order they are asked for. A repository that reached this
   rule without a path draws nothing at all: a cell of a table is not the place
   to announce that a field was empty, and every other answer here would be a
   sentence built around a value that was never there — `./undefined` says
   something false about where a repository is. The project root itself draws
   `./`, because the row is about the project and a reader already knows which
   one is open. Something inside the project draws the same mark and the path
   under it — `./services/backend` — which is short, and which says at a glance
   that this repository is part of the folder on screen. Outside the project but
   inside the person's home folder draws `~/work/smetana-infra`, the form every
   shell has been writing for decades. Anything else draws its absolute path
   unchanged: there is nothing shorter that is still true.

   `relativeTo` answers the root, the inside and the tilde shapes, and it is
   asked twice rather than reimplemented once — `src/paths.js` exists because
   "is this path inside that one" had been written out three times and the
   copies disagreed. `''` for the folder itself, the path under it for something
   inside, `null` for outside: the same three answers serve the project root and
   the home folder, which is what makes the second question a second call and
   not a second rule.

   The home folder arrives as an argument and is never read here. Nothing in
   `src/` knows one — no command the front end calls today answers with it — and
   a rule that went looking would need Tauri, which is the one import that would
   put this file out of reach of every test in the repository.

   The Rust side has been asking the question for a while, which is the half of
   this worth knowing before writing a fifth reader of the environment:
   `tracker::access::home()` is the one that carries a name, and
   `agents::library`, `sessions::read` and `runs::browser` each read `HOME`
   directly beside it. None of them answers over IPC — `tracker_access_repair`
   calls `access::home()` on every platform, and returns only which of three
   sentences to draw — so what is missing is a command handing that answer to
   the front end, not a way to find it. Without the argument the tilde shape
   simply does not apply and an outside repository draws its absolute path,
   which is true rather than merely shorter.

   The separator is `/` in everything this builds, because that is what
   `relativeTo` normalises to and what a path drawn under `./` or `~/` is read
   as on every system. The absolute fallback keeps the platform's own form: it
   is the path itself less a trailing separator, not a path this rule
   composed. */
export function repoPath(root, path, home = null) {
  const full = typeof path === 'string' ? path : ''
  if (!full) return ''
  /* A trailing separator is the shape a path arrives in often enough to be
     worth dropping here: `/p/a/` must not draw `./a/`. A path that is nothing
     but separators keeps them — a bare `/` is a place, and an empty string is
     not. */
  const trimmed = full.replace(/[/\\]+$/, '') || full

  const inside = relativeTo(root, trimmed)
  if (inside !== null) return inside ? `./${inside}` : './'

  const underHome = relativeTo(home, trimmed)
  if (underHome !== null) return underHome ? `~/${underHome}` : '~'

  return trimmed
}
