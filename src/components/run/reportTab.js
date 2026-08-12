/* Which centre tabs are run reports rather than files to edit.

   Another of the `branchChoice.js` family — the whole of one rule, pure, with no
   Vue, no DOM and no Tauri in it. A `.vue` file is the one thing no test in this
   repository can reach, so the whole of the path rule lives here and none of it
   in `ReportView.vue`.

   A report is a third kind of centre tab beside the pinned `terminal` and
   `kanban`, and it needs no storage of its own: it rides in `settings.json`'s
   `openTabs` as an ordinary project-relative path and survives a restart the way
   every other tab does. What makes it a report rather than something to edit is
   only where it sits, which is what this file says. */

/* Where `runs::service` writes a run's document. Project-relative, with the
   trailing slash, because that is the shape `openTabs` carries and comparing
   against it is the whole of the rule. */
export const REPORTS_DIR = '.smetana/reports/'

/* True only for a document directly inside that folder.

   Three things are refused and each for its own reason. Anything outside the
   folder, however it is named, is somebody's own file — `src/index.html` opens
   in the editor like any other text. Anything in the folder that is not `.html`
   is not a document to render — a stray note there is text, and drawing it as a
   page would show a person their own words with the markup eaten. And a name
   carrying a separator at all is refused rather than resolved: `files_read`
   already confines every path to the project root, so this is not the boundary,
   but a path that climbs out of the folder is not a report and this rule has no
   business pretending it can tell where it lands.

   Anything that is not a string answers `false` rather than throwing. The value
   comes from `project.activeTab`, which is `null` before a project is open and
   `'terminal'` or `'kanban'` the rest of the time. */
export function isReportPath(path) {
  if (typeof path !== 'string' || !path.startsWith(REPORTS_DIR)) return false
  const name = path.slice(REPORTS_DIR.length)
  return name.endsWith('.html') && !name.includes('/')
}
