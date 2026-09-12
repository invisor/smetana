/* What a click on the conversation panel's local link is actually allowed to
   touch, pulled out of `views/DesktopApp.vue`'s `onConversationLocalLink` for
   the reason every file in this family is: a `.vue` file is the one thing no
   test in this repository can reach, and this is a rule, not wiring.

   `classifyLink` (`components/markdown/links.js:174`) reads any target that
   merely *looks* like a path as a local link, absolute ones included — its
   own header says as much, and `tests/components/markdown/links.test.js:141`
   pins it. But the channel the two doors on the other side of a click both
   speak is root-relative: `openFile` (`stores/tabs.js`) writes whatever it is
   given straight into `project.openTabs`, and `revealInTree`
   (`views/DesktopApp.vue`) walks `ancestors` of whatever it is given straight
   into `project.expanded` — both lists `sane_list`
   (`src-tauri/src/settings/model.rs`) saves to `settings.json`. Neither door
   checks the path it is handed; both trust the caller, which is why the
   checking belongs here, once, before either is called at all.

   An absolute path inside the project names the same file a relative one
   would, so it is translated rather than refused: `relativeTo`
   (`src/paths.js:133`) is the one conversion from an absolute path to the
   project's own space, and this module reaches for that copy rather than
   writing a second. An absolute path outside the project — or one at all
   with no project open to be inside — has nothing to translate it against,
   and `null` says so: a caller that pushed it into `openTabs` or `expanded`
   anyway would open a tab `files_read` can never fill (`reject_traversal`,
   `src-tauri/src/files/model.rs:215`, refuses anything starting with `/`, `\`
   or a drive letter) and expand folders `files_list` refuses the same way,
   both surviving into `settings.json` with no way back but by hand.

   A relative path is returned unchanged: `classifyLink`'s own channel is
   documented as root-relative already (`links.js:226`), so a target that is
   not absolute is trusted exactly as it always was — this module only ever
   narrows what an *absolute* target does.

   Only a leading `/` is read as absolute. A Windows drive letter never
   reaches this function at all: `OTHER_SCHEME` in `links.js` declines
   `C:\Users\x\a.rs` as an unrecognised scheme before `classifyLink` ever
   builds a `local` result, which is that module's own documented, accepted
   gap rather than one this file has to repeat. */
import { relativeTo } from '../../paths.js'

/** Whether `path` reads as a filesystem root rather than one relative to the
    project — the only shape `classifyLink` can hand this a local link with,
    since a Windows drive letter never reaches it (see this module's own
    header). */
export function isAbsoluteLinkPath(path) {
  return typeof path === 'string' && path.startsWith('/')
}

/* The one conversion this whole module exists for: `path` unchanged when it
   is already relative, the project-relative translation when it is absolute
   and inside the root, and `null` when it is absolute and there is nothing
   for it to resolve against — no root at all, or a root that does not
   contain it. */
export function resolveLocalLinkPath(root, path) {
  if (!isAbsoluteLinkPath(path)) return path
  return relativeTo(root, path)
}
