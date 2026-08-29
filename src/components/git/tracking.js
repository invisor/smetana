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

/* The one thing this file borrows from its sibling, and it is borrowed rather
   than repeated for the reason `publishes` is exported downwards: which rows
   are drawn above the tree decides which rows a fold is hiding, and two copies
   of that test would put a mark on a heading standing in for a row that is on
   screen. */
import { liftedOut } from './branchTree.js'

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

/* What a folded folder has to say for the branches it is hiding.

   A folded folder leaves its rows out of the list altogether, so without this
   the mark would be missing in exactly the repositories that need it — one
   `feature/` folder holding thirty branches is the ordinary state of this list.
   It answers yes or no and never a number: the heading already carries the
   count of what it holds, and a second number beside it would read as a
   subtotal of the first.

   The prefix test is the folder's path and a slash, so a heading answers for
   everything below it however deep — `fix` is behind because `fix/legacy/…` is,
   which is what keeps the mark on screen while the fold is closed at any level.
   Here rather than in the component drawing it, for the reason the rest of this
   file is here: a `.vue` file is the one thing no test in this repository can
   reach.

   Every branch `branchTree.js` lifts above the tree is passed over however its
   name reads — the current one, and the ones somebody marked as favourites. A
   heading standing in for a row that is on screen anyway would be saying it
   twice, and saying it about a branch the fold is not hiding. Which rows those
   are is `liftedOut` in that same file rather than a second copy of the test
   here: the two have to agree exactly, since the tree is built from what is
   left over. */
export function folderBehind(path, branches, tracking, favorites = []) {
  const prefix = `${path}/`
  return branches.some(
    (branch) =>
      !liftedOut(branch, favorites) &&
      String(branch?.name ?? '').startsWith(prefix) &&
      trackingMark(tracking[branch.name]).orange
  )
}

const NO_UPSTREAM = 'This branch has no upstream yet, so there is nothing to pull.'
const UPSTREAM_GONE = 'The upstream of this branch was deleted on the remote.'
const NOTHING_TO_PULL = 'This branch already has everything the remote has.'

/* Pull is refused when the branch is level, which is the same shape as Push
   one function down: a control offering an act with no effect is a control
   somebody presses to find out, and the two verbs answer that question the
   same way rather than one each.

   It was the other way around once — live wherever there was an upstream, on
   the argument that pressing it is how a person makes the count they are
   reading current. That argument was right about the need and wrong about the
   control: what it describes is a *fetch*, and the panel now has one of its
   own in the same caption. With somewhere else to ask the remote, a live Pull
   over `behind: 0` was only a button whose whole answer was "nothing
   happened". */
export function pullAction(tracking, actions) {
  const count = trackingMark(tracking).behind
  if (!actions.allowed) return { allowed: false, reason: actions.reason, label: 'Pull', count }
  if (tracking?.gone) return { allowed: false, reason: UPSTREAM_GONE, label: 'Pull', count }
  if (!tracking?.upstream) return { allowed: false, reason: NO_UPSTREAM, label: 'Pull', count }
  if (count === 0) return { allowed: false, reason: NOTHING_TO_PULL, label: 'Pull', count: 0 }
  return { allowed: true, reason: null, label: `Pull ${count}`, count }
}

const NOTHING_TO_PUSH = 'This branch has nothing the remote does not already have.'

/* Whether sending this branch means publishing it — the one question the label
   on the button and the arguments git is run with are both answers to.

   Exported, and read by `stores/vcs.js` as well as by `pushAction` below,
   because those two are exactly the pair that must not disagree: the caption
   would otherwise say "Publish branch" while the store ran a plain `git push`
   that git then refuses, or say "Push 2" while the store re-pointed the
   branch's upstream. Neither would fail a test — `tracking.test.js` pins one
   copy and `vcs.test.js` the other, and both go on passing while the two come
   apart — which is the whole reason this rule is one expression in one file. */
export function publishes(tracking) {
  return !tracking?.upstream || Boolean(tracking?.gone)
}

/* Push has a second shape rather than a second button. A branch with no
   upstream — the ordinary state of one cut by `New branch from this` — is
   published, which is `git push --set-upstream origin HEAD` and a different
   word on the control, since "push" for a branch the remote has never heard of
   says less than what is about to happen. A branch whose upstream was deleted
   is the same act: there is nothing there to push to. */
export function pushAction(tracking, actions) {
  const { ahead } = trackingMark(tracking)
  const setUpstream = publishes(tracking)
  const label = setUpstream ? 'Publish branch' : ahead ? `Push ${ahead}` : 'Push'
  if (!actions.allowed) {
    return { allowed: false, reason: actions.reason, label, count: ahead, setUpstream }
  }
  if (!setUpstream && ahead === 0) {
    return { allowed: false, reason: NOTHING_TO_PUSH, label, count: 0, setUpstream }
  }
  return { allowed: true, reason: null, label, count: ahead, setUpstream }
}

const FETCHING = 'Asking the remote what it has…'

/* The third control in the caption, and the only one of the three that is
   about the repository rather than about the branch it is on.

   It is live in every state the other two are refused in, and that is the
   whole reason it exists: with Pull dimmed when the branch is level and Push
   dimmed when it is ahead of nothing, a person reading `behind: 0` had no way
   left to ask whether that number is still true. The background sweep answers
   it every five minutes and answers it not at all with `git.autoFetch` off, and
   a fact somebody is deciding on is a fact they must be able to refresh
   themselves.

   `actions` — the runs verdict `gitActions.js` returns — is deliberately not a
   parameter. That rule is about a write under a batch that may be mid-merge,
   and `git fetch` writes remote-tracking refs and touches neither the working
   tree nor the index. It is the argument that already keeps the commit box's
   sparkle alive under a run, and the store keeps the same line: the background
   sweep goes out under a batch too.

   The one state that refuses is one already in flight, which is not really a
   refusal but the same call saying it is still running — the same word the
   spinner on the control is saying. */
export function fetchAction(fetching) {
  if (fetching) return { allowed: false, reason: FETCHING, label: 'Check the remote' }
  return { allowed: true, reason: null, label: 'Check the remote' }
}
