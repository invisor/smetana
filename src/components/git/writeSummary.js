/* What the corner says after git carried commits from one place to another, and
   the closed list of the writes it says anything about at all.

   Pure, with no Vue and no DOM in it — the family `tracking.js`,
   `gitActions.js` and `branchTree.js` belong to, and for the reason that family
   exists: a `.vue` file is the one thing no test in this repository can reach,
   so the whole of a rule lives outside the component that draws it.

   **The measuring is Rust's and the wording is this file's.** `src-tauri/src/vcs/`
   is the only place in the tree that can read the repository at the two moments
   this needs — immediately before the operation and immediately after — so what
   arrives here is a `Landed` record and never a repository to ask questions of.
   Every field of it is optional, and a missing one is a measurement nobody
   could take: it falls out of the sentence rather than becoming a zero, since a
   zero here is the whole of "nothing came in" and would be a lie about an
   operation that may have brought everything.

   **Four writes and no others.** A checkout, a commit, a branch created,
   renamed or deleted, and an abort all change the repository too, and every one
   of them shows its own result in the same moment: the row names the new
   branch, the change list empties, a row appears or goes. There is nothing for
   a phrase to add, so this answers `null` for them — and this list is the only
   place that decision is written down. */

/* U+2212, the minus this design system draws, and never a hyphen: the gallery's
   own toast has drawn it that way since before this file existed. */
const MINUS = '−'

/* The separator between the counters, which is the one the app already uses
   between identifiers in a single line of secondary text. */
const DOT = ' · '

/* One entry per write this corner speaks for, and the entry is the whole of
   what it says.

   `landed` is the title when something came, `nothing` the title when nothing
   did, and `because` the sentence under that second title — which is the half
   the person actually needs, since "Nothing to merge" on its own reads as a
   refusal rather than as an answer.

   `theirs` is always there by the time these are called; `ours` may not be, on
   a detached HEAD, so the two sentences that name it say nothing rather than
   naming a blank. */
const PHRASE = {
  merge: {
    landed: (ours, theirs) => `Merged ${theirs}`,
    nothing: 'Nothing to merge',
    because: (ours, theirs) => (ours ? `${theirs} is already in ${ours}` : '')
  },
  rebase: {
    landed: (ours, theirs) => `Rebased onto ${theirs}`,
    nothing: 'Nothing to replay',
    because: (ours, theirs) => `${theirs} has nothing this branch does not`
  },
  pull: {
    landed: (ours, theirs) => `Pulled ${theirs}`,
    nothing: 'Nothing to pull',
    because: (ours, theirs) => (ours ? `${ours} is level with ${theirs}` : '')
  },
  push: {
    landed: (ours, theirs) => `Pushed to ${theirs}`,
    nothing: 'Nothing to push',
    because: (ours, theirs) => `${theirs} already has this branch`
  }
}

/* A number this app was actually given, or nothing.

   The three ways a field can carry no answer — absent, `null` from Rust's
   `Option`, or a value that is not a number at all — are one case here, because
   they are one case on screen: a counter nobody could take is left out of the
   phrase. */
function known(value) {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

/* Nothing came in, and it is known that nothing came in.

   **Both counters have to be there and both have to be zero.** One unknown
   number is not evidence of an empty merge, so a record half-measured falls
   back to the plain title and asserts neither thing — the same direction the
   Rust side takes when it refuses to turn a failed measurement into a zero. */
function landedNothing(landed) {
  return !!landed && known(landed.commits) === 0 && known(landed.files) === 0
}

/* `3 commits · 7 files · +41 −12`, with everything unmeasured left out.

   **A zero counter is left out for the same reason an unknown one is.** The
   case where both are zero has its own sentence above, so a zero reaching here
   is one half of a record whose other half is unknown, and `0 commits` in a
   phrase announcing a success says less than nothing. The zero half of the line
   counter goes the same way, which is also what git itself does — its own
   `--shortstat` prints only the half that moved.

   Empty is an ordinary answer: an operation that worked, measured by nothing at
   all, is the title on its own. */
function counts(landed) {
  if (!landed) return ''
  const parts = []
  const commits = known(landed.commits)
  if (commits) parts.push(`${commits} commit${commits === 1 ? '' : 's'}`)
  const files = known(landed.files)
  if (files) parts.push(`${files} file${files === 1 ? '' : 's'}`)
  const lines = []
  const insertions = known(landed.insertions)
  const deletions = known(landed.deletions)
  if (insertions) lines.push(`+${insertions}`)
  if (deletions) lines.push(`${MINUS}${deletions}`)
  if (lines.length) parts.push(lines.join(' '))
  return parts.join(DOT)
}

/* The whole of what the corner says about one write, or `null` for a write it
   says nothing about.

   `{ op, ours, theirs, published, landed }` — the operation, the branch this
   repository was on, the other side of it (the branch merged, the branch rebased
   onto, the upstream pulled from or pushed to), whether the push published a
   branch that had no upstream, and what git moved.

   A branch being published is its own answer and takes no counters: there was
   no upstream to measure against, so the record is empty by construction, and
   what happened is not that commits arrived somewhere but that the remote has
   heard of this branch at all. */
export function writeSummary(write) {
  if (!write) return null
  const { op, ours, theirs, published, landed } = write
  const phrase = PHRASE[op]
  if (!phrase) return null
  if (op === 'push' && published) {
    return ours ? { title: `Published ${ours}`, description: '' } : null
  }
  /* Nothing here can be said about a side with no name. Every one of the four
     has one in the app — the row that was pressed, or the upstream the caption
     was drawn from — so this is for a caller that went round the button. */
  if (!theirs) return null
  if (landedNothing(landed)) {
    return { title: phrase.nothing, description: phrase.because(ours, theirs) }
  }
  return { title: phrase.landed(ours, theirs), description: counts(landed) }
}
