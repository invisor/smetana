/* What the board's place says when the operating system, not bd, is what is in
   the way.

   It is a module rather than a fourth entry in `HEALTH_NOTICE` because this one
   notice has three forms and the choice between them is a rule: the same
   refusal is drawn everywhere, and what can be done about it depends on where
   the folder is and what platform this is. A rule inside a `.vue` file is the
   one thing no test in this repository can reach, hence the split.

   The three come from Rust's `AccessRepair` (`tracker/access.rs`), which is
   asked per project rather than per build — `~/Desktop/a` and `~/code/b` get
   different answers in the same launch:

   - `reset` — a TCC service macOS will prompt for again governs the folder.
     The one form with a button.
   - `full-disk-access` — macOS, but only Full Disk Access governs it, and macOS
     never prompts for that one. There is a repair and it is not ours to press:
     pressing `tccutil reset` here would clear a grant with no way to ask for it
     back. So the sentence names the place instead.
   - `unavailable` — no `tccutil` on this platform at all.

   The register is `HEALTH_NOTICE`'s and is worth stating, since matching it is
   most of the work here. It names the cause first, in the plainest words there
   are; it says what the button will do before the button does it, because there
   is no confirmation dialog in front of any of these; and it says nothing it
   cannot vouch for. The state this replaces claimed the tracker was probably
   built by an older bd and offered a database migration for a folder nobody was
   allowed to open — so the second clause of all three forms is about what this
   is *not*, and it stays.

   "Restarts the app", not "relaunches": the app already has a control that ends
   itself and comes back, and it says "Install and restart" / "Installing
   restarts the app" (`components/settings/update.js`). One act wants one verb. */

/* The name of the thing doing the refusing. macOS is named where the answer is
   macOS's, and not otherwise: on a platform this app is not sure of, "the
   system" is the true word and a wrong one would be a small lie in the one
   sentence somebody is reading to find out what happened.

   `System Settings → Privacy & Security` is quoted with the operating system's
   own capitals rather than put into this app's sentence case, and joined with
   the arrow `README.md` and the release workflow already use for a route
   through those same settings. It is somebody else's interface, and a person
   reading this is going to look for those exact words on a screen that is not
   ours — which is also why the app is named here exactly as `productName`
   spells it rather than in this app's own prose: that list shows the app under
   a name the bundle carries, so this sentence has to move whenever
   `productName` does. Which of those names the pane reads is not established —
   it resolves a display name, `CFBundleDisplayName` then `CFBundleName` then
   the `.app` filename, and nobody has opened it to look. One case turns on
   that, which is why the question is written down rather than left out: the
   updater replaces a bundle's contents without renaming its directory, so a
   copy updated in place rather than reinstalled still sits in `smetana.app`
   while carrying `CFBundleName = Smetana`. Somebody reporting that the list
   says `smetana` is that copy before it is a bug in this string. */
const DESCRIPTION = {
  reset: 'macOS is refusing this app access to the folder — a permission, not the tracker or the data in it. Resetting it makes macOS ask again, and restarts the app.',
  'full-disk-access':
    'macOS is refusing this app access to the folder — a permission, not the tracker or the data in it. This one cannot be asked for again: grant Smetana Full Disk Access in System Settings → Privacy & Security, then open the project again.',
  unavailable:
    'The system is refusing this app access to the folder — a permission, not the tracker or the data in it. Grant this app access to the folder, then open the project again.'
}

export function folderRefusedNotice(repair) {
  return {
    icon: 'lock',
    title: 'No permission to read this folder',
    /* An answer nobody recognizes falls to the form that offers nothing and
       claims nothing about the platform — the same way this store treats a
       question that never came back. */
    description: DESCRIPTION[repair] ?? DESCRIPTION.unavailable
  }
}

/* Whether a button goes under it. One export rather than the caller comparing
   against a string, so the spelling of Rust's answer is known in exactly one
   place on this side. */
export const folderRefusedHasReset = (repair) => repair === 'reset'
