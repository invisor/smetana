/* What a changed file is captioned with: one mark, one word, one token. The
   mark is a letter for every kind but the conflict, which is `!`; the kind
   nobody has heard of falls through to `?` at the bottom of this file.

   Pure, with no Vue and no DOM in it — the family `branchChoice.js` and
   `boardView.js` belong to, and for the reason that family exists: a `.vue`
   file is the one thing no test in this repository can reach, so the rule lives
   outside the component that draws it.

   A token name and never a colour, so the browser repaints on a theme change
   with nothing here to keep in step. The names are the ones the file tree
   already draws a modified file's mark with (`files/FileTreeRow.vue`): one file
   changed in one repository has to look the same in both places, and a second
   palette for the same fact is how the two halves start disagreeing. */

/* The kinds Rust names (`vcs::model::ChangeKind`), and nothing else — a kind
   this table has never heard of falls through to `UNKNOWN` rather than
   throwing.

   Two marks are worth saying out loud. `U` is untracked, which is VS Code's
   vocabulary rather than git's own `--short` format, where that letter means
   unmerged: the panel is read beside that editor and not beside `git status
   -s`.

   The conflict is not a letter at all. It is `!`, the same mark the file tree
   draws a conflicted file with (`files/FileTreeRow.vue`), and the two matching
   is the point: a merge that stopped leaves the one row nobody may miss, and a
   letter in a column of letters is not enough to say so. It is the loud row
   here in three ways at once, of which this is the first — `conflictsFirst.js`
   puts it above the others and `ChangeList.vue` colours the whole of it rather
   than the mark alone. The tree keys the same fact `conflict` and this table
   keys it `conflicted`, after Rust's `ChangeKind` through serde; the keys stay
   apart while the mark and the token are shared.

   Leaving VS Code's vocabulary for that one kind is what frees `C`, which the
   conflict held here until this table drew it as `!` — and a copy is
   deliberately not moved onto the letter. `C` in `git status -s` is the copy's
   own, so the move would put this table into agreement with that format; the
   panel is not read against it. It is read beside the editor whose vocabulary
   the rest of this table still keeps, and there `C` on a change list is a
   conflict — the very fact this list now says with `!`. A copy wearing it would
   be handing an unrelated row the mark a reader arrives already knowing. `P` is
   borrowed from the letters of the word itself and belongs to nothing else
   here. A copy is very nearly unreachable in any case: `git status` reports
   none without copy detection turned on. */
const KINDS = {
  modified: { letter: 'M', label: 'Modified', token: '--git-modified' },
  added: { letter: 'A', label: 'Added', token: '--git-added' },
  deleted: { letter: 'D', label: 'Deleted', token: '--git-deleted' },
  untracked: { letter: 'U', label: 'Untracked', token: '--git-untracked' },
  /* `!` rather than a letter, and `Conflicted` stays: a bare `!` says nothing
     at all to a screen reader, which is what the label is the accessible name
     for. */
  conflicted: { letter: '!', label: 'Conflicted', token: '--git-conflict' },
  /* The three with no colour of their own. The neutral set rather than a hue
     borrowed from a neighbour: `--git-modified` on a rename would say the file
     was edited, which is the one thing a rename is not. */
  renamed: { letter: 'R', label: 'Renamed', token: '--type-plain-fg' },
  copied: { letter: 'P', label: 'Copied', token: '--type-plain-fg' },
  typeChanged: { letter: 'T', label: 'Type changed', token: '--type-plain-fg' }
}

/* A kind nobody has heard of is an ordinary outcome, not an error: the file did
   change, and leaving it off the list is the one thing this cannot honestly do.
   "Changed" is what is left to say about it. */
const UNKNOWN = { letter: '?', label: 'Changed', token: '--type-plain-fg' }

export function changeStatus(kind) {
  return KINDS[kind] ?? UNKNOWN
}
