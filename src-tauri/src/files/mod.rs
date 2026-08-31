//! The project's files: the tree in the left panel and the tabs in the centre.
//!
//! `model.rs` is the vocabulary and the pure logic and carries the tests,
//! `fs.rs` is the disk, `commands.rs` is thin commands over it.
//!
//! **Copying, moving and renaming are three verbs and not one with a flag.** A
//! rename takes a **name**, and what checks a name is `resolve_new_within` —
//! the split into a folder and a name that `files_create` is built on — while a
//! copy and a move take a **folder** and check that one is not inside the
//! other. Two of the three carry ceilings of their own (`MAX_COPY_ENTRIES`,
//! `MAX_COPY_BYTES`) for the reason every ceiling here exists: there is no
//! progress bar, no cancel and no watcher, so an unbounded copy is a frozen
//! panel with nothing on screen to say why. A name already taken is never
//! overwritten and never asked about — the newcomer takes the next name,
//! `report copy.md` and then `report copy 2.md`.
//!
//! **With one exception, and it is the property that costs somebody a file, so
//! it is written here and not only where it lives.** A copy claims a name by
//! *trying* to make the entry — `create_new`, `create_dir`, `symlink` all
//! refuse when something is there — and cannot overwrite anything, whoever else
//! is writing into the folder. A **move** cannot be built that way: `fs::rename`
//! replaces whatever is at the destination without a word, and a conditional
//! rename exists on Linux alone (`renameat2`), so `put_move` looks first and
//! renames second and there is a window between the two. `rename_entry` carries
//! the same window for the same reason. What closes it in practice is that the
//! loop only reaches a second name because the first was taken — and what does
//! not close it is anything in this module, which is why an agent writing into
//! the destination folder at that instant is a real, if narrow, way to lose a
//! file.
//!
//! **A listing is a `read_dir` and one spawn of git.** It was only the first for
//! most of this module's life, and `fs.rs`'s header still opens on what follows
//! from that — no worker, no queue, no watcher — which is all still true. What is
//! no longer true is the cost: `list_dir` asks `git check-ignore` which of the
//! entries it just read are ignored, so the tree can draw those rows muted.
//!
//! **The numbers, measured rather than guessed** (Apple Silicon, macOS, warm
//! cache, `git check-ignore -z --stdin` in the shape `mark_git_ignored` makes
//! it). An ordinary listing of a couple of dozen names in this repository, whose
//! index holds 508 files: **7 ms median**, against a `read_dir` measured in
//! fractions of one. A full `MAX_ENTRIES` listing — 1000 names — in a small
//! repository: **16 ms**.
//!
//! **It is the index that scales it, not the listing.** `check-ignore` consults
//! the index, which is what buys the `git add -f` case, so the cost follows the
//! size of the repository rather than the size of the folder: the same 1000-name
//! listing measured against a 50 000-file index came to **0.5 s**. That is the
//! worst case worth knowing about — a wide folder in a monorepo — and what it
//! costs is the tree rows arriving a beat late, not a stalled window, for the
//! reason below.
//!
//! **Two multipliers.** `catchUp` in `views/DesktopApp.vue` re-lists **every open
//! folder** when the window is focused, so a focus costs one git spawn per
//! expanded folder — a dozen at worst — and `refreshDirs` in `stores/files.js`
//! fires them as a `Promise.all`, so they arrive at once rather than in turn.
//! That is precisely why `files_list` runs its work on the **blocking pool** and
//! not in the body of its `async fn`: see `commands.rs`, and `vcs/commands.rs`
//! for the rule it follows. None of it can fail in a way a person sees.
//!
//! **`clipboard.rs` is the one file here that is not about this project's
//! files**, and that is what puts it beside them rather than under `vcs/` or in
//! a module of its own: it is the machine's clipboard, holding absolute paths
//! that may name anything on the disk, and the only reason it exists is that a
//! paste in the tree has to be able to land a file somebody copied in Finder.
//! It is also the only file here that talks to a platform API rather than to
//! `std::fs` — three of them, one per platform, none interchangeable with
//! another. Its header carries the formats and the reasons; what matters from
//! out here is that **it is allowed to fail and nothing above it is**. A
//! clipboard that will not answer answers an empty list, and the paste rides on
//! the tree's own record, which is `stores/files.js`'s half.
//!
//! **`copy_external_entry` in `fs.rs` is the other half of that**, and it is the
//! one call in this module whose source is not checked against the project root.
//! That is the point rather than an omission: a file copied in Finder is
//! ordinarily somewhere else entirely, and copying it **into** the project is
//! what a paste means. Only the destination is resolved with `resolve_within`,
//! and everything else — the containment check, the ceiling, the free name, a
//! link copied as a link — is the same code `copy_entry` runs.

pub mod clipboard;
pub mod commands;
pub mod fs;
pub mod model;
