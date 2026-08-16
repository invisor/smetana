---
paths:
  - "src-tauri/src/attachments/**"
  - "src/stores/attachments.js"
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
guards no state — and it is four commands over pure functions that carry the tests: `mod.rs` is the
disk and the vocabulary, `cleanup.rs` is the whole of the deleting rule with no filesystem in it.

Three gestures put a picture in the list and they arrive as only two kinds of thing. A file already
on disk arrives as a path and Rust copies it (`attachment_import`); the clipboard exists inside the
page and nowhere this process can reach, so a paste arrives as bytes (`attachment_write`). Both
answer with the same record, which leaves the strip one shape to draw. The list lives in the store
rather than in the dialog because a drop is not the dialog's event to hear: Tauri intercepts file
drops before the webview sees them and reports them against the *window*.

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
