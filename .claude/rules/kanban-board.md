---
paths:
  - "src/components/kanban/**"
---

# The column order, and what of the board is drawn

`components/kanban/columnOrder.js` says it plainly: bd owns which columns exist, the settings own
only the sequence, and this file is the reconciliation between them — pure, no Vue and no DOM, which
is what makes it the one part of the reordering a test can reach. The stored order is per project,
because the set of statuses is: bd carries custom ones and one repository's status has no meaning in
another's order.

A stored order is a **hint, never the truth**. A status bd no longer has cannot be conjured onto the
board by a line in a settings file, and a status bd grew since the last visit has to appear even
though nothing stored names it. So columns the stored order knows are drawn in its sequence and the
rest go after them in bd's own order — appended rather than dropped, since a column nobody has
arranged yet still holds issues, and appended rather than slotted back into bd's position, since
there is no honest position left once the neighbours have been moved by hand. Names matching nothing
are passed over rather than pruned, so a custom status deleted and recreated finds its old place.

`moveColumn` returns the very array it was given, by reference, when nothing moved — an out-of-range
index, or a move to where the column already is. The caller leans on that identity to tell "nothing
happened" from "something did" without comparing contents.

**Which of those columns are drawn, and which of their cards, is the second question and a separate
file**: `boardView.js`, over the global `kanban` settings. `DesktopApp.vue` composes the two —
`orderColumns` first, then this — and the order is deliberate, since the sequence of the columns is a
property of the whole board and must not depend on which of them happen to be on screen today, or a
column would come back from a hidden spell somewhere it never was. Both settings default to today's
behaviour exactly, and its two closed lists are the doubling against `settings/model.rs` that
`.claude/rules/settings.md` names.

**What order the cards inside a column sit in is the third question, and `cardOrder.js` is where it
lives** — pure and outside the component for the same reason as its two neighbours. It answers for
exactly one column: done goes newest-finished first, and every other column keeps the order the
store's `Map` yields, which is to say the order bd's snapshot arrived in. That is a decision rather
than an unfinished job — "what was closed last" has no counterpart in ready or running, where a
person's own priorities decide the sequence and the tracker holds no date standing for them.

The key is `closed_at` and deliberately not `updated_at`. The second is always there, which is what
makes it right for the board's period setting in `boardView.js` — that rule asks whether a task
*moved* and must have no holes — and wrong here: every write by an agent freshens it, so a task
closed last week and edited this morning would surface as the most recently finished thing on the
board. So the card carries `closedAt` beside `updatedAt` (`stores/tracker.js`, `boardColumns`), as bd
wrote it; sorting off the issue instead would mean keeping cards in the bucket and fetching the sort
key from another structure, which nothing else on this board does.

Three edges, each of them cheap and each covered by a test. **Equal times break on the id, ascending**
— a batch merge closes several tasks inside one second, and `Array.prototype.sort`'s stability is no
help when the incoming order is the `Map`'s, which is the snapshot's order after a `tracker_resync`
and the upsert order during a session; two cards would trade places by themselves between the two.
**An unreadable or absent `closed_at` falls back to `updated_at`**, since the field is optional in
`tracker/model.rs` and a rule with a hole in it leaves a card wherever insertion put it. **Neither
readable sends the card to the bottom** and never off the board: out of place costs a glance, missing
costs somebody's work.

It is applied in `boardColumns` rather than in `DesktopApp.vue`'s composition, which is why the order
is one fact for the whole app instead of one per reader — that computed is already a projection for
the interface, translating statuses through `toUiStatus` and working out the Blocked column, and a
store importing a pure module out of `components/` is settled practice here (`runs.js`,
`notifications.js`, `settings.js`, `vcs.js`). The rule matches on the design system's `done`, the
column, and not on bd's `done` *category*: this repository puts exactly one status in that category —
`closed` — while its own `ready_to_merge`, `parked` and `human_check` come through as `unspecified`,
so there would be nothing to tell apart, and `boardColumns` would have to carry a column's category
through when today it drops it on purpose.

`columnHelp.js` is the third of that family and holds what a column *means* — the sentence a person
gets after holding a column head for two seconds. It is deliberately not a line beside the glyphs in
`status/status.js`: that file is the design-system layer and answers what a status *looks* like,
while "which tasks end up here" is knowledge about this board and this project's way of working —
runs, parking, findings that turned up during a review. Two questions, two files, and nothing in the
tooltip explains bd, because a person hovering a column head is asking about their tasks.

Moving a whole deferred column into the queue is `PromoteColumnModal.vue`, the one bulk write to the
tracker in the app. The count is the entire content of the question and sits in the title, because
there is no undo — putting a task back is one issue at a time in the inspector — and it is a snapshot
taken at the press rather than a live reading, since a number that moved between being read and being
confirmed would describe a set nobody agreed to. Each issue costs about two seconds, so a column of
twenty is most of a minute: the dialog owes progress rather than a spinner, and afterwards how many
landed and how many did not.

**Every id in that column came out of Deferred, so the write it makes leaves a trail beside the
status.** `confirmPromote`'s loop (`DesktopApp.vue`) sends `status: 'open'` and `append_notes:
promotedNote('column')` in the same `bd update` — the note text is `components/run/promotedNote.js`,
beside `readyPromote.js` for the reason that whole family lives outside a `.vue` file — because a run
finding the very same task back in `bd ready` a batch later has no other way to tell a person's
promote from a status that merely slipped (smetana-fpw7). The other two places a person can move a
task to ready the same way — starting a run over one card, and a direct status change from the card
menu or the inspector header — leave the identical `promoted:` note; see `.claude/rules/runs.md` for
the marker itself and why the running-tasks skill reads it as the sign nothing needs undoing.

`components/run/branchChoice.js` is the next of that family and was pulled out for the same reason:
a `.vue` file is the one thing no test in this repository can reach, so the whole of the rule filling
the run dialog's branch field lives outside the component. `pickBranch` is three steps in one order —
what this project was left at last time, then its own `[defaults].target_branch`, then whatever the
list puts first, which is the most recently worked-on branch because `target_branches` orders by
reflog. A remembered name that is no longer in the list is skipped in silence rather than offered,
since a branch deleted since it was remembered would sit in the field as an option that fails on the
first merge. The list itself holds `{ name, missing_in }` records rather than bare strings:
`needsCutting` is the single rule behind both the field's hint and the run's `create_target`, and
`branchOptions` is what splits the two groups the field draws.

The defect it was written for was not the rule being wrong but the rule running **once**, against a
list that had not arrived yet (smetana-6gs, smetana-o8r): the dialog is shown first and the branches
are fetched afterwards, so the fill on opening ran against nothing and the field opened on "Pick a
branch" with Run disabled, which left the remembered branch, the config default and the fall-back to
the most recent branch all dead at once. A watcher now refills when the list lands — **but only while
nobody has chosen**. That is what `branchChosen` guards, and it is why the control is deliberately not
on `v-model`: through `v-model` a fill and a person's pick are the same assignment and nothing
downstream could tell them apart, so a late answer would overwrite a choice somebody had already made.

The other half of that fix is in `git.js`, and it is a trade taken with its eyes open. `loadBranches`
clears the list when it belongs to *another* project — offering the branches of a repository somebody
has already left is worse than offering none — but leaves **this** project's list in place while it
reads it again, because clearing unconditionally emptied the field under the dialog that had just
opened. The cost is that for the length of one call the field can be filled from a list one read out
of date, and somebody picking a since-deleted branch inside that window has the choice frozen by
`branchChosen`, so the run goes out against a branch that is not there. Clearing first made that
impossible — by keeping Run disabled every time, for everybody.

## Locking a task by hand (smetana-44mw)

`Block` and `Unblock` in the card menu — `taskMenu.js`, directly above `Move to…` — are a person's own
hold on a task, separate from the dependency the Blocked column already computes. **A lock is bd's own
stored `blocked` status and not a label**, which was the rejected design: a label would have left a
locked card sitting in its own column, and the filter that keeps it out of a run's hands would then
have had to be written three times over — in `runs/queue.rs`, in the prose of two skills an agent
reads, and in `runnableTask` here — while `bd ready --claim` takes a task atomically before an agent
reads any of that prose. The stored status needs none of it: `bd ready` already refuses an issue held
at `blocked`, so the Rust queue and the running-tasks and provisioning skills refuse a locked task
today, with nothing new to teach any of them. The card moves to the Blocked column for the same reason
a computed dependency does, and it needs its own look there (`TaskCard.vue`, below) because the two
things share a column and nothing else — one leaves on its own, the other never does.

**The cascade is `components/kanban/lockCascade.js`**, pure and outside `DesktopApp.vue` for the reason
this whole family exists: a `.vue` file is the one thing no test here can reach. `lockIds(issues, id)`
answers every open descendant through `parent`, deepest first, then `id` itself if it is open — so
locking an epic never leaves it `blocked` over a child still `open`. `unlockIds(issues, id)` answers the
reverse: `id` first if it is locked, then its locked descendants, shallowest first — so nothing comes
back before the ancestor that was holding it. A descendant sitting anywhere else — claimed, reviewed,
parked, deferred, closed, pinned, hooked or a project's own custom status — is left exactly where it
is, because it is already out of a run's reach for a reason of its own and locking or unlocking it
would be answering a question nobody asked. `parentBlocked(issues, id)` walks the same `parent` chain
upward and answers whether any ancestor — not only the immediate parent — is itself locked; the merge
lock label is filtered out of every one of the three, the same exclusion `boardColumns` makes.

**The write is `DesktopApp.vue`'s `toggleLock`**, one `updateIssue` per id in the cascade's own order,
the same one-at-a-time shape `confirmPromote` already uses above and for the same reason — bd's worker
serialises writes anyway. Every id the cascade will touch is added to `writingIds` — a `reactive(new
Set())` that replaced the single `writingId` ref precisely for this, since a cascade greys several
cards at once rather than one — before the first write goes out, so a descendant three levels down
reads as busy from the first frame rather than only once its own turn comes; each id leaves the set the
moment its own write lands, success or failure alike. The loop stops on the first refusal, the same way
`confirmPromote`'s does, and a project switch mid-cascade stops it too — what the loop never reached is
dropped out of `writingIds` in a `finally` rather than left grey for good.

**Only Unblock leaves a `promoted:` trail, and Block leaves none at all.** `promotedNote('unblock')`
rides beside every `open` write the unlock cascade makes, for the same reason `run`, `column` and
`status` already carry one (`.claude/rules/runs.md`): a lead who sees the task back in `bd ready` a
batch later has no other way to tell a person's own release from a status that merely slipped. Block
writes nothing beside the status, because the stored `blocked` status *is* the whole of the record and
the actor who set it already stands in bd's own history — there is nothing here for a later reader to
mistake for a slip.

**The menu row itself is one row wearing two labels, never two rows.** `taskMenuItems` in `taskMenu.js`
draws `Block` (glyph `lock`) on a card bd holds `open` — Ready and the computed Blocked column alike —
and `Unblock` (glyph `lock-open`, registered in `core/icons.js` beside `lock`) on one it holds
`blocked`; on every other status there is no row at all, the same absent-rather-than-greyed trade the
`resolve` and `fix` rows above it already make. The new `parentBlocked` argument only ever greys
Unblock, with the reason written into the row itself — `Unblock — its epic is blocked` — since
`ContextMenu` clips a row and gives it no tooltip to recover the rest from; it never touches Block,
because nothing stops a person locking one more task under an epic that is not itself locked. The same
fact greys `Move to… → Ready` on that card through the second door of the identical menu, and leaves
Pinned and Done alone — a straight move to Ready would free one task from a lock the epic is still
holding it under, which is the one write this rule exists to stop. `parentBlocked` is computed once per
card in `orderedColumns`, the way `runnable` already is, and rides in the task object so the card's own
menu and the Task & details header's copy of it (`inspectedMenu`) read the identical fact.

**`TaskCard.vue` draws the locked variant only at bd's stored `blocked`** — a card waiting on a
dependency, in the same column, is untouched. `--surface-sunken` at rest, rising to `--surface` under
the pointer and never all the way to the `--surface-raised` an ordinary card stands on, so it keeps
reading as held down even while somebody is looking straight at it; `--status-blocked-border`; the
title steps from `--text-secondary` to `--text-primary` on hover by colour, deliberately not by
`--attn-quiet-opacity`, since that token belongs to `done`'s dimming and a locked card is not quiet, it
is held. No dashed `DependencyBand` — that hatching promises a release nobody is coming to give — while
the `DependencyMark` counters stay, since the dependency count is still a fact worth showing. The
footer leads with `StatusBadge status="blocked" size="sm"`, already drawing the lock glyph and the
`--status-blocked-*` tokens with nothing new to teach it, ahead of `TypeBadge`'s new `muted` prop — a
bare `--border` outline and `--text-muted`, no fill and no type hue — because a locked card is one
argument, not two colours competing for the same attention. `selected`, `drop-target`, `dragging` and
`flash` are untouched: those states win outright over the locked look exactly as they already win over
the ordinary one.

**`columnHelp('blocked')` names both reasons a card can be in this column** — waiting on another task
that will move it to Ready by itself, or locked by a person and staying until that same person unlocks
it — and still says neither `parent` nor `bd`, the same constraint the rest of that table keeps: a
person hovering a column head is asking about their tasks, not about the tracker.
