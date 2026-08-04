---
name: filing-a-task
description: Use when filing a task into this project's bd tracker from Smetana — covers which fields bd wants and how to word them
---

# Filing a task in bd

The board this app shows is the `bd` tracker in the project directory. A task
is filed with `bd create`, and the board picks it up on its own — there is
nothing to refresh and nothing else to notify.

## The command

```sh
bd create --title "<title>" --type <type> --priority <0-4> --description "<text>"
```

`--title` rather than the positional form bd's help shows first. bd checks a
positional title for a leading dash and refuses to create the issue, even after
`--` — a title like `-n 5 is not enough` comes back as `looks like a flag`. A
flag's value goes through no such check and is taken as written.

`--type` is one of `task`, `bug`, `feature`, `chore`, `epic`, `decision`.
`task` is the default and the right answer when nothing else fits.
`--priority` is 0 (highest) to 4 (lowest); 2 is ordinary.

## Wording

The title says what needs doing, not what is wrong: a reader scanning the
board sees titles and nothing else. The description carries what the title had
to leave out — where in the tree, what the current behaviour is, what would
count as done.

If the work splits into pieces that could be done by different people on
different days, file them as separate tasks and say in each description how
they relate. One task that means four is a task nobody can pick up.

## After filing

Report the id `bd create` printed. Do not start the work unless you were asked
to.
