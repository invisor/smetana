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

## 8. The report: Markdown for reading, JSON for the page

The prompt gives you a path with no extension. Write two files:

- `<report>.md` — the review in Markdown, for a person who reads it as text
- `<report>.json` — the same review as data, in the shape below

Same content in both: the same findings, the same score, the same pairs. The Markdown is
yours to lay out; the JSON is not, and **you do not write a page.** Smetana draws the
review from the JSON when you finish, in one design for every review, so there is no HTML
to produce, no stylesheet to write and nothing to escape. Write plain text in every
string and let the app do the rest.

The JSON file holds that object and nothing else: no comment, no code fence, no text
before or after it. It is parsed strictly enough that a missing required field gives a
page saying the report could not be built, and loosely enough that a field the schema does
not know is ignored.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "smetana/code-review-report.schema.json",
  "title": "Code review report input",
  "type": "object",
  "required": ["score", "pairs", "findings", "reviewer", "date"],
  "additionalProperties": false,
  "properties": {
    "score": { "type": "integer", "minimum": 0, "maximum": 100, "description": "Readiness, percent. Range is derived: >=90 high, 70–89 mid, <70 low." },
    "pairs": {
      "type": "array", "minItems": 1,
      "items": {
        "type": "object",
        "required": ["repo", "base", "head", "files", "added", "removed", "hazards"],
        "additionalProperties": false,
        "properties": {
          "repo": { "type": "string" },
          "base": { "type": "string", "examples": ["main"] },
          "head": { "type": "string", "examples": ["feature/smetana-6tev-worktree-gc-skip-live-agents"] },
          "files": { "type": "integer", "minimum": 0 },
          "added": { "type": "integer", "minimum": 0 },
          "removed": { "type": "integer", "minimum": 0 },
          "hazards": { "type": "boolean", "description": "Reviewer had [merge].hazards project rules for this repo." },
          "rulesSource": { "type": "string", "description": "Optional path the rules were loaded from, e.g. .smetana/review.toml" },
          "reviewedFiles": {
            "type": "array",
            "description": "Used only by the zero-findings state.",
            "items": {
              "type": "object", "required": ["path", "added", "removed"], "additionalProperties": false,
              "properties": { "path": { "type": "string" }, "added": { "type": "integer" }, "removed": { "type": "integer" } }
            }
          }
        }
      }
    },
    "findings": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["level", "title", "repo", "path", "line", "defect", "scenario", "fix"],
        "additionalProperties": false,
        "properties": {
          "level": { "enum": ["blocking", "serious", "minor"] },
          "title": { "type": "string", "description": "Short, sentence case, no trailing period." },
          "repo": { "type": "string", "description": "Must match a pairs[].repo." },
          "path": { "type": "string" },
          "line": { "type": "integer", "minimum": 1 },
          "defect": { "type": "string", "description": "One sentence. Inline `code` allowed." },
          "scenario": { "type": "string", "description": "input → result. Inline `code` allowed; rendered with .io for literals." },
          "fix": { "type": "string" },
          "code": {
            "type": "object",
            "required": ["startLine", "lines"],
            "additionalProperties": false,
            "properties": {
              "path": { "type": "string", "description": "Caption; defaults to finding.path." },
              "language": { "type": "string", "examples": ["rust", "ts"] },
              "startLine": { "type": "integer", "minimum": 1 },
              "lines": { "type": "array", "items": { "type": "string" }, "maxItems": 12 },
              "highlight": { "type": "array", "items": { "type": "integer" }, "description": "Absolute line numbers to mark with .hl" }
            }
          }
        }
      }
    },
    "reviewer": { "type": "string", "examples": ["claude-2"] },
    "date": { "type": "string", "description": "Local time, rendered as YYYY-MM-DD HH:mm." }
  }
}
```

How to fill it:

- **`score`** is the readiness percentage from §7, an integer. The page derives the range
  from it (90 and up, 70 to 89, below 70) and draws the scale, so do not describe the
  scale in a field. A score that contradicts the findings — 70 or more with a blocking
  finding — is drawn as written, but it tells the reader you did not mean one of the two.
- **`pairs`** has one entry per pair you were given, with the refs exactly as given. `files`,
  `added` and `removed` come from `--stat`. `hazards` is whether you had `[merge].hazards` to
  work from (§2), and `rulesSource` names the file you read them from. `reviewedFiles`
  lists every changed file with its line counts, and is drawn only when there are no
  findings; leave it out otherwise.
- **`findings`** is the list from §6, one object per finding, with `level` one of
  `blocking`, `serious`, `minor` as defined in §7. `repo` is a `pairs[].repo` and `path`
  is relative to that repository. `title` is short, in sentence case, with no trailing
  period. `defect` is one sentence. `scenario` is the failure from §6 written as
  *input → result*. `fix` says what to change. An empty list is a clean review, and it is
  drawn as one.
- **`code`** is the quote from §6: up to 12 lines, `startLine` the number of the first,
  `highlight` the absolute number of the line the finding is about. Give lines as they are
  in the file, without line numbers and without escaping anything.
- **Backticks** are the one piece of formatting there is. Put an identifier, a path, a
  literal or a snippet between a pair of backticks in `title`, `defect`, `scenario` and
  `fix` and it is drawn as code. There is no Markdown beyond that: no bold, no lists, no
  links.
- **`reviewer`** is your own name or model. **`date`** is the local time as
  `YYYY-MM-DD HH:mm`.

Say in the Markdown which pairs you reviewed, ref for ref, and whether you had
`[merge].hazards` to work from; the JSON's `pairs` carries the same two facts for the page.

## 9. You never file what you find

You will find real defects. Reporting them is the job; **turning them into work is not
yours to do.** Do not create a tracker issue, do not run any command that writes to a
tracker, and do not ask for one to be created — the same rule the `reviewing` skill holds,
and for the same reason: a reviewer who files their own findings is a queue that feeds
itself, and nobody downstream can tell the noise from the work.

The report is where a finding goes. Whoever asked for the review decides what happens to
it.
