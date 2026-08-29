/* How git's own refusal is drawn: a failed-red title, and git's stderr under it
   exactly as git wrote it.

   Out here rather than in either component that draws one, for
   `promoteTitle.js`'s reason one directory over — two places need the same
   thing and neither may guess at the other's. `GitPanel.vue` draws this block
   twice, for a read that failed and for a write git declined, and
   `DeleteBranchModal.vue` draws it a third time inside the window that asked,
   where the refusal is one `-D` would only repeat. A person who has seen one of
   these has seen all of them, and three copies of five declarations is three
   places for the mono to go sans in.

   Style objects rather than a component, which is deliberate: the two live in
   different boxes — a flex item of a column in the panel, ordinary flow in the
   dialog — and the surrounding layout is the caller's. What is shared is what
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
