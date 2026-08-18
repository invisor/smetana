/* What a branch's standing against its upstream looks like: the marks on a row,
   and the two buttons in the section header.

   Pure, with no Vue and no DOM in it — the family `gitActions.js`,
   `commitBox.js` and `branchTree.js` belong to, and for the reason that family
   exists: a `.vue` file is the one thing no test in this repository can reach,
   so the whole of a rule lives outside the component that draws it.

   It deliberately does not repeat `gitActions.js`. Whether this panel may write
   at all is that file's one verdict, and it arrives here as an argument: a
   second copy of the rule is the half that drifts. What is added here is only
   what is true of these two verbs and of nothing else in the panel — that there
   is an upstream, and that there is something to send or to bring in. */

/* The token, and never a colour: the browser repaints on a theme change with
   nothing here to keep in step. `--git-modified` is the orange the file tree
   and the change list already draw "differs from what is committed" in, and
   "differs from origin" is that sentence one step further out. Deliberately not
   a status hue — `--status-needs-you` is budgeted at one or two rows on a
   screen, and a list can hold ten branches that are behind. */
export const BEHIND_TOKEN = '--git-modified'

/* The ahead count is drawn and does not colour: what was asked for is a branch
   with something to pull, and colouring both would leave them indistinguishable
   at a glance. */
export const AHEAD_TOKEN = '--type-plain-fg'

const NO_MARK = { behind: 0, ahead: 0, orange: false }

/* A record for a branch the tracking read has not answered for is an ordinary
   outcome, not a hole: the branch list and the tracking list are two answers
   merged by name, and either can be one refresh older than the other. */
export function trackingMark(tracking) {
  if (!tracking) return NO_MARK
  const behind = tracking.gone ? 0 : (tracking.behind ?? 0)
  const ahead = tracking.gone ? 0 : (tracking.ahead ?? 0)
  return { behind, ahead, orange: behind > 0 }
}

const NO_UPSTREAM = 'This branch has no upstream yet, so there is nothing to pull.'
const UPSTREAM_GONE = 'The upstream of this branch was deleted on the remote.'

/* Pull is live wherever there is an upstream, including when there is nothing
   behind: asking the remote and finding nothing is a legitimate thing to press,
   and it is how somebody makes the count they are reading current. */
export function pullAction(tracking, actions) {
  const count = trackingMark(tracking).behind
  if (!actions.allowed) return { allowed: false, reason: actions.reason, label: 'Pull', count }
  if (tracking?.gone) return { allowed: false, reason: UPSTREAM_GONE, label: 'Pull', count }
  if (!tracking?.upstream) return { allowed: false, reason: NO_UPSTREAM, label: 'Pull', count }
  return { allowed: true, reason: null, label: count ? `Pull ${count}` : 'Pull', count }
}

const NOTHING_TO_PUSH = 'This branch has nothing the remote does not already have.'

/* Push has a second shape rather than a second button. A branch with no
   upstream — the ordinary state of one cut by `New branch from this` — is
   published, which is `git push --set-upstream origin HEAD` and a different
   word on the control, since "push" for a branch the remote has never heard of
   says less than what is about to happen. A branch whose upstream was deleted
   is the same act: there is nothing there to push to. */
export function pushAction(tracking, actions) {
  const { ahead } = trackingMark(tracking)
  const setUpstream = !tracking?.upstream || Boolean(tracking?.gone)
  const label = setUpstream ? 'Publish branch' : ahead ? `Push ${ahead}` : 'Push'
  if (!actions.allowed) {
    return { allowed: false, reason: actions.reason, label, count: ahead, setUpstream }
  }
  if (!setUpstream && ahead === 0) {
    return { allowed: false, reason: NOTHING_TO_PUSH, label, count: 0, setUpstream }
  }
  return { allowed: true, reason: null, label, count: ahead, setUpstream }
}
