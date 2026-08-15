/* What a changed file is captioned with: one letter, one word, one token.

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

   Two letters are worth saying out loud. `U` is untracked and `C` is a
   conflict, which is VS Code's vocabulary rather than git's own `--short`
   format, where those two letters mean copied and unmerged. The panel is read
   beside that editor and not beside `git status -s`. A copy therefore cannot be
   `C`, and it takes the next letter of its own word instead: the conflict is
   the loud row here and must never be ambiguous, while a copy is very nearly
   unreachable — `git status` reports none without copy detection turned on. */
const KINDS = {
  modified: { letter: 'M', label: 'Modified', token: '--git-modified' },
  added: { letter: 'A', label: 'Added', token: '--git-added' },
  deleted: { letter: 'D', label: 'Deleted', token: '--git-deleted' },
  untracked: { letter: 'U', label: 'Untracked', token: '--git-untracked' },
  conflicted: { letter: 'C', label: 'Conflicted', token: '--git-conflict' },
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
