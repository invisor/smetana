---
name: reviewing-branch-changes
description: Use when reviewing the difference between two refs — one repository or several — and writing a report a person will read afterwards
---

# Reviewing what a branch changed

Somebody has asked what one branch does to another and wants an answer they can read.
Nothing is being merged, there is no task behind this, and there is no queue waiting on
your verdict — the report is the whole of what this session produces.

That is what makes this different from `reviewing`, which is the review a change gets on
its way into a target branch. There, severity means "does this block the merge", and the
answer is needed in the next few minutes. Here the pair of refs is whatever was asked
for, the answer outlives the session, and the person reading it may not be the person who
wrote the code.

## 1. What you are reviewing

The prompt names the pairs, one line per repository:

```
<repo>: <base> → <head>
```

The refs are already resolved — `main` is a local branch, `origin/main` is
remote-tracking — and which of the two was meant was decided before you were started.
Use them exactly as given. Do not substitute one for the other, do not fetch in the hope
of a better base, and do not guess at a repository that was not named.

For each pair, in order:

```bash
git -C <repo> diff <base>...<head> --stat
git -C <repo> diff <base>...<head>
```

**Three dots, not two.** `<base>...<head>` is what the branch added since the two
diverged; `<base>..<head>` is that plus everything the base did in the meantime, which is
somebody else's work arriving in your report as this branch's doing. The `--stat` first,
so you know the shape and size before you start reading, and the full diff after it.

A pair whose diff is empty is a finding of its own — say so, and say which of the two
refs you were given, rather than reporting nothing about that repository.

## 2. Where this project's rules are

There is no universal checklist, and inventing one turns a review into taste. Before you
read a line of the diff, read what the project has already written down about itself:

- `.smetana/project.toml` — `[merge].hazards` is the list of things that break here and
  do not announce themselves. **Every item in it is a review item**, and you walk all of
  them against this diff.
- `CLAUDE.md` and `AGENTS.md` in each repository, or whatever the root carries in their
  place — conventions, forbidden patterns, and what the architecture is meant to be. A
  change that contradicts them is a finding even when the code works.

`.smetana/` is kept out of the repository, so it exists in the project root and nowhere
else — a review running somewhere else may find no such file. **If you review without the
hazards, say so in the report.** A verdict reached without them is still a verdict; it is
a narrower one, and the person reading it has to know which of the two they are holding.

## 3. Read the file, not the diff

For every changed file, open it whole. A diff has no context: it shows you the lines that
moved and hides the invariant three functions up that says why they were the way they
were. Most wrong findings, and every expensive one, come from reviewing the hunk instead
of the file.

## 4. Find every place the change is used

A change to something used in more than one place is finished in exactly one of them
until you check. For each changed function, component, export, constant or signature:

```bash
grep -rn "<name>" <repo>
```

Then open each call site and check it against the **new** signature or the **new**
behaviour, not the old one. A caller that still compiles under a changed meaning is the
failure this step exists to catch, and it is invisible in the diff — the caller is not in
it.

The same walk applies to anything two parts of the system agree on: a generated artefact
and its source, a list written out in two files, a constant duplicated across a language
boundary. If the diff moved one side, find the other.

## 5. Try to disprove every finding before you write it down

**This is the most valuable step in this skill.** Do not skip it because the finding
looks obvious.

Before a finding goes into the report, re-read the code around it and ask: *what would
have to be true for this code to be correct as it stands?* Then go and find out whether
it is. Much of what looks strange is deliberate, and much of what is deliberate is
explained in a comment a few lines away.

**If a comment explains the decision and your objection does not address that comment,
delete the finding.** Not soften it — delete it. An objection that has not read the
answer already written next to the code is noise, and it costs the report its credibility
for every real finding beside it.

Five firm findings are worth more than twenty speculative ones. A report somebody stops
trusting is a report nobody reads.

## 6. Every finding is a concrete failure scenario

A finding says: *this input, or this state, produces this wrong behaviour* — and names
the file and the line where it happens.

"There may be a problem here", "this could be risky", "consider whether this is safe" are
not findings. If you cannot write the scenario, you have not finished step 5; either
finish it or drop the finding.

Show the code. Quote the few lines that matter, name the file and line, and say what the
consequence is rather than which rule was broken. A finding without its consequence is an
opinion.

## 7. Three levels, and how ready this is

| | |
|---|---|
| **Blocking** | this must not land as it is: data loss, a security hole, a crash, a broken contract between two parts of the system, a caller left behind by a changed signature |
| **Serious** | should be fixed: defects with a narrower blast radius, missing error handling, avoidable duplication, a performance regression, a break with a pattern this project holds everywhere else |
| **Minor** | optional: naming, readability, a refactoring worth considering |

**Never inflate a level to make somebody act.** Blocking means the change should not land
as it is. It does not mean "I would like attention paid to this".

Close with a readiness percentage, and **say what the scale means** in the report itself
rather than leaving the number to be interpreted:

- **90–100%** — ready as it stands; findings are minor or absent
- **70–89%** — serious findings, none of them blocking; it can land after they are talked
  through
- **below 70%** — at least one blocking finding; it goes back for work

## 8. The report: two files, and the HTML stands alone

The prompt gives you a path with no extension. Write both files:

- `<report>.md` — the review in Markdown
- `<report>.html` — the same review, same structure, same findings

Same content in both. They are two renderings of one document, not a summary and a
long form.

**The HTML is opened inside an application, in a sandboxed frame.** These are
requirements, not preferences:

- no external stylesheet — every style is in a `<style>` block in the document
- no font from a network — system font stacks only
- no `<script>`, of any kind, inline or otherwise
- no `<img>`, no external icon, nothing fetched from anywhere

The document reaches nowhere at all. Anything it needs, it carries.

**It has to be a whole page with an explicit `<html>` tag, not a fragment.** Smetana draws
it in a frame whose DOM it cannot reach, so it hands the document its theme by writing a
`data-theme` attribute onto that tag, and a document with no root tag is never handed one.

Declare the palette four times over, in this order:

```css
:root { /* the light one */ }
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { /* the dark one */ } }
:root[data-theme="dark"] { /* the dark one */ }
:root[data-theme="light"] { /* the light one */ }
```

The first pair is for a browser, which has nothing of ours loaded and only the machine's
answer to go on; the second is for a tab of this app, and it has to win in both directions
— so guard the media query with `:not([data-theme="light"])` and write the two attribute
blocks after it. Either of those two carries the hard case on its own — a light app on a
dark machine — and both are here because they fail differently: the guard says which
reader the query is for and survives being moved, while source order says nothing and does
not.

Write the colours as custom properties on `:root` and refer to them everywhere else, so
that a block is a list of names redefined rather than the whole stylesheet written out
four times.

**Escape everything that came out of the repository.** Every code fragment, every file
name, every branch name, every quoted diff line goes through HTML escaping: `&` first,
then `<`, `>` and `"`. A single unescaped `<` in a quoted line of code silently eats the
rest of your report in the frame, and a review that reads as if it stopped halfway is
worse than no review.

Say in both files which pairs you reviewed, ref for ref, and whether you had
`[merge].hazards` to work from.

## 9. You never file what you find

You will find real defects. Reporting them is the job; **turning them into work is not
yours to do.** Do not create a tracker issue, do not run any command that writes to a
tracker, and do not ask for one to be created — the same rule the `reviewing` skill holds,
and for the same reason: a reviewer who files their own findings is a queue that feeds
itself, and nobody downstream can tell the noise from the work.

The report is where a finding goes. Whoever asked for the review decides what happens to
it.
