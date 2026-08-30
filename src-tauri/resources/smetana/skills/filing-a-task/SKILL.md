---
name: filing-a-task
description: Use when filing a task into this project's bd tracker from Smetana — the standard a task has to meet before anybody can start on it, which fields bd wants, and how to word them
---

# Filing a task in bd

The board this app shows is the `bd` tracker in the project directory. A task
is filed with `bd create`, and the board picks it up on its own — there is
nothing to refresh and nothing else to notify.

## The bar: nobody can ask you afterwards

A task filed here is picked up by an agent working on its own, hours later,
quite possibly overnight. It reads the issue and the repository, and that is
all it gets: you will not be there, and neither will the person who asked for
the work.

So whatever the issue leaves out becomes one of two things, and both are
expensive. In a supervised run it is a question put to somebody at three in the
morning, and the run waits until it is answered. In an automatic run there is
nobody to ask, so the task is parked and the night's work is one task shorter.

The standard is therefore not "somebody could work out what I meant". It is:
**an agent that has read this issue and this repository, and can ask nobody,
can do the work and can tell when it is done.**

That is why the sections below are required rather than suggested, and why
`--validate` is not optional.

## The command

```sh
bd create --title "<title>" --type <type> --priority <0-4> --validate --body-file - <<'EOF'
<description, sections and all>
EOF
```

Four things about that line, each of which has cost somebody something:

- `--title` rather than the positional form bd's help shows first. bd checks a
  positional title for a leading dash and refuses to create the issue, even
  after `--` — a title like `-n 5 is not enough` comes back as `looks like a
  flag`. A flag's value goes through no such check and is taken as written.
- `--body-file -` reads the description from stdin, and the **quoted** heredoc
  (`<<'EOF'`, not `<<EOF`) is what carries newlines, backticks, quotes and `$`
  through untouched. A description with sections in it passed as `-d "…"` is a
  quoting accident waiting to happen, and the accident silently truncates a
  spec rather than failing.
- `--validate` refuses to create an issue whose description is missing the
  sections its type requires. It is the one mechanical check here — everything
  else is prose that can be skimmed — so never file without it. But know what it
  is worth: it looks for the wording of a heading and nothing else. An empty
  section passes, `### Acceptance Criteria` passes, lower case passes. It
  catches having forgotten the criteria; it cannot catch criteria that say
  nothing, and the agent it was meant to protect will be just as stuck on those.
  Passing it is the floor, not the standard above.
- `--type` is one of `task`, `bug`, `feature`, `chore`, `epic`, `decision`.
  `task` is the default and the right answer when nothing else fits.
  `--priority` is 0 (highest) to 4 (lowest); 2 is ordinary.

## The sections bd requires, by type

| type | headings `--validate` insists on |
|---|---|
| `task`, `feature` | `## Acceptance Criteria` |
| `bug` | `## Steps to Reproduce`, `## Acceptance Criteria` |
| `epic` | `## Success Criteria` |
| `decision` | `## Decision`, `## Rationale`, `## Alternatives Considered` |
| `chore` | none |

`chore` is the one type bd validates nothing for. That is bd's judgement about
chores, not permission to file a vague one — everything under the next heading
still applies, and a chore that needs criteria should have them.

## What every task owes, whatever its type

In the prose above the required sections:

- **What needs doing**, not merely what is wrong. A reader scanning the board
  sees titles and nothing else, so the title carries this too.
- **Where** — the files, modules or screens involved, named. An agent that has
  to find the place before it can start is an agent whose first question is
  where the place is.
- **What it does now**, whenever the task is a change to something that already
  exists. "Make it use the new format" is unworkable without the old one.
- **What is out of scope**, whenever the wording could reasonably be read
  wider than it was meant. This is the cheapest sentence in the whole issue and
  it prevents the most expensive kind of rework.

In the criteria section: things a person could actually check, one per line.
"The toggle is off when neither tool is installed, and its tooltip names the one
that was looked for" is a criterion. "Works correctly" is not — it is the
question restated, and it will come back as a question.

## Write down what was decided, not only what was asked

If you discussed this task before filing it — and Smetana's Brainstorming switch
may well have told you to — **the outcome of that discussion is the most
valuable thing you are holding, and it is the thing most easily lost.** The
person's original few sentences survive in your prompt regardless; half an hour
spent narrowing down what they actually meant survives nowhere at all unless you
write it into the issue.

So everything settled goes into the description, under `## Design`: the approach
agreed on, and the alternatives that were considered and dropped, with why they
were dropped. An implementer who does not know an option was already rejected
will propose it again — or, worse, build it.

Keep what the person actually wrote as well; do not shorten it away. Their
wording carries what they want, and yours carries what you agreed to do about
it. Neither replaces the other.

## When the work does not fit in one task

If it splits into pieces that could be done by different people on different
days, file them as separate tasks and say in each description how they relate.
One task that means four is a task nobody can pick up.

How you cut it also decides how much of it can be done at once. A run claims
several ready tasks and gives each its own worktree, so pieces filed separately
run in parallel by default, and pieces filed as one never do. The cut is worth
thinking about rather than something to get through.

**Stages are separated by a dependency, and a dependency is not a note to the
reader.** `bd ready` returns only what is unblocked, so the edge is the one
thing that physically keeps a later stage out of the same batch as the earlier
one:

```sh
bd dep <earlier-id> --blocks <later-id>
```

Use that form rather than `--deps` on `bd create`: `type:id` reads either way
round to a person, and a stage wired backwards is one picked up before the
thing it needs exists. `bd dep` spells the direction out in the flag.

**Within a stage, cut along file boundaries.** Two tasks that can be claimed
into the same batch must not edit the same file. They work in separate
worktrees cut from one target branch and are merged one at a time, so an
overlap is not a race that surfaces early — it is a merge conflict found after
both are finished and reviewed, and `merging` resolves conflicts by class,
which is no help whatever against two deliberate changes to one region.

**A piece earns its own task only if it can be merged alone**: without its
siblings, with its repository's gates passing and the app still building. A
piece that leaves the branch half-done until a sibling lands is not a task, it
is part of one — which is why the model and the code that uses it are one task
and not two, however neatly they divide on paper.

**A shared contract is not a seam.** Where two pieces can be separated only by
first agreeing a type, the shape of an event or the signature of a command,
leave them in one task. Do not file the contract as a third task to unblock
them: a worktree, a review pass and a merge cost more than the pair of
signatures they would carry. Parallelism inside a single task belongs to
`running-tasks`, and its Phase 1 already knows how to run layers that have
agreed a contract at the same time.

## If you cannot meet the bar

There are two honest outcomes, and filing a vague task is neither of them.

**You can still ask** — then ask. That is what the discussion is for. A question
now costs a minute; the same question during a run costs the night.

**You cannot ask, and you genuinely do not know** — then file what you do know,
say plainly in the criteria what is still undecided and who has to decide it,
and park it so that no run picks it up and stalls:

```sh
bd config set status.custom "ready_to_merge,parked,human_check"
bd update <id> --status parked
bd note <id> "parked: <the one thing that needs a person, concretely>"
```

The config line is idempotent and must always carry the full set — a partial
value clobbers the rest. That set is the repository's, not this skill's, so
`human_check` belongs in it here too even though nothing on this page ever writes
that status: naming only the ones you need deletes the others, and the damage
lands somewhere else entirely. `parked` does not exist as a status until it is
set, and `bd update` refuses an unknown one, so run it first rather than reading
the refusal as a bd problem. A parked task sits on the board where somebody will
see it; `bd ready` never returns it.

What must not happen is a section filled in to get past the validator. An
invented acceptance criterion is worse than a parked task, because nothing
downstream can tell it from a real one.

## Attached images

A task can arrive with images — a mock, a screenshot of the thing that is
wrong. The prompt names them by absolute path, and some harnesses are handed
the files themselves as well.

The description owes two things for each one, and one without the other is not
enough:

- **The path**, copied in exactly as it was given. Whoever picks the task up
  opens the picture by that string and has nowhere else to find it — bd carries
  no attachments of its own.
- **What matters in the picture, in words.** The files live in Smetana's own
  data directory on one machine; they are not in the repository, so in another
  clone the path leads nowhere and the words are all that is left. A described
  mock is not a mock, which is why the path is required too.

Say what the picture is for, not what it contains pixel by pixel: which screen,
what is wrong with it or what should change, and anything in it the text does
not already say.

## After filing

Report the id `bd create` printed. Do not start the work unless you were asked
to.
