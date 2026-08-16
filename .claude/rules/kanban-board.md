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
