---
name: starting-a-project
description: Use when Smetana starts you in an empty folder — agreeing what the project is before creating anything, laying a minimal foundation, committing it, and then setting the project up for runs
---

# Starting a project in an empty folder

Smetana checked that this folder holds nothing but housekeeping, and it has
already run `bd init` here, so the board is live. What it cannot do is decide
what the project is. That is the person's, and it is the first thing you do.

## Talk first, files second

Nothing is created before the person has agreed to it. Ask one question at a
time, and stop after each until it is answered:

1. **What is this project?** One sentence, in their words. Repeat it back.
2. **Which stack?** Language, framework, toolchain. If they do not know,
   offer two or three fitting the sentence above and say why.
3. **What is it called?** The name that goes in the manifest.

Then say, in one short list, exactly which files you are about to create and
what the first command that has to pass will be. Wait for a yes.

## The foundation

Minimal. A manifest, a README with the sentence from question 1, a
`.gitignore` for the stack, and **one command that passes** — a single test
or a build — so that a gate has something real to check. A generator that
downloads templates or installs packages is something to ask about, never to
run on your own: the same rule `project-setup` holds for Playwright. Do not
scaffold features, pages or modules nobody has asked for.

If there is no repository, `git init`. Make the first commit on the branch
git created. Run the one command before you commit, and put its output in
front of the person.

You file no tasks and write nothing under `.smetana/` except what the next
step writes.

## Then set the project up

Use the `project-setup` skill exactly as it stands. You have an advantage
its usual reader does not: you chose the stack and you have just run the
command that becomes the gate, so write that gate and nothing you have not
run. `repos` is `["."]`. Leave every section out that the folder cannot
answer yet, and say which ones you left out.
