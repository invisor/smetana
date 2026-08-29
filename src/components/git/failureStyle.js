/* How git's own refusal is drawn: a failed-red title, and git's stderr under it
   exactly as git wrote it.

   Out here rather than in any one component that draws one, for
   `promoteTitle.js`'s reason one directory over — several places need the same
   thing and none may guess at the others'. **Three components and four blocks**:
   `GitPanel.vue` draws it twice, for a read that failed and for a write git
   declined; `ConflictModal.vue` draws it over "Git did not abort";
   `DeleteBranchModal.vue` draws it inside the window that asked, where the
   refusal is one `-D` would only repeat. A person who has seen one of these has
   seen all of them, and three copies of five declarations were three places for
   the mono to go sans in.

   That count is the reason this header names its consumers instead of saying
   "wherever git's stderr is drawn". It was written naming two and was already
   false: `ConflictModal.vue` had its own copy in this same directory, under a
   comment saying it was "drawn the way `GitPanel` draws it" — the exact
   borrowing this file exists to end. A header that enumerates has to be
   re-counted when a consumer is added, and a header that generalises never
   catches the copy nobody moved.

   Style objects rather than a component, which is deliberate: the boxes around
   them are all different — a flex item of a column in the panel, which carries a
   `flexShrink` and a `gap` that mean nothing anywhere else; ordinary flow under
   a scrolling file list in the conflict dialog; ordinary flow at 440px in the
   delete one. The surrounding layout is the caller's. What is shared is what
   the two lines *look* like and nothing about where they sit.

   Every value is a token reference, like every other style object in this
   system. The bare `/1` on the title is a line height and not a length: a
   single line of sans that has to sit tight against the block under it. */

export const failureTitleStyle = {
  font: 'var(--weight-medium) var(--text-sm)/1 var(--font-sans)',
  color: 'var(--status-failed-fg)'
}

/* Mono and pre-wrapped, because this is machine output and is shown exactly as
   it stands. `overflowWrap: 'anywhere'` is for the paths in it — git names a
   worktree by its absolute path, and one long enough would otherwise push the
   block out of a 440px dialog and a 252px column alike. */
export const failureTextStyle = {
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-mono)',
  color: 'var(--text-secondary)',
  whiteSpace: 'pre-wrap',
  overflowWrap: 'anywhere'
}
