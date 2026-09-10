/* Which centre tabs are rendered documents rather than files to edit, and which
   of them are documents this project wrote for itself.

   Another of the `branchChoice.js` family — the whole of one rule, pure, with no
   Vue, no DOM and no Tauri in it. A `.vue` file is the one thing no test in this
   repository can reach, so the whole of the path rule lives here and none of it
   in `ReportView.vue`.

   A report is a third kind of centre tab beside the pinned `terminal` and
   `kanban`, and it needs no storage of its own: it rides in `settings.json`'s
   `openTabs` as an ordinary project-relative path and survives a restart the way
   every other tab does. What makes it a report rather than something to edit is
   only where it sits, which is what this file says.

   **There are two writers now, not one.** A run's own account is written by
   `runs::service` into `.smetana/reports/`, and a branch review is written by
   the agent itself into `.smetana/reviews/` — at a path this app composed
   before the session started (`components/git/reviewRows.js`) and named in the
   prompt. The two folders are kept apart because the documents are about
   different things and a person looking for one is not looking for the other;
   what they share is everything this file says about them, so the rule below
   takes a list of folders rather than growing a second copy of itself.

   **There are two rules here now, and they answer two different questions.**
   `isDocumentPath` decides how a tab is *drawn*, and it asks nothing but the
   extension: a `.html` file is a document wherever it sits, because a document
   written to be read is one whether it landed in `.smetana/reviews/` or in the
   `docs/` folder a project's own conventions sent it to. That second case is
   what this rule was widened for — the report of a branch review opened as
   markup in front of somebody, because the agent that wrote it followed its
   project's rules rather than the path in the prompt, and no rule keyed on a
   folder can be right about a folder it has never heard of.

   `isReportPath` keeps the old, narrow question — is this one of *this
   project's own* documents — and it has two callers left, both of which are
   about belonging rather than about drawing: `reportTabPath`, which translates
   a run's absolute path into a tab, and `reviewReportPath`, which composes the
   path a review was told to write to. Neither may open an arbitrary file off
   the disk, so neither may use the wide rule. */

/* The byte no path on any filesystem may contain, and therefore the mark every
   tab id that is *not* a path carries.

   `diffId` and `termTabId` in `stores/tabs.js` are both built on it, and so is
   `STUB_MARK` in `src/paths.js`; the literal is repeated here rather than
   imported from any of the three, because what this file needs is the property
   and not any one of their meanings — a report rule that imported the file
   tree's stub marker would be borrowing a name for a different thing that
   happens to use the same byte. What all four rely on is one sentence: an id in
   `project.activeTab` that is not a path has a zero byte in it. */
const NOT_A_PATH = '\u0000'

/* Whether a tab is drawn as the document it is rather than as its source.

   The extension and nothing else, upper case included — a file named `.HTML`
   by a Windows tool or by a hand is the same document. `.htm` beside `.html`
   for the same reason: the two have meant one thing since the eight-character
   filename stopped being a constraint, and a person who has one of each in a
   folder does not think of them as two kinds of file.

   **The zero byte is refused first, and that clause is load-bearing rather than
   defensive.** `project.activeTab` is not always a path: a diff's id is
   `\u0000diff:<repo>\u0000<path>`, which *ends in the path*, so the extension test on
   its own answers `true` for a diff of any `.html` — `src/index.html` in this
   very repository. What follows from that is not a cosmetic fault: the document
   branch is tried before the diff branch, a synthetic id has no buffer, and the
   person gets an empty sandbox where their diff should be, with no error and
   nothing to press. The old folder rule refused these ids as a side effect of
   demanding a folder prefix; this one has to refuse them on purpose.

   A terminal's id (`\u0000term:<n>`) does not end in a path and would have fallen
   through anyway. It is covered by the same clause regardless, because the rule
   worth writing is "this is not a path", not "this is not one of the two kinds
   of id that exist today".

   Anything that is not a string answers `false` rather than throwing. The value
   is `null` before a project is open and `'terminal'` or `'kanban'` the rest of
   the time.

   No folder is consulted and no separator is refused, which is the whole
   difference from `isReportPath`: this rule does not care where the file sits,
   so there is nothing about its position for it to be wrong about. What draws
   it is a sandboxed frame over the buffer this app already read, so a path it
   answers `true` for is not a path it grants anything. */
export function isDocumentPath(path) {
  if (typeof path !== 'string') return false
  if (path.includes(NOT_A_PATH)) return false
  const name = path.toLowerCase()
  return name.endsWith('.html') || name.endsWith('.htm')
}

/* Where `runs::service` writes a run's document. Project-relative, with the
   trailing slash, because that is the shape `openTabs` carries and comparing
   against it is the whole of the rule. */
export const REPORTS_DIR = '.smetana/reports/'

/* Where a branch review's agent writes its own, the same shape and for the same
   reason. The app names the path and the agent writes `<report>.md` beside
   `<report>.html`; only the second of the two is a document to draw, and the
   `.html` test below is what keeps the Markdown one opening in the editor,
   where a person can read the marks it is made of. */
export const REVIEWS_DIR = '.smetana/reviews/'

/* The folders in the order a path is tried against them, which is no order at
   all: a path is inside one of them or inside neither, and the two names share
   no prefix. */
const FOLDERS = [REPORTS_DIR, REVIEWS_DIR]

/* True only for a document directly inside one of those folders — one of *this
   project's own* reports, which is a narrower question than how to draw it.

   Three things are refused and each for its own reason, and the three hold for
   both folders alike. Anything outside them, however it is named, is somebody's
   own file — `src/index.html` is not a report, though it is still drawn as a
   document, because `isDocumentPath` above answers that and this does not.
   Anything in one of them that is not `.html` is not a document to render — a
   stray note there is text, and drawing it as a page would show a person their
   own words with the markup eaten, which is precisely the `<report>.md` a
   review writes beside its document. And a name carrying a separator at all is
   refused rather than resolved: `files_read` already confines every path to the
   project root, so this is not the boundary, but a path that climbs out of the
   folder is not a report and this rule has no business pretending it can tell
   where it lands.

   Anything that is not a string answers `false` rather than throwing. The value
   comes from `project.activeTab`, which is `null` before a project is open and
   `'terminal'` or `'kanban'` the rest of the time. */
export function isReportPath(path) {
  if (typeof path !== 'string') return false
  const folder = FOLDERS.find((dir) => path.startsWith(dir))
  if (!folder) return false
  const name = path.slice(folder.length)
  return name.endsWith('.html') && !name.includes('/')
}

/* The tab path for the absolute one a run's summary carries, or `null` when
   there is no honest answer.

   Two vocabularies meet here and this is the whole of the translation between
   them. `RunSummary.report` is absolute, because it is written by a worker that
   knows nothing of tabs and has to name a file on disk; `openTabs` is
   project-relative, because the project's own path is already the key that list
   sits under. Neither is going to change, so something has to do this, and it
   lives here rather than in the component for the reason the rule above does.

   Separators are normalised on both sides before they are compared: every path
   inside `files.js` uses `/`, while the string Rust wrote is the platform's, so
   on Windows the two would never match and the button would silently do
   nothing. That is `basename`'s trade in `src/paths.js` taken a second time and
   for the same reason.

   `null` rather than a guess for anything that does not land squarely inside
   one of this project's document folders — a document belonging to a project
   this window has left, or a path in a shape this rule cannot read. Opening the
   wrong file, or opening a report as text, is worse than a button that
   declines. */
export function reportTabPath(report, root) {
  if (typeof report !== 'string' || typeof root !== 'string' || !root) return null
  const slashes = (path) => path.replace(/\\/g, '/')
  const base = slashes(root).replace(/\/+$/, '') + '/'
  const full = slashes(report)
  if (!full.startsWith(base)) return null
  const relative = full.slice(base.length)
  return isReportPath(relative) ? relative : null
}

/* The document a finished branch review has left, or `null` when there is none
   to open.

   The app composed this path itself before the session started — the stamp and
   the branch's slug are `components/git/reviewRows.js`'s — and named it in the
   prompt, extension and all (`agents::prompt::review_branch`). So the file is
   known by arithmetic rather than by looking, and **nothing here watches the
   disk**: `.smetana/reviews/` has no watcher and is not polled, because the one
   thing a watcher could add is the news that a file appeared, and the app
   already knows which file that will be and when the session it belongs to has
   finished.

   `report` is stored without an extension because the agent writes two files at
   it, `<report>.md` and `<report>.html`. Only the second is a document to draw,
   which is why the `.html` is put back here and not stored.

   Three refusals, and the third is the one worth keeping. A session still
   running has written nothing yet. A session doing anything else has no report
   to open, and the `kind` is the wire's own word — `SessionWork::ReviewBranch`
   under `#[serde(tag = "kind", rename_all = "camelCase")]`. And a composed path
   that does not answer `isReportPath` is refused rather than opened: `report` is
   a field off the wire, so the arithmetic that produced it happened in another
   process and could have produced anything, and a tab opened for it would be
   this app reading a file somebody else named. It is `isReportPath` here and
   deliberately not the wider `isDocumentPath`: what is being checked is that the
   path is one this app itself composed for this project, not that it would draw
   nicely. */
export function reviewReportPath(session) {
  if (session?.state !== 'exited') return null
  const work = session?.work
  if (work?.kind !== 'reviewBranch' || typeof work.report !== 'string') return null
  const path = `${work.report}.html`
  return isReportPath(path) ? path : null
}

/* The same over a whole panel of sessions, against the project on screen:
   `{ id, path }` for every finished review among them that belongs there, in
   the order they are listed.

   The id travels with the path because the caller has to remember which endings
   it has already answered for — `terminalState.sessions` is replaced wholesale
   on a project switch and read again on a return, so an ending arrives here
   many times over and the tab must open once. It is the session's own id, which
   the worker issues from one counter across every project, so a single set in
   the caller covers them all.

   **`project` is not a tidiness check, it is what keeps a review's tab from
   being lost for good.** A move between projects sets `settings.activeProject`
   first and only then awaits the new project's layout (`moveTo` in
   `stores/projects.js`), so for that round trip the session list still holds
   the project being left. A review ending inside that window would otherwise be
   answered with an `openFile` into the *new* project's tab list, which
   `applySection` overwrites a moment later — and since the caller marks the
   ending answered whether or not the tab survived, the document would never
   open again on the way back. Refused here, the ending is simply not one of
   this panel's yet, and it is answered when its own project is on screen.

   Nothing at all when no project is named: there is no tab list to open into,
   and this is the one comparison that must not fall back to "close enough".

   A list rather than the first of them: two reviews of two branches can finish
   within a second of each other, and a rule that answered one would silently
   drop the other's document. */
export function reviewReportTabs(sessions, project) {
  if (!Array.isArray(sessions) || typeof project !== 'string' || !project) return []
  const tabs = []
  for (const session of sessions) {
    if (session?.project !== project) continue
    const path = reviewReportPath(session)
    if (path) tabs.push({ id: session.id, path })
  }
  return tabs
}
