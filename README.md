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

A column is a task status, and which of them exist is bd's business rather than this app's. The ones
the work is built around:

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

## Features

### Tasks

- **Tasks live in `.beads/`, inside the project's own repository**, and travel with the code — no
  server, no second copy of them anywhere. The order of the columns is a setting, per project,
  because one repository's custom status means nothing in another's.
- **Tasks are filed from the app**, and a screenshot goes onto one by dropping it, pasting it, or
  picking a file.
- **A task keeps its whole history**: its notes, the questions a run could not settle for itself —
  written in as `parked:` lines — and the answers you give, written back into the issue rather than
  into a chat nobody keeps.

### Runs

- **Three modes.** Solo is one task. Crew is one batch. Autopilot is a night of batches, one after
  another, until the queue in scope is empty or something needs a person.
- **Several runs at once.** Different scopes in one project — the queue beside a single epic — and
  several projects at the same time. The one thing refused is a second run over the *same* scope,
  where two leads would race for the same tasks.
- **Parallel sessions inside a batch.** The run's lead agent hands tasks to several sessions at once
  (up to eight, three by default), and each task gets its own git worktree in every repository it
  touches, so two of them cannot tread on each other's checkout.
- **A report for every run**: a self-contained HTML document under `.smetana/reports/`, saying what
  closed, what was parked and how long the whole thing took.

### Sessions

- **Two harnesses: Claude Code and Codex.** Everything the app asks an agent for is written once and
  translated per harness, so a third one is a profile rather than a rewrite.
- **Terminal tabs on real PTYs**, one per session, that you can read along in and type into at any
  moment.
- **The app sees when a session is waiting for you**, and says so where you will notice: the agent's
  row in the Agents list, the project's tile in the rail, the counter in the scope bar, and a sound —
  rather than leaving you to check every tab.
- **A conversation panel** that reads a session as messages instead of as a screen. Claude Code
  today; a Codex session is a terminal like every other.

### The window around them

- **The project's file tree, and a CodeMirror editor** with tabs, for looking at what came out.
- **A Git panel**: the working tree's status, merge, rebase and conflict resolution.
- **Notifications**: a bell with what the app has to say right now, a sound when a run ends, and the
  report itself put in front of you when it does.
- **Dark and light themes, comfortable and compact density, an app-wide font scale**, and the app
  updating itself in place.

## Requirements

- macOS on Apple silicon (arm64). There is no Intel, Windows or Linux build.
- [Claude Code](https://claude.com/claude-code) or [Codex](https://github.com/openai/codex),
  installed and signed in — the app drives a harness you already have, it is not a model client of
  its own.
- git.

bd ships inside the bundle (`bundle.externalBin`), so there is nothing to install for it, and an
agent that runs `bd` in a session started by the app reaches that same binary.

## Install

Download the `.dmg` from [Releases](https://github.com/invisor/smetana/releases) and drag Smetana to
Applications. It is signed with an Apple Developer ID and notarized, so it opens on a double-click
with nothing to dismiss and nothing to grant first.

Releases up to and including v0.1.23 were not signed, and there is one crossing to make from one of
those. A folder permission is stored against the exact build it was granted to, so updating an
unsigned copy to a signed one loses it — and macOS, holding a decision it thinks it has already made,
does not ask a second time. The app notices that it cannot read the folder and offers the repair
itself. From the first signed release on it stops happening: the permission is stored against the
certificate instead, which does not change between versions.

## Getting started

1. **Add a project.** Press `+` on the project rail down the left and pick the folder. A folder
   inside a tracked repository resolves to that repository's root. If it has no bd tracker yet, the
   app offers to run `bd init` in it.
2. **The board comes up** from `.beads/` in that repository, and follows it as it changes — whoever
   changed it, this window, an agent, or you in a terminal.
3. **Set the project up for runs.** The project tile's menu has "Set up", which starts an agent
   session that writes `.smetana/project.toml`: which repositories the project is, what branch
   work goes onto, the commands that bring it up, and the gates a task has to pass before it can
   merge. `.smetana/` is added to `.gitignore` for you.
4. **File a task** with `+` at the top of a column, attaching a screenshot if that says it faster.
5. **Choose what to run.** Press play on a card, on an epic, or on the queue as a whole, and pick the
   mode — Solo, Crew or Autopilot — the target branch, and how many tasks may go at once.
6. **Start it.** The run bar says which run is going and where it has got to; the Agent tab is where
   its sessions are, and you can read along or type into any of them.
7. **Read the report.** When the run ends its report opens in a tab, and it is on disk under
   `.smetana/reports/` for as long as you want it.

## How it works

- **The board is the bd tracker in the repository itself.** bd has no daemon and no API — its CLI is
  the API — so the app keeps a snapshot, watches `.beads/` and syncs the difference. Nothing is
  copied anywhere else, and a task filed by an agent in a terminal appears on the board a moment
  later.
- **A run is the app driving itself**: read the board, start an agent session on a batch of it, wait
  for that session to hand its work back, read the board again, decide whether to go round again.
- **Work happens in worktrees.** The run's lead agent cuts a git worktree per task, in each
  repository that task touches, and — unless you turn that off — removes it once the work has merged.
- **Sessions are real PTYs**, spawned by the app, with the bd sidecar's directory on the front of the
  `PATH` they inherit. That is why the app can tell an agent is waiting for a human: it reads the
  same screen you would.
- **State is files.** Tasks in `.beads/`; the project's run configuration, what a live run has taken,
  and every report in `.smetana/`. The app never falls asleep while a run is going, and picks up
  after an unclean exit by reading those files back.

## Status and limits

This is an early version, and honest about it:

- **macOS on Apple silicon only.** Windows and Linux builds are wanted and neither has been checked
  by eye yet; there is no Intel build.
- The app drives a harness rather than a model, so what a run can finish unattended is whatever
  Claude Code or Codex can finish unattended.
- **Runs have been driven far harder on Claude Code than on Codex**, and the conversation panel is
  Claude Code's only — on Codex a session is watched as a terminal.

## Development

```sh
npm install          # postinstall fetches the bd sidecar; it warns and continues without one
npm run dev          # http://localhost:5173 — the front end alone, backed by a mock of the back end
npm run build
npm test             # front-end tests (vitest), single run
npm run tauri dev    # the actual desktop app: Rust worker, real bd, live board
cd src-tauri && cargo test
```

`npm run dev` needs no Rust toolchain and no bd: `src/stores/mockBackend.js` answers the read
commands with fixtures, and writes to the tracker reject loudly rather than pretending to work. The
front end reads three query parameters:

| parameter | values | default |
|---|---|---|
| `theme` | `dark`, `light` | `dark` |
| `density` | `comfortable`, `compact` | `comfortable` |
| `view` | `gallery`, `settings` | the app |

`?view=gallery` renders every exported component once — the harness for catching a broken component,
code-split and never in the app bundle. `?view=settings` is the settings window, which in the desktop
app is a second OS window loading that same query string.

The front end is a port of the Smetana Design System's React sources, and its rules are not
negotiable per component: read [`CLAUDE.md`](CLAUDE.md) before changing anything under `src/`, and
[`AGENTS.md`](AGENTS.md) for how work is tracked here. Prose about one subsystem lives in
`.claude/rules/`, next to the code it is about. Cutting a release is [`RELEASING.md`](RELEASING.md).
