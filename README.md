<h1 align="center">
  <img src="assets/icon.png" alt="" height="64" align="absmiddle">&nbsp;Smetana
</h1>

<p align="center">
  <a href="https://github.com/invisor/smetana/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/invisor/smetana"></a>
  <img alt="macOS on Apple silicon" src="https://img.shields.io/badge/macOS-Apple%20silicon-111?logo=apple&logoColor=white">
  <a href="https://claude.com/claude-code"><img alt="Works with Claude Code" src="https://img.shields.io/badge/agent-Claude%20Code-d97757"></a>
  <a href="https://github.com/openai/codex"><img alt="Works with Codex" src="https://img.shields.io/badge/agent-Codex-10a37f"></a>
</p>

<p align="center">
  <b>English</b> ·
  <a href="README.ru.md">Русский</a> ·
  <a href="README.zh.md">中文</a>
</p>

<p align="center">
  <b>Autonomous coding. File the tasks, start the run, go for coffee.</b><br>
  Describe what you want and an agent files it as tasks, asks you whatever it could not settle for
  itself, and sets the blockers and dependencies between them. You press play once; the run works
  through the queue on its own. What is left for you is reading the result.
</p>

<h3 align="center"><a href="https://github.com/invisor/smetana/releases/latest">Download Smetana</a></h3>

![The board, a project's agent sessions on the left and the selected task on the right](assets/screenshot-board.png)

## What it is

Smetana is an ADE — an agentic development environment. Not an editor with a chat bolted onto it: a
place where you say what the product should do and agents do the work. Two things ship inside the
app. The [bd](https://github.com/gastownhall/beads) kanban tracker, which keeps tasks in your own
repository rather than on somebody's server. And the
[Superpowers](https://github.com/obra/superpowers) skill library, which is where an agent gets its
method — how to interrogate a task, plan it, review a change, merge it.

### How a piece of work goes

1. **You describe it in your own words.** A paragraph, a screenshot; no ticket discipline required.
2. **The agent asks.** It works through everything ambiguous with you, until nothing load-bearing is
   left for it to guess.
3. **The agent files the task** — one issue, or several with the dependencies drawn between them —
   and puts it in Ready.
4. **You start a run**, naming the git branch the result is to land on.
5. **The run takes a batch off Ready** and carries the tasks in parallel where they allow it, each in
   its own worktree, so no two tread on the same checkout.
6. **It merges what passed** into that branch, conflicts and all.
7. **It picks up the next batch**, and the next, until Ready is empty.
8. **You bring the project up and look at what came out.**

The app tries to keep a person out of the loop wherever that is honest. Filing a task, editing one,
resolving a merge conflict, deciding a branch is fit to merge — that is the agent's own work, and you
are asked only where the thing cannot be settled without you: a question written into the task's
notes, or a run that stops and says which task it stopped on. Everything it knows sits in files you
can open — tasks in `.beads/`, run state and reports in `.smetana/`. No server, no account, no
database.

## The Kanban board

A column is a task status, and the board draws every status the project has. These are the ones the
work is built around:

- **Ready** — ready to start: the questions are answered, the implementation plan is written where
  one was needed, and nothing unfinished is holding it up. A run takes its batch from here.
- **Running** — being worked on right now: an agent has claimed it and is on it.
- **Blocked** — two kinds of card land here. One is waiting on another task — something it depends on
  is not finished — and moves to Ready by itself the moment that one closes. The other carries a lock
  badge: a person locked it by hand, and only that same person, unlocking it, moves it back.
- **Parked** — work throws up things nobody could have foreseen while the task was being written, and
  some of them need a person. An agent that meets one mid-run parks the task: the question goes into
  the task's notes, and **Answer questions** starts an agent that puts it to you and returns the task
  to Ready. The run itself keeps going — that batch ends, and the next one is taken from Ready
  without anything that depended on the parked task. Only the same question coming back a second time
  ends the run.
- **Ready to merge** — done and reviewed, waiting to land on the target branch. It closes when it
  lands.
- **Human check** — done and merged, waiting for somebody to look at it by hand. A run leaves one
  behind when it could not check the work itself: you look, then close it or send it back to Ready.
- **Done** — finished and closed.
- **Deferred** — put off on purpose, with nothing holding it up. Findings that turn up while another
  task is being worked on land here: an agent that trips over a bug files it into Deferred rather than
  into Ready, and that is deliberate — a run that picked up its own findings would never reach the end
  of the queue. Only a person moves one back to Ready.
- **Pinned** — never taken into work. A backlog, in other words: tasks to be done some day, but not
  now.
- **Hooked** — an agent has taken a whole group of related tasks at once. It says whose work it is,
  not how far it has got, and a run leaves these alone.

A project can add columns of its own. Any status bd carries becomes one, with a colour and a
two-letter code the app picks for it.

## Runs

A run is a process inside the app driving the work itself: it reads the board, hands a batch of tasks
to agent sessions, waits for them, merges what passed and reads the board again. Every batch gets a
session of its own, so the context starts clean each time round. It watches the subscription as well
— when the allowance runs out the run pauses where it stands, and picks up again once it is back.

You start a run on a single task, on an epic, or on the whole Ready queue. Then you say where the
result is to land — the target branch — how many tasks may go at once (one to eight, three by
default), and, for the queue, a priority floor below which nothing is taken (P2 and better by
default).

**Solo** — one task, and the agent does the work itself rather than handing it to anyone. It asks you
freely as it goes and waits for the answer before carrying on. Offered only where you pointed at a
task: for an epic or the queue there is nothing to be solo about. The mode for keeping the most
control.

**Crew** — a lead agent working with teammate sessions. A teammate's question is the lead's to
answer, and only what the lead cannot settle itself reaches you; the task is not parked while you
think about it — the session waits for you. A Crew run takes exactly one batch, merges it, and that
is the end of it, which is what makes it the mode for staying close to the work.

**Autopilot** — coding with nobody in the room. The run takes a batch off Ready, carries it through
to a merge, takes the next one, and keeps going until the queue in its scope is empty or something
needs a person. Whatever needs a person is parked, with the question in the task's notes, and the run
carries on with everything that does not depend on it. The sessions here are unattended: they do the
work and exit.

In Crew and Solo the session outlives the run — it sits at its prompt and you can go on talking to
it, and the run ending does not close it. In Autopilot it does not, that mode being built for a room
with nobody in it.

**Several runs can go at once**: over different scopes in one project — the queue beside a single
epic — and over several projects at the same time. Exactly one thing is refused, a second run over
the *same* scope, where two leads would be fighting over the same tasks.

Two switches ride along with all of this. **Check each task for real before closing it** sends the
agent to bring the project up and exercise the work rather than trusting its own tests. **File what
it finds along the way** lets it write down the bugs it trips over — into Deferred, never into Ready.

<p align="center">
  <img src="assets/screenshot-run.png" alt="The run dialog: target branch, mode, how many at once, priority floor" width="420">
</p>

However a run ends, it writes a report — a self-contained HTML document under `.smetana/reports/`
saying what closed, what was parked, how long the whole thing took, and which mode did it.

## Requirements

- **macOS on Apple silicon (arm64)** — the only build released so far, and the only platform the app
  has been used on. Windows and Linux are wanted and planned: Tauri builds for both, the release
  workflow has a row waiting for each, and neither has been checked by eye yet.
- **A harness CLI, installed and signed in** — [Claude Code](https://claude.com/claude-code) or
  [Codex](https://github.com/openai/codex). The app drives the command-line tool you already have;
  it is not a model client of its own, and it holds no keys. Both are supported, but nearly all of
  the testing so far has been on Claude Code.
- **git.**

## Install

Download the `.dmg` from [Releases](https://github.com/invisor/smetana/releases) and drag Smetana to
Applications. It is signed with an Apple Developer ID and notarized, so it opens on a double-click:
nothing to dismiss, nothing to grant first.

## Getting started

1. **Add a project.** Press `+` on the project rail down the left and pick the folder. A folder
   inside a tracked repository resolves to that repository's root, and if there is no bd tracker in
   it yet the app offers to run `bd init` for you.
2. **The board comes up** from `.beads/` in that repository, and follows it from then on — whoever
   changes it: this window, an agent, or you in a terminal.
3. **Set the project up for runs.** The project tile's menu has **Set up**, which starts an agent
   session that asks about the project and writes `.smetana/project.toml`: which repositories the
   project is made of, what branch work goes onto, the commands that bring it up, and the gates a
   task has to pass before it may merge. `.smetana/` is added to `.gitignore` for you, so none of it
   is committed.
4. **File a task** with `+` at the top of a column. Write what you want in your own words; the agent
   asks about the rest.
5. **Press play** — on a card, on an epic, or on the queue — and pick the mode, the target branch and
   how many tasks may go at once.

## Getting involved

The app is early, and it is being built in the open because that is the only way the rough edges get
found. **Something broken? [Open an issue](https://github.com/invisor/smetana/issues)** — say what
you were trying to do when it went wrong. **An idea, a wish, a question about how this is meant to
be used? [Start a discussion](https://github.com/invisor/smetana/discussions).** The road map is
short on purpose, and what people actually ask for moves up it.

**Help with the code is welcome as well.** Four things would help more than anything else right now:

- **Checking the Windows and Linux builds.** The release workflow has a row waiting for each
  platform; nobody has run the app on either yet.
- **A night on Codex.** Both harnesses are supported, and almost all the testing so far has been on
  Claude Code.
- **Agent profiles for other harnesses.** Everything the app asks an agent for is written once and
  translated per harness, so a third one is a profile rather than a rewrite.
- **Anything that broke for you**, with the run report from `.smetana/reports/` attached if a run was
  involved — it says what the app thought was happening.

Before changing anything under `src/`, read [`CLAUDE.md`](CLAUDE.md): the front end is a port of a
design system with rules that are not negotiable per component. [`AGENTS.md`](AGENTS.md) is the same
thing for agents working in this repository.

## License

[MIT](LICENSE).
