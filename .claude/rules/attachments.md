---
paths:
  - "src-tauri/src/attachments/**"
  - "src/stores/attachments.js"
  # The store's only consumer in `src/` since the dialog became a window: the
  # host loads it, subscribes it to that window's drops and answers the three
  # image emits itself. Somebody editing the host has to see this rule.
  - "src/views/DialogWindow.vue"
  # The store's second consumer, and the one that holds nothing: one picture by
  # path, no list and no drop. Somebody editing it has to see why that is not a
  # second owner of the list.
  - "src/views/ImageWindow.vue"
  - "src/components/kanban/AttachmentStrip.vue"
  - "src/components/settings/StorageSettings.vue"
  - "src/components/settings/storage.js"
---

# Attachments: pictures on a task nobody has filed yet

A screenshot is the fastest way to say what is wrong, so the new-task dialog takes images:
`src-tauri/src/attachments/`, `src/stores/attachments.js` and
`components/kanban/AttachmentStrip.vue`, plus the Storage tab of the settings window
(`components/settings/StorageSettings.vue` over the pure `settings/storage.js`). The Rust side is the
same no-worker shape as `files/` and `git.rs`, for the same reason — writing a couple of megabytes
guards no state — and it is five commands over pure functions that carry the tests: `mod.rs` is the
disk and the vocabulary, `cleanup.rs` is the whole of the deleting rule with no filesystem in it,
plus the one rule about where a path this store reads may point.

Three gestures put a picture in the list and they arrive as only two kinds of thing. A file already
on disk arrives as a path and Rust copies it (`attachment_import`); the clipboard exists inside the
page and nowhere this process can reach, so a paste arrives as bytes (`attachment_write`). Both
answer with the same record, which leaves the strip one shape to draw.

**The list belongs to the New task window, and the app window does not hold it.** Tauri intercepts a
file drop before any webview sees it and reports it against the *window* it landed on, so where the
list lives follows entirely from what the dialog is. While it was a modal over the board the drop was
the app window's event and never the dialog's, and the store therefore sat outside the dialog, in the
window that could hear one. The dialog is an OS window of its own now (`smetana-at3`), so the drop
**is** its own event — and the only process that can hear it is the one somebody dropped the file on.
That is why the store moved: `DialogWindow.vue` imports it, lazily and only for the `new-task` kind,
subscribes `watchDrops` to that window with an `accepting` that is simply `true`, and answers the
`attach`, `files` and `remove` emits on the spot instead of forwarding them. `DesktopApp.vue` does
not import this store at all; what still reaches it from the app window is `surveyStorage`, through
`notifications.js`, which is a question about a folder and not about the list. Only the paths travel
back, in `submit`.

There is a second way a path comes back into that list, and it exists because the window can be
destroyed with a draft still in it. A project switch closes the New task window
(`views/dialogRegistry.js`), and what the app window keeps behind is the draft's *paths* — never its
bytes: `restorePaths` in this store walks them when the window is rebuilt, and `attachment_reopen`
reads one of those files back out of the store and answers with the record an import answers with, so
the strip draws the same thumbnail and no second copy is written. That matters more here than
anywhere: nothing in this app deletes an attachment except the Storage tab's button, so a re-import
would leave one more file on disk per switch, for good. Its argument is confined to `store_root()` by
`cleanup::in_store`, pure and tested with no filesystem under it — everything that command may
legitimately be handed came out of this store to begin with. `taskDraft.js` beside `NewTaskModal.vue`
is what says when a draft may come back at all.

**The second reader of this store holds nothing at all, and that is what keeps the rule above
true.** Clicking a thumbnail shows the picture whole in an OS window of its own —
`src/views/ImageWindow.vue`, `?view=image`, opened by `window::image_window_open` and labelled
`image`, one per app and re-aimed by `image:show` rather than rebuilt. That window calls exactly one
thing here, `readAttachment(path)`: one `attachment_reopen`, the record handed straight back, and
**nothing written to `attachmentsState`** — no list, no `lastError`, and above all no `watchDrops`.
The invariant is untouched because a command is not a subscription: the list still belongs to the New
task window alone, and nothing in the image window's webview can hear a drop or keep one.

**What travels to it is the path, and never the bytes.** A record's `url` is a `data:` URL of up to
`MAX_IMAGE_BYTES` of base64: it fits in no URL, and putting it on the event channel would be eleven
megabytes over IPC per click. So Rust percent-encodes the path and the name into the window's URL
(`image_query`), the window reads the file itself, and `cleanup::in_store` confines that read exactly
as it already confined the restore path — nothing new is allowed and no second check on a path was
written. A file the Storage tab swept while the draft still names it is an ordinary outcome and the
window draws an empty state carrying the name, which is why `readAttachment` rejects rather than
swallowing the refusal the way `restorePaths` does: there is no list here for the rest of to arrive
into. The strip itself opens nothing — it emits `view` with `{ path, name }` and `DialogWindow.vue`
answers it beside `attach`, `files` and `remove`, so `AttachmentStrip.vue` stays drawable in
`?view=gallery` with no store behind it. Before smetana-msxp the picture was an overlay pinned to
`inset: 0`; once every dialog became a window of its own that meant the viewport of a 440-point
window nobody can resize, so "the picture, larger" came out the size of the dialog it was opened
from.

**No drop is heard twice, and the reason is the webviews rather than anything in this file.** The only
other subscriber to a window's drag-drop event in the whole tree is `watchSessionDrops` in
`terminals.js`, which types a dropped path into a live agent; it is subscribed from `TerminalView.vue`
and therefore lives in the app window's webview, while this store now lives in the dialog's. Tauri
delivers a drop only to the window it landed on, so the two never see the same event and need no
arbiter — there is deliberately none. That is what makes `() => true` safe here, and it is safe **only
for as long as this store stays out of the app window**: importing it into `DesktopApp.vue` again, or
widening either side's acceptance, puts both subscribers back on one webview, where a single file is
copied into a draft task *and* typed into somebody's running agent, with nothing on either side to
catch it. `.claude/rules/terminal.md` carries the full argument, beside the hit test that settles
which pane of the app window a drop belongs to.

Nothing is *collected* twice either, which is the smaller half: `attachment_import` and
`attachment_write` are commands rather than subscriptions, so the move added no second observer and
no second copy of the list.

**The bytes are copied, never pointed at.** They go into `app_data_dir()` and the path that reaches
the agent is absolute, because the case this exists for is a screenshot in `~/Downloads` that a
person throws away in a week and the link in the issue has to outlive that. Writing into the
repository instead would work in every clone and worktree, but only for files somebody committed, and
committing binaries into another person's tree is not this app's decision. The price is plain: in
somebody else's clone, and in CI, the pictures are not there.

There is no `resolve_within` here, and its absence is the design rather than an oversight.
`files/fs.rs` confines every path to the project root because everything it touches belongs to the
project; nothing here does — the *source* is whatever a person picked in the OS's own dialog or
dragged off their desktop. What is confined is the *destination*: always `app_data_dir()/attachments`,
under a name that is not the one that arrived. `stored_name` builds it from a timestamp and a `slug`
keeping ASCII letters and digits and nothing else, so no incoming name can climb a directory, hide
behind a dot or need quoting — that string ends up in a prompt, in a shell argument and in an issue
description. The extension comes from `sniff`ing the bytes, not from the name, so a JPEG somebody
renamed `.png` reaches the agent labelled with what it is.

Two numbers are deliberately not shared. `MAX_IMAGE_BYTES` is 8 MiB and is **not**
`files::model::MAX_FILE_BYTES`: that one is 2 MiB and answers how much text a textarea will open
without freezing the window, while this one answers how big a screenshot is, and a full-screen retina
PNG routinely lands between the two. A test asserts they are still different. The other is the copy
of that ceiling in `attachments.js`, which exists only so a file certain to be refused is not first
read into an ArrayBuffer and encoded a third larger again; drift there is not symmetrical, since
above Rust's is harmless while below Rust's makes every file between the two impossible to attach at
all. The front end's copy must never be smaller.

**The store is laid out by project, and that layout is the boundary the one deleting thing in this
app works inside.** A picture goes into `attachments/<key>/`, where the key is `cleanup::project_key`:
the folder's own name through the same `slug`, and the FNV-1a hash of the whole absolute path after
it. Three properties are wanted at once — derivable from the path alone, since nothing written down
anywhere can be lost; the same on every run, which is why the hash is written out here rather than
taken from `DefaultHasher`, documented as free to move between Rust releases and so able to strand
every picture under the old name; and safe as a single path segment, since this string is joined onto
the store's root and everything deleted is found by walking the result. The name half is for a person
opening the directory in Finder; the hash is what tells two projects called `app` apart.

**Nothing deletes on its own, at any moment.** Not on start, not on a schedule, not when the new-task
dialog closes on images nobody filed — taking a thumbnail out forgets the path and leaves the file.
The one thing that deletes is `attachments_clean`, at the end of a person's press on the Storage tab,
after `attachments_survey` has told them how many files and how many bytes it is about to take.

**What survives is what an unfinished task still names.** `cleanup::removable` is the rule, pure,
over a list of files and a snapshot of the board: a file whose absolute path appears in any of an
issue's four prose fields — description, acceptance criteria, design, notes — stays if that issue is
anything but `closed`; a file only closed issues name goes; a file nothing names at all goes, and
that third case is most of the rubbish and the reason the directory stops growing. The four fields
are deliberately more than the prompt asks for, because the agent decides where the link lands and a
field too many costs a file kept for nothing while one too few costs somebody's screenshot. There is
no record of which task a picture belongs to and there cannot be one: nothing comes back from
`bd create` saying which id it wrote — the same missing channel `claimedBy` reconstructs around.

**An empty board and an unreadable board are the same `Snapshot` and opposite facts**, and keeping
them apart is `cleanup::refusal` — the guard both commands ask before anything is listed. `open()`
resets the store and then ignores whether the first sync worked, so a worker that cannot reach bd
sits with a project open and an empty snapshot; `removable` reads that as "no task refers to any of
these files" and the sweep takes every attachment of every live task in the project. The ways in are
ordinary rather than exotic — no bd on the machine, a version mismatch, a damaged `.beads`, or a
folder with no tracker at all, which the app deliberately keeps open so `bd init` can be offered. So
`Request::Current` carries `Health` beside the snapshot, in the same message as the emptiness it
explains, and anything but `Ok` refuses with `NoBoard` — the rule `runs/browser.rs` sets for the whole
repository: anything unobservable reads as "no", loudly. The survey counts zero in that state rather
than counting everything as rubbish, because a number offering the whole folder under a button that
refuses to press is the same lie told quietly; the front end's `canClear` holds the button on the
same field, and treats a health word it has never heard of, or a missing one, as unread.

**The button reaches one project's folder and physically cannot reach another's.** The directory is
`store_root()/project_key(dir)` where `dir` comes from the tracker worker — `Request::Current`, which
answers with the folder being watched *and* the board it holds in one message, so the two cannot name
different projects across a switch. Everything deleted is that directory joined with a name out of
its own `read_dir`, checked once more by `plain_name`; no subdirectory is entered and no string from
the front end reaches the sweep. Reading every open project's tracker instead was refused: a project
closed in the list would still go unread, and its live tasks would lose their pictures while looking
like nobody's. The files in the root of `attachments/` from before the split are out of reach for
that reason and stay for good — they belong to no project, so there is no board to ask about them,
and they are finite. An attachment made while no project is open also lands in that root: the honest
place for it rather than a fallback, since the root is the part of the store nothing sweeps.
