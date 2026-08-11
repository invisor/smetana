---
name: resolving-questions
description: Use when a parked bd task's open questions have to be put to a person and the answers written back — reading the parked notes, asking one at a time, folding the outcome into the description, and unparking the task
---

# Resolving a parked task

A parked task is one an agent gave up on. It ran into something it could not
settle from the code, the spec or the skills, and rather than guess it wrote the
question down and moved on — that is what `smetana:running-tasks` tells it to do
in an automatic run, and what a lead does by hand when there is nobody awake to
ask.

You are the other half of that. A person is at the terminal now. Your job is to
put the questions to them, write what they answer into the issue, and put the
task back in the queue — in that order, and only if the questions actually get
answered.

## The questions are in the notes

```sh
bd show <id> --json
```

Every open question is a line in `notes` beginning `parked:`. That prefix is
written by two things and means the same in both: the run's own parking
(`runs::queue::parking_note` in the app) and a lead following
`smetana:running-tasks`.

An answered question has a `resolved:` line after it. **Everything below the
last `resolved:` line is still open**; anything above it went through a session
like this one and is settled. That is the pairing rule the app itself reads when
it decides whether to warn somebody, so keeping to it is not bookkeeping — write
the answers any other way and the app will go on saying the task has open
questions.

A task parked with no note at all is an ordinary case, not a broken one.
Somebody parked it by hand. Read the issue, work out what is actually undecided
in it, and ask about that.

## Asking

**One question at a time.** Wait for the answer before the next one. A person
handed four questions at once answers two of them.

**Ask it in your own words, with the context they need.** The `parked:` line was
written by an agent for its own benefit, mid-task, and often names a file or a
symbol the person has never looked at. Read enough of the issue and the code to
put the question in terms of the work, and say what turns on the answer — what
you would do differently either way.

**Answer nothing yourself.** The task is parked precisely because guessing was
not good enough for the agent that stopped. If you can settle a question by
reading the code, then it was never a question — say so, say what you found, and
move on to the next one. What you must not do is pick the more likely of two
answers and carry on as though somebody said it.

**Follow the thread.** If an answer opens a further question, ask that too. If
an answer makes the task itself wrong — the thing being asked for turns out not
to be wanted — say so and stop; that is a task to close or rewrite, not one to
unpark.

## Writing it down

When every open question has an answer, and not before:

1. **Fold each decision into the description.** The description is the spec:
   `smetana:provisioning` reads it to decide whether the task can be started at
   all, and whoever implements it works from that and the repository. A decision
   that lives only in the notes is a decision the implementer never reads.
   Put it where it belongs — under `## Design` if it is an approach, in
   `## Acceptance Criteria` if it settles what done looks like.

2. **Make the acceptance criteria real.** This is the usual reason a task was
   parked in the first place: nobody could say what finished looked like. If the
   answer settles that, the criteria have to say it now, in terms somebody can
   check off. Do not leave them as they were because they technically exist.

3. **Note one line per question.**

   ```sh
   bd note <id> "resolved: <the answer, in one line>"
   ```

   One per `parked:` line, so none is left looking unanswered. The line is a
   summary for somebody scanning the notes — the real content is in the
   description, and the note points at the decision rather than repeating it.

4. **Then, and only then, unpark it.**

   ```sh
   bd update <id> --status open
   ```

The status is the last write. A session interrupted between the note and the
description leaves a parked task with a stray note, which costs somebody a
minute of reading; one that unparks first and is then interrupted puts the task
back in the queue with the answer written nowhere, and the next agent to take it
parks it again on the same question.

## When it does not resolve

The person cannot answer. They need to talk to somebody else, or look at
something that is not in front of them, or they simply say they do not know.

**Leave the task parked. Change nothing about its status.** Say what is still
open and, if the conversation produced anything at all — a narrowing, a
rejected option, who has to decide — note it:

```sh
bd note <id> "parked: <what is still open, now more precisely>"
```

A task left parked has cost nothing. A task put back in the queue with its
question still open costs the next run the same night the last one lost, and
this time with a person's implicit blessing on it.

## What this is not

This is not the task. You are not implementing anything, not cutting a worktree,
not touching the code beyond reading enough of it to ask a decent question. The
work is somebody else's, later, from a task that now says what it needs to say.
