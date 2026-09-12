/* What a click on an attachment chip does — `ConversationView.vue`'s own
   `openAttachment`, pulled out because a `.vue` file is the one thing no test
   in this repository can reach, and because this same answer decides two
   things at once: which channel a click opens (smetana-4x3w's own scope) and,
   before that, whether the chip is a control at all — `UserMessage.vue` and
   `Composer.vue` both need the second half to decide what to draw, and
   neither may work it out for itself (see their own headers for why: one is
   a library component with no store behind it in the gallery, the other is
   drawable with no window and no desktop behind it either).

   A picture always opens — in the image window, wherever it sits, because
   that window takes an absolute path and has no notion of a project at all.
   Anything else opens in an editor tab, and an editor tab exists only for a
   path inside the open project: `files_read` refuses a leading slash, and
   `stores/tabs.js`'s `openFile` has nothing to open a path outside the root
   into. `relativeTo` is `null` for exactly that case — `src/paths.js`'s own
   answer for "not inside this folder at all" — so an attachment that is
   neither a picture nor inside the project has no channel this app can open
   it through: not a second copy of "refuse in words", and not the system
   opener either (`openExternal`'s allow-list is `http`/`https` alone, and
   handing it an arbitrary local path would need a Rust command that does not
   exist). `null` is that third outcome, and the two callers draw it as plain
   text rather than a dead button — see their own templates.

   **The `'file'` case carries the project-relative path along with it,
   rather than leaving the caller to ask `relativeTo` a second time.**
   `open-local`'s own `path` is relative to the project root — the same
   shape a prose local link's own `data-path` already carries (`links.js`'s
   header) and what `views/DesktopApp.vue`'s `onConversationLocalLink` hands
   `openFile` unchanged — while an attachment travels everywhere else as an
   absolute one (`session_send`, the journal's own `EventKind::UserMessage`).
   Handing back the bare tag and making `ConversationView.vue` re-derive the
   string would put the one conversion this feature actually depends on —
   get it wrong and every in-project attachment opens a tab named for a path
   `files_read` refuses — in a `.vue` file, where nothing here can check it;
   folding it into this module's own answer keeps it inside what
   `tests/components/conversation/attachmentAction.test.js` can reach. */
import { relativeTo } from '../../paths.js'
import { isImagePath } from '../../stores/attachments.js'

/* `{ kind: 'image' }` | `{ kind: 'file', path }` | `null` — never a bare
   string or a boolean, for the reason above: the `'file'` case has a second
   thing to say, the project-relative path itself, and the two openable
   kinds already need different verbs at the call site (`openImageWindow`
   versus `emit('open-local', …)`). `root` is the active project's absolute
   path, or `''` where there is none — `MarkdownInline.vue`'s own prop, and
   `relativeTo` already answers `null` for either. */
export function attachmentAction(path, root) {
  if (isImagePath(path)) return { kind: 'image' }
  const relative = relativeTo(root, path)
  return relative !== null ? { kind: 'file', path: relative } : null
}
