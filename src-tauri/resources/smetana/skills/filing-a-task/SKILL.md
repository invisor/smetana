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

What Smetana hands you is the person's own words about what needs doing — one
piece of prose, not a title and a description. Both are yours to write from it:
you have read it and the app has not.

The title says what needs doing, not what is wrong: a reader scanning the
board sees titles and nothing else. The description carries what the title had
to leave out — where in the tree, what the current behaviour is, what would
count as done. Keep what the person actually wrote; do not shorten it away.

When the prompt tells you to decide the type or the priority yourself, decide
from the same text, using the values listed above. `task` and 2 are the right
answer whenever nothing in the text argues for something else.

If the work splits into pieces that could be done by different people on
different days, file them as separate tasks and say in each description how
they relate. One task that means four is a task nobody can pick up.

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
