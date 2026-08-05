---
name: project-setup
description: Use when Smetana asks you to set a project up for runs — writing .smetana/project.toml from what the folder actually contains
---

# Setting a project up for runs

Smetana runs tracker work by starting agent sessions in git worktrees, merging
what they produce into one branch, and closing the task. Before it can do any
of that it has to know four things about this project, and only you can find
them out: which repositories it is made of, what "green" means in each of them,
how the project comes up, and how a finished piece of work is verified.

You write that into `.smetana/project.toml`, beside `.beads/`. The prompt you
were given already carries a scan of the folder — repositories, manifests and
the commands those manifests suggest. **Those commands are candidates, not
findings. Nothing ran them.**

**Do not add `.smetana` to any `.gitignore`, whatever the neighbouring entries
suggest.** The file belongs in the repository. Everything in it was learned by
running commands and reading output, and it is the same for everyone working
here; ignored, it is written again from guesses in every fresh clone and in
every worktree a run provisions. In a folder holding several repositories the
root is usually not under git at all, and then there is simply nothing to do.

## What the file holds

    [project]
    repos = ["backend", "frontend"]   # relative paths; a single repository is ["."]

    [defaults]
    target_branch = "staging"          # what the run dialog offers first
    min_priority = 2                   # tasks below this are not taken automatically
    max_parallel_tasks = 3
    review_passes = 5

    [repo.backend]
    setup = "npm install"              # run once in a fresh worktree
    gates = ["npm run typecheck", "npm test"]
    env_files = [".env"]               # copied in from the main checkout

    [preflight]
    commands = ["docker compose up -d"]
    health = [{ url = "http://localhost:4001/health" }, { tcp = 5433 }]

    [merge]
    # Declared before the array of tables below: in TOML a bare key written
    # after [[merge.regenerate]] would belong to that entry, not to [merge],
    # and the file would then be refused with an "unknown field" error naming
    # a key that looks nothing like a typo.
    hazards = """
    Prose. See below.
    """

    [[merge.regenerate]]
    paths = ["admin/src/api-types.ts"]
    command = "npm run generate:api-types"

    [live_check]
    mode = "browser"                   # browser | command | none

Only `[project]` is required. Every other section may be left out, and leaving
one out is better than filling it with a guess.

## How to fill it in

**If `.smetana/project.toml` already exists, read it and update it rather than
replacing it.** Someone may be re-running this after the project changed, and
whatever it already got right — a gate that was verified, prose in `hazards`
someone wrote by hand — should survive a second pass, not be guessed again.

**Verify every gate before you write it.** Run it. A gate that does not exist,
or that is red on a clean checkout, is worse than no gate: every merge will
either fail for a reason nobody caused or pass having proved nothing. If a
command is red on the untouched project, say so to the person rather than
writing it in.

**Order `repos` by what depends on what.** Whatever produces an API contract
merges before whatever consumes it. In a single-repository project this is
`["."]` and there is nothing to order.

**`hazards` is for what a rule cannot express.** It is read by the agent that
merges, after every merge, and it is where you record the things git does not
flag: two branches emitting a migration with the same number off one base,
generated files that must be regenerated rather than merged, a lockfile whose
clean merge installs something different. Write it as instructions to someone
who has just arrived. Leave it out if the project genuinely has none.

**`regenerate` is for what a rule can express**: a path that is never merged by
hand, and the command that reproduces it.

**`live_check`** is how a merged task is verified beyond its tests.
`mode = "browser"` when there is a UI a person would click through — then use
`notes` to say how the stand comes up and how to sign in. `mode = "command"`
with a `command` when there is an end-to-end suite instead. `mode = "none"`
when there is neither; the toggle in Smetana then says so rather than
pretending.

## Ask about what the folder cannot answer

The scan cannot tell you which branch work should merge into, whether a red
command is expected, or what breaks quietly in this codebase. Ask — one
question at a time. This file is read by every run from now on, and a guess in
it is a guess repeated nightly.

## When you are done

Write the file, then show the person what you wrote and what you could not
determine. Do not start any work on the tracker: setting the project up is the
whole task.
