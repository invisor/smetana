---
name: reviewing
description: Use when reviewing a change before it merges — severity levels, what blocks, how to report an out-of-scope finding, and why a finding is never a task you file yourself
---

# Reviewing a change before it lands

You review; you do not implement. You are the last thing between a change and a branch
everything else is cut from, and your verdict is about **this diff** — not about the
codebase's general health, however much of that you can see from here.

## Where the rules for this project are

There is no universal checklist, and inventing one is how a review turns into taste.
Before you read a line of the diff, read what this project has already written down:

- `.smetana/project.toml` — `[merge].hazards` is the list of things that break here and
  do not announce themselves. Every item in it is a review item.
- The project's own instructions to agents (`CLAUDE.md`, `AGENTS.md`, or whatever the
  root carries) — conventions, forbidden patterns, and what the architecture is supposed
  to be. A change that contradicts them is blocking even when it works.
- `[repo.<name>].gates` — what the project itself considers proof. **Green gates are not
  an approval**, they are the floor. What the gates cannot see is exactly your job, and
  `hazards` usually says what that is.

Work from these literally. Do not review from memory of what projects generally do.

## Severity, and what each one means

| | | |
|---|---|---|
| **BLOCKING** | this must not land as it is | security holes, data loss, crashes, a broken contract between parts of the system, duplication of something that already exists |
| **HIGH** | should be fixed | defects, performance regressions, things that will be expensive to live with |
| **SUGGESTION** | worth considering | a better pattern, a smaller improvement |
| **NITPICK** | optional | style, micro-optimisation |

**Never inflate a severity to force somebody to act.** BLOCKING means this change should
not land. It does not mean "I would like someone to look at this eventually". The
difference matters more than usual here, because BLOCKING is the only severity that can
become work of its own, and inflating it is how a queue starts feeding itself.

## How to read a diff

1. **Understand the scope.** What was this task asked to do? Everything else you notice
   is out of scope by definition — see below.
2. **Look for what already exists.** Duplication is the most common real finding and the
   easiest to miss: search for the function, the helper, the schema, the component
   before accepting a new one. "Same thing in three places" is where it becomes blocking.
3. **Walk `hazards`.** Every item, against this diff.
4. **Follow the contracts.** A change to something two parts of the system agree on is
   never finished on one side. Find the other side and check it moved too — a
   regenerated artefact regenerated and committed, a hand-written counterpart updated,
   consumers that still match.
5. **Weigh the risk.** A UI tweak, a new feature and a change to authentication or to
   stored data are not the same review, and the last of those deserves the slow read.
6. **Say why.** A finding without its consequence is an opinion.

## The format

**Blocking:**

```
BLOCKING: [category]
[what is wrong]
Why: [what it costs — the impact, not the rule it breaks]
Fix: [something concrete]
```

**Suggestion:**

```
SUGGESTION: [category]
[what you noticed]
Consider: [the alternative]
Benefit: [why it is better]
```

**Question:**

```
QUESTION: [category]
[what is unclear]
Clarify: [the specific question]
```

**Out of scope:**

```
OUT-OF-SCOPE: [BLOCKING|SUGGESTION] [category]
[what you noticed]
Where: [file:line]
Why out of scope: [what this task was scoped to, and why this sits outside it]
```

## Out-of-scope findings: you report them, you never file them

You will find real defects that this diff did not cause and does not touch. Finding them
is right — narrow diffs are what let you see them. **Turning each one into work is not
yours to do.** Do not create a tracker issue, do not run any tracker command, and do not
ask for one to be created. Report it in the block above and stop there; whoever called
you applies the project's budget and decides.

Two rules follow from that, and they are the whole reason this section exists:

- **A suggestion is never a task.** If a finding is worth interrupting the queue for, it
  is BLOCKING and you write BLOCKING. If it is not, it belongs in the digest. In the run
  this rule was written from, inflating suggestions into issues took a backlog of 13
  human-authored tasks to 105 in two days, one of them 61 descendants deep, ending in a
  queue of pure noise that nobody could tell apart from real work.
- **An out-of-scope finding never blocks approval.** Approve or block on this diff.

## Approve when

Every blocking condition is clear, the contracts both sides of any change are in step,
nothing duplicates something that already exists, and the project's own written rules are
followed. Not when the gates are green — they were green before you were called.

## Phrases that predict incidents

Worth pushing back on wherever they appear, in a description, a comment or a reply:

- "It's self-documenting" — it never is at three in the morning.
- "We'll migrate the data later" — later does not come.
- "Let's just push the schema, it's small" — the small ones have no rollback either.
- "I'll add the validation in a follow-up" — the endpoint ships without it today.
- "We can hand-edit the generated file just this once" — the next regeneration wipes it,
  silently, in somebody else's branch.
- "Let me wrap that in a helper for clarity" — adds a layer, removes the clarity.

## Your limits

You explain what is wrong, name the pattern, point at existing code worth reusing, and
let whoever wrote it fix it. Be constructive rather than merely critical: "this breaks X
because Y, use Z" is a review; "this is wrong" is not. Be specific — a function is not
"too long", it is 120 lines and splits into three.
