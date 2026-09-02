---
paths:
  - "src-tauri/src/agents/**"
  - "src-tauri/resources/**"
  - "src/components/kanban/NewTaskModal.vue"
  - "src/components/kanban/taskStages.js"
---

# The agents: one intent, two harnesses

`src-tauri/src/agents/` is what the app knows about the CLI coding agents it runs, one file per
agent, and everything harness-specific lives in it. Claude Code and Codex are supported; which one
runs is the `agent` field in `settings.json`.

The split that makes this a module rather than a `match` in the terminal worker: **what the app wants
done is the same for every agent, and how it reaches one is not.** An `Intent` — `Bare` from the
"+ New agent" row, `NewTask` from the new-task dialog, `EditTask` from a card's "Edit", `ResolveTask`
from a parked card's "Answer questions", `FixTask` from a done card's "Fix this",
`RepairTracker` from the second button under a failing
board, `Setup` from the dialog a person gets when they add a project,
`ResumeSession` from either of the two launching rows of a card in the Sessions tab, and `Run` for
one batch of a run — is where the product decision lives, written once. `FixTask` is deliberately not
an `EditTask` pointed at something else: an edit changes an issue's own prose and is the only way
this app has of changing it, while a fix changes the code behind a task that is closed and merged and
turned out not
to be finished. That makes it the one intent about a single issue in **both** `writes_to_the_tracker`
and `commits_to_git` — it leaves a note saying what was put right, carrying no `parked:`/`resolved:`
marker since it is neither an open question nor an answered one, and it commits the correction. The
issue stays closed: a change worth reopening for is worth a task, which is the Follow-up task row
directly under it in the same menu. That menu is also where the difference is visible — a done card
draws neither the play (`runnableTask` refuses a closed issue anyway, so the row was greyed for ever)
nor the edit. `Run`
is the only one no person sends: `runs::service` builds it, carrying the whole of what the run was
asked to do rather than a reference to it, because a session outlives a settings change and a batch
that quietly retargets halfway through is worse than one wrong from the start that says so.
`SkillDelivery` is how a skill library reaches a particular harness, and there is no uniform answer:
Claude Code takes `--plugin-dir` and loads a plugin for one session, installing nothing
(`PluginDir`); Codex has no per-session mechanism at all — its skills system reads `~/.codex/skills/`
and the only way to add a root is a JSON-RPC method on the app-server, a different process from the
TUI this app spawns — so its skills ride as text in the prompt (`Inline`), since writing into
someone's home directory or repointing `CODEX_HOME` would reach into their own setup. Nothing about
either harness leaks into the code deciding what we want done: `prompt.rs` takes an `Intent` and a
`SkillDelivery` and is pure, which is where the tests are.

**Every prompt is a whole instruction, and a test pins it.** A prompt rides as the agent's positional
argument, and both harnesses submit that argument as the session's first message rather than leaving
it in the composer — so there is no such thing as a prompt somebody finishes by hand. `EditTask`'s
stopped mid-sentence at a colon on the theory that the person would type the second half; they never
got the chance, and the agent's first move was to ask whether the message had been truncated. It is
finished now by **asking** rather than by guessing, since an agent that decides for itself rewrites
an issue nobody asked it to touch. `no_prompt_stops_mid_sentence` walks every intent and both
deliveries and refuses a prompt ending in dangling punctuation.

| file | what it does |
|---|---|
| `mod.rs` | `Profile`, `Intent`, `Stage`, `SkillDelivery`, `ImageDelivery`, `TaskDraft`, `Autonomy`, `Launch` — the vocabulary, the registry, `cascade` and `IDS` |
| `library.rs` | where the bundled skills are, whether the person already has their own superpowers, and reading a `SKILL.md` for inlining |
| `prompt.rs` | an intent becomes the text the agent opens on — pure; the skill text, where one is needed, is read by the caller and passed in |
| `claude.rs` | Claude Code: `--plugin-dir`, and layer B, its permission dialog read off the screen |
| `codex.rs` | Codex: `Inline`, `-i` for images, and its own layer B — the approval dialog read off a screen with no frame anywhere on it (smetana-603) |

**Codex's layer B is genuinely a different reader, not Claude's with the glyphs swapped**, and the
two deliberately share no code, because a glyph one harness happens to use today is exactly what
drifts. Its rules are measured off fixtures in `src-tauri/tests/fixtures/`, captured under a PTY at
60 and 120 columns from CLI 0.146.0, and `codex.rs` carries each rule with the screen that forced it.
Three properties of that interface are why it cannot be shared: the cursor `›` (U+203A) is also drawn
in front of the person's own submitted prompt and the empty composer, so it counts only as the first
non-blank character of a line; there is no frame anywhere, so **the only structural boundary is
indentation**, with two blank rows between top-level blocks against one between paragraphs inside
them; and a block is **refused for what it hangs off** — a conversational turn, `•`, `◦` or `›` — not
for how closely it sits, which is what survives a turn wrapping over several rows in a narrow pane.

One known gap is recorded rather than papered over: **a scrolled screen with no anchor left on it**,
where the walk upward reaches row 0 having met nothing and indented prose above a numbered draft
still reads as a dialog. A test pins it by name. Closing it would mean requiring every block to be
anchored, which would refuse Codex's update prompt — a real dialog drawn from row 0 — so it is a
false match in a rare scroll position against a miss in an ordinary one, with no measurement to
settle it. Every rule here fails closed for the reason the design budgets loudness: a session wrongly
turned `needs-you` spends one of the one or two loud rows on the screen and makes
`terminal_run_capture` refuse a session with nothing open on it, so a change to that CLI should cost
a miss rather than a false alarm.

The rest of `Profile` is the same split one level down, and each method's **default is a
working answer rather than a gap** — the shape to keep when the next one is added. `images` says how
pixels reach a harness: Codex takes `-i/--image`, Claude Code simply opens a path the prompt names,
so the default is `InPrompt`, the one channel every CLI has. `usage_command` and `parse_usage` are a
pair, and a profile answering one without the other reads as unaskable, which the run gate treats as
no reason to hold anything up. `autonomy` is the extra arguments and environment for working with
nobody watching; the default is nothing, so a harness with no such switch stops at its first
permission prompt and turns `needs-you` — exactly what `Supervised` already is, which is the app
saying a harness cannot be autonomous by behaving like it rather than pretending otherwise.
`batch_args` and `transcript` are the last pair and hang off one predicate, `agents::is_batch`: an
interactive session finishes its work and sits at its prompt, so a loop waiting on the process would
never come round at all, and the non-interactive form that fixes that is also the one printing a
machine format nobody reads. That is the unattended answer, and it is only half of the question —
the attended modes keep the interactive session on purpose, and what ends a batch there is the
account it writes rather than the process it never leaves (see `handed_back` in `.claude/rules/runs.md`). So the first says how this
harness is told to carry one batch out and **exit**, in front of everything else on the line, and
the second says how a line of what it then prints becomes a line in the pane. Their defaults are
nothing and no translator, working answers again: a harness given neither runs exactly as every
harness ran before they existed — which is Codex today, deliberately and with its own task behind
it.

`oneshot_args` is the only one with no session behind it at all: how this harness is
asked **one question** and nothing more. Claude Code answers it with the same `-p` `batch_args`
opens with, and the two are still different questions — that one is "carry this batch out and exit"
and comes with a stream format and a translator because somebody watches a batch work, this one is
"answer this and exit" and wants the answer on stdout with nothing around it. The caller is
`agents::oneshot`, whose one user today is the commit-message button in the Git panel
(`.claude/rules/vcs-panel.md`), and it is the same spawn `runs/usage.rs` makes — `std::process`, no
PTY, the login shell's `PATH`, a deadline with a kill behind it — with one difference that decides
its whole error type: an unreadable allowance is no reason to hold a run up, so `usage::read`
answers `None` for every way of failing, while here somebody pressed a button and is watching a
field, so each way of failing keeps its own name and reaches the panel as a sentence. The default is
`None` again, and the panel draws the button for everybody rather than hiding it: a harness that
cannot be asked says so, which the front end could not decide for itself anyway, since it never
learns an agent's name.

`resume_args` and `fork_args` are the newest of them and the pair a whole feature hangs off: how
this harness is told to pick a **recorded session** up again by its id, and how it is told to carry
one on in a *new* session instead, as the arguments in front of everything else on its command line.
Claude Code answers `--resume <id>` and `--resume <id> --fork-session`; Codex keeps both defaults of
`None`, and that is a decision rather than a gap — its argument grammar is its own and this app does
not get to guess it, which is the same rule `claude.rs`'s `command` already follows about argument
*order*. The capability and the arguments are one answer for `usage_command`'s reason: a profile that
said "yes" without saying how would leave the caller inventing somebody else's command line.

**Two methods and not one flag appended to the other's answer**, and that is the same rule one level
down: reopening a transcript and branching it are two capabilities, a harness that grows the first
without the second is an ordinary shape, and a caller that appended `--fork-session` to whatever
`resume_args` said would be composing somebody else's command line out of halves. `claude.rs` writes
the forked line out whole for exactly that reason, repeating `--resume <id>` rather than borrowing
it.

What the defaults cost is nothing and refuse everything: `terminal::service` asks before it spawns
and answers `TerminalError::NoResume` or `NoFork` — its own variant apiece, because a sentence saying
a harness cannot resume would be untrue about the row nobody pressed — since the alternative,
starting the agent anyway, is a *fresh* session in a worktree under a card promising the conversation
somebody left. The front end greys the rows before that, from `RESUMES_BY_ID` and `FORKS_BY_ID` in
`components/agent/sessionMenu.js`, which are the second copy of this fact and say so; nothing
mechanical joins the two sides, and both directions of drift are quiet, which is why those lists may
be wrong there and never here.

`Intent::ResumeSession` carries `fork`, which is the whole difference between the Sessions tab's two
launching verbs and nothing else about it: the directory, the id and the row it draws are one path.
`Intent::work` reads the flag and drops it, so a resumed session and a forked one are the same row —
what a person picks a session out of that list for is the conversation, not which file it goes on
being written into.

It is also the one intent that opens on **no prompt at all**, and `prompt::build`
refuses it before it composes a word. A prompt rides as the positional argument and both harnesses
*submit* it as the session's first message; a resumed conversation already has somebody's words in
it, so even the conversation-language paragraph — which reaches every other intent, `Bare`
included — would be this app talking over the person whose session it is. Whatever was settled in
there was settled before this window existed.

`agents::IDS` is the single copy of the agent-id list, and `settings/model.rs` validates against it
rather than repeating it — the side-tab hazard again: a value that survives the session and silently
comes back as something else. The front end never learns the names either: `settings.js` holds
whatever string is in the file and passes it to `terminal_create`, and Rust resolves it. A configured
agent that is not on `PATH` falls back to the first one that is, and `Session.agent` carries what
actually started; nothing on screen reads it, so the substitution is silent and the terminal is the
only way to see it. When nothing at all is installed the session fails with `NoAgent`.

`agents::LANGUAGES` is the same idea one field over: the twelve languages a person may choose, as
BCP-47 ids **with the English name of each**, and the only copy of that list — `settings/model.rs`
validates `agentLanguage`, `taskLanguage`, `commitLanguage` and `reportLanguage` against it exactly
as it validates `agent` against
`IDS`. The name is carried beside the id because the name is what goes into the prompt: `zh-Hans` is
a tag out of a settings file, "Chinese (Simplified)" is a sentence. Every one of them defaults to
`en` rather than to an Auto position, which would have meant "say nothing about language" — today's behaviour exactly,
so an update changes nothing until somebody chooses — and for the commit language that default is
today's behaviour to the letter, since `oneshot::commit_prompt` asked for a message "in English"
outright before the setting existed. The price is deliberate: `Intent::Bare` no
longer opens on nothing, since it carries the one sentence naming the conversation language, and the
alternative was that the session where a person talks to the agent most is the one the setting cannot
reach.

None of them crosses the IPC. `settings::languages(app)` reads the file where
`settings::agent(app)` already does, and `terminal::service`'s `Create` arm calls it while building
the `Launch` — the one place every session in the app is built, so a person's session and a run's
batch get the same answer by construction. From the `Launch` the ids reach `prompt::build`,
which stays pure. The commit language has one reader outside a session, and it reads the same field
by the same road: `vcs_suggest_message` calls `settings::languages(&app).commit` for the Git panel's
button, so the message a person is offered and the messages a run writes overnight cannot disagree —
closing only one of the two was the rejected design, since a setting that lies about half its cases
is worse than none. Two costs come with reading it there and both are accepted: a session started in the same
fraction of a second as a language change reads the previous language (the front end writes on a
400 ms debounce, the lag `settings::agent(app)` already lives with), and a run reads the languages
**per batch** rather than snapshotting them, so a language changed at 2am reaches the next batch and
one run's issues can end up in two languages. Putting them on `Intent::Run` instead would be a second
road into a session, which is what reading them in one place exists to prevent.

What each moves is not the same, and `prompt.rs` carries one predicate per language for it. The
conversation language goes into **every** intent. The commit language goes where the agent's own
hands reach git — `commits_to_git`, which is `Run`, `ResolveConflict`, `FixTask` and `Bare` — and it leaves
whatever sits in front of the colon exactly as the project already writes it, along with any
identifier in the message and anything git wrote itself. **It names no form**, and the paragraph
saying `type: subject` with the six Conventional Commits types is the version that was thrown away:
the session prompt said nothing about commit form before this setting existed, `smetana:merging`
commits `merge: <branch> into <target>` with a word that is not one of the six, and
`smetana:provisioning` greps that subject for the branch name afterwards — so a prompt asserting a
convention costs a rewritten merge subject and a blocker nobody can find. This repository's own
commit subjects are Russian words in front of the colon, which is the second reason: a language
field has no business moving a project's conventions into English. `oneshot::commit_prompt` still
names the six, and the difference is who writes the message — there the app composes the whole of
it, so the form is its own to choose. `Bare` is in for the reason the conversation sentence is in
every intent — the ordinary session is exactly where somebody says "commit this" — and `FixTask` is
in because committing the correction is half of what its prompt asks for, while `NewTask`,
`EditTask`, `ResolveTask`, `Setup` and `RepairTracker` are out because they commit nothing: what
`NewTask` writes goes under `.smetana/`, which is not in the repository at all, and a repair session
works on `.beads`, which bd commits for itself. The task language goes where the agent may write
into bd — `Bare`, `NewTask`, `EditTask`, `ResolveTask`, `FixTask` and `Run`. `Bare` is in for
the same reason it is in the commit half: "+ New agent" is exactly where somebody says "file tasks
for this", and a bare session left out of it filed English issues under a Russian setting. The price
is that session opening on three language paragraphs before any work, taken knowingly — and it is
the shape `Run` has always had, since a lead is the other intent in which the conversation, the
issues and the commits are all three true at once, so the cost is one already in the tree rather
than a new one. `Run` carries a fourth on top of those, the report language below, and `Bare` does
not: a bare session writes no batch file. `Setup`, `ResolveConflict` and `RepairTracker` stay out
because none of the three files an issue — and the last of them could not if it wanted to, since bd
is what is broken. The paragraph carries a caveat that is not optional, because what the setting
must never move is a string some other piece of software matches on. The
`##` section headings, since `bd create --validate` matches the wording of a heading and nothing
else, so a translated `## Acceptance Criteria` is bd refusing the issue. And the markers a note
begins with: `parked:` and `resolved:` are matched as literals by
`components/kanban/parked.js`, so a translated one empties `openQuestions` and the parked card's
dialog says nothing is open while the Ready warning goes quiet — silent, and landing on somebody
trying to answer a parked task. What the setting moves is the title, the body of the description,
the criteria themselves and what follows the colon in a note. Specifications and plans are English
whatever either setting says (`IN_ENGLISH` in `prompt.rs`): they are read by whoever picks the work
up months later and by every agent after them.

The report language is the narrowest of the four: `leaves_a_run_report` is `Intent::Run` and nothing
else, since a run's lead is the only session that ever writes
`.smetana/runs/<token>/batch-<n>.json` and a session that never writes one has nothing to hear about
how to word it. What it moves is that file's **prose** — the `did` line per task and the batch's
`notes` — and the paragraph closes two exceptions in a fixed order, the field names first, because a
model that reads the sentence and stops has to have met the half that breaks the document.
`report::parse_batch` reads `tasks`, `id`, `did` and `notes` through serde by literal match, so a
translated key is not a document in another language: it is a batch drawn in the report as having
left no account of itself. An identifier inside a line travels unchanged for the reason it does in a
commit message, and `report::prose` draws it as `<code>`. The last clause names the *other* report —
what the lead says back in the conversation stays under the conversation language — because the two
reports come out of the same batch, and somebody who set this and then watched the terminal would
otherwise have been told nothing at all. `report.rs`'s own labels (`smetana · run report`, `closed`,
`parked`, `batch N`, `<html lang="en">`) do not move and are not mentioned in the prompt: they are
this product's interface copy, CLAUDE.md says interface copy is English, and translating them would
be a table of twelve languages in Rust for words one long. The switch that hides the report changes
none of this — `runs::service::finish` renders the document whatever it says, so the setting goes on
moving text that lands on disk; **Show run report** only decides whether anybody is handed it.

`agentPrompt` is the fifth field of that family and the first that is not a language: a person's own
standing instruction — "talk to me briefly", "this machine has no Docker", "here it is pnpm, not
npm" — written once on the Agents tab and put in front of every session they are actually in. It
travels the languages' road exactly, and that is the load-bearing decision rather than a
convenience: `settings::agent_prompt(app)` beside `settings::languages(app)`, read by
`terminal::service` in the `Create` arm while it builds the `Launch`, carried as `Launch.agent_prompt`
into a still-pure `prompt::build`. It never crosses the IPC and is never an argument to
`terminal_create`. One place builds every session in the app, a person's and a run's alike, so
reading it there once is what makes it impossible for the two to disagree; handing it in from the
front end would be a second road into a session, which is the shape this module exists to prevent.
It lives with the same two costs the languages do, unchanged: the 400 ms debounce, and a value read
per session rather than snapshotted.

Empty by default, and empty is today's behaviour **to the letter** — no framing line, no paragraph,
and not one extra blank line in any prompt. That is the opposite shape from the four languages, which
default to `en` and print their paragraph anyway, and the difference is that a language always has an
answer where a standing instruction usually does not. The person's words are not pasted bare: one
framing sentence (`STANDING`) says whose they are, because everything else in a prompt is this app
asking for something, and "answer briefly" read as a task is a session that answers briefly and does
nothing else.

It lands **after** the four language paragraphs and before the work. Near the front for the reason
the languages are — what is said last can be pushed off the top of what the agent reads first by
seven kilobytes of skill text. After them rather than before because those paragraphs close silent
failures (a translated `## Acceptance Criteria` is bd refusing the issue, a translated `parked:`
marker empties a parked card's questions) and a reader resolves a contradiction in favour of what
came later: a person who deliberately writes across a language setting gets what they wrote, and
everybody else costs the language rules nothing.

`talks_to_a_person` is the predicate, and unlike `writes_to_the_tracker`, `commits_to_git` and
`leaves_a_run_report` it is written as a **negation** — `!matches!(intent, Intent::Run { .. })`. Two
reasons. Those three name a capability a session *has*, and a positive list is the honest shape for
that; this one names the **absence of a listener**, so a list of the conversations would be the
complement of the rule rather than the rule. Neither this paragraph nor the predicate's own doc
writes the count down, on the reasoning `commits_to_git`'s comment already carries: a number is wrong
the next time an intent is added and nothing fails when it goes stale — both of them had come to say
"the eight" over nine of them by the time `ReviewBranch` landed. And a variant added to `Intent`
later is, on the evidence of every variant there is, another conversation: the negation hands it the
instruction for free,
which is the right default, since an instruction reaching one more conversation is benign and missing
one is the bug the field exists to fix. A positive list would leave a new variant out silently — the
same quiet drift this file records about `RESUMES_BY_ID`. `Intent::Run` is the one exclusion: nobody
is in a run's conversation, so an instruction written for one would shape autonomous work overnight
with no one to correct it, on top of the four language paragraphs a run already opens with. That was
offered in the discussion and declined. `ResumeSession` is deliberately **not** named in the
predicate and never reaches it — `build` refuses it a prompt on its first line — and a clause for it
would be dead code wearing the clothes of a decision. `agents::oneshot` is outside all of this too:
it is one question with its answer on stdout, not a conversation anything can be carried into.

The caveman level is the sixth field of that family and the one that is not this app's own words at
all: it is somebody else's skill, and the only way a chosen level reaches an agent is a line in the
conversation. caveman keeps no level on disk — its `SKILL.md` says `/caveman <level>` holds until the
session ends — so the app's first message is the one chance to say it without a person typing it.
`settings::caveman_level(app, project)` resolves a project's `inherit` against the global level and
`terminal::service` reads it where it already reads `agent_prompt`, onto `Launch.caveman_level`, into
a still-pure `prompt::build`. One line comes out of it, a sentence with the command at the front of
it: at the front because that is the form the skill listens for, a sentence because every prompt here
is a whole instruction.

It sits **after** the four language paragraphs and **before** the standing instruction, and both
halves are decisions. After the languages, because those close silent failures and a rule about style
does not belong in front of them. Before the person's own words, because a reader resolves a
contradiction in favour of what came later, and a hand-written "do not abbreviate anything" has to
beat a level chosen on a settings screen.

It carries **no `talks_to_a_person` gate**, which is the opposite answer to its neighbour's and
deliberate: a standing instruction is withheld from a run because nobody is in that conversation to
have written it for, while how compressed an answer is is a question about tokens, and a run's
batches are where the most of them go. What keeps that safe is caveman's own Boundaries section,
which leaves code, comments, commits, issue text and reports as ordinary prose — so neither the
language paragraphs nor `filing-a-task` is touched by it. The first rung, `off`, is **silence rather
than `/caveman off`**: it means the app says nothing about caveman, which is what it said before the
setting existed, and an empty level reads the same way, since a `Launch` can be built with no
settings file behind it. `ResumeSession` never sees the line at all — `build` refuses that intent a
prompt before it composes a word, and a conversation already runs at whatever level it was left at.

Claude Code's `--append-system-prompt` was the rejected alternative, and it would have been a real
system prompt, closer to what the field is called. Only one of the two supported harnesses has such a
switch: Codex has no per-session equivalent — the same asymmetry that already forces
`SkillDelivery::Inline` — so the setting would be an invisible system prompt on one harness and
visible prose in the first message on the other, one field with two behaviours, with a person's
instruction appearing and disappearing from the transcript as they moved between them. It would also
have moved the feature out of `prompt.rs`, which is pure and holds every test in this module, and
into the per-harness `command` builders, which are checked only against captured fixtures. The four
languages already ride as prose in the positional argument; this is the fifth field of that family
and travels the same way.

A setting for the language of *code comments* was
asked for and refused — it would either do nothing in a repository with a convention, or produce
exactly the regression the Language section names.

Two directories under `src-tauri/resources/` are the library itself, both bundle resources.
`smetana/` is ours — the directory is the list, for the reason the test-count note under Commands
gives — laid out as a plugin in its own right (`.claude-plugin/plugin.json`, `skills/<name>/SKILL.md`)
because that is what `--plugin-dir` accepts and what makes them answer to `smetana:filing-a-task` and
the rest. The intents that name one apiece are `NewTask`, which names `filing-a-task`;
`ResolveTask`, which names `resolving-questions`; `Setup`, which names `project-setup`; `Run`, whose
batch names `running-tasks`; and `ReviewBranch`, which names `reviewing-branch-changes`. Named rather
than counted, for the reason the count above was dropped, and because the list is what a reader came
here for anyway. `running-tasks` is the one the rest hang off, since an agent carrying out a batch
reaches `provisioning`, `reviewing`, `merging` and `live-checking` because `running-tasks`
sends it to them, not because the prompt lists them. That is the point of a library over a longer
prompt: the prompt names an entry point and the library carries the depth. `superpowers/` is a
committed copy of that plugin, 668 K of markdown under MIT, with its own `LICENSE` and a
`SUPERPOWERS_VERSION` recording version and commit sha, the way `BD_VERSION` does for the sidecar —
committed rather than downloaded because 668 K of text is not 128 MB of binary, and committing makes
the build hermetic.

The vendored copy is stripped of its `hooks/` directory, the one exclusion that changes behaviour
rather than size. Superpowers ships a `SessionStart` hook injecting "you MUST invoke" into every
session the plugin is loaded into; through `--plugin-dir` that would impose the process on "+ New
agent" and on editing an issue — the two intents this design deliberately leaves alone — and would
make the Brainstorming switch a lie in its Off position. A person who installed superpowers
themselves keeps their own hook, and our copy is never loaded for them. `library.rs` decides that
from `~/.claude/plugins/installed_plugins.json`, where a key is `<plugin>@<marketplace>` and its
value is the list of scoped installs — both halves matter, since a key with an empty list is a plugin
uninstalled everywhere. Anything unreadable answers "no": a second copy costs a duplicate line in a
list, while withholding it removes the feature with nothing on screen to say so. When it is handed
over it keeps its own name, which lets the prompt say `superpowers:brainstorming` in both cases.

**Filing a task is an agent session, not a write.** `NewTaskModal` no longer emits an issue: its
fields become a `TaskDraft` inside a `NewTask` intent, and `DesktopApp.vue` switches to the agents
side tab and the terminal centre tab and calls `createSession`, exactly as "Ask agent to edit" does.
The agent runs `bd create` itself and the watcher puts the card on the board — and `createIssue`,
`tracker_create`, `NewIssue` and `create_args` are deleted rather than left unused, because a live
write path into the tracker that nothing calls is the kind of thing that gets called again in six
months.

The dialog collects one piece of prose, not a title and a description: the person writes what needs
doing in a single `Textarea`, and the title bd wants is written by the agent, the only party that has
read the text. Five `Dropdown`s sit under it in two rows, and every one defaults to **Auto** — type,
priority and Brainstorming, then Spec and Plan. For the first two, Auto travels as `null`, never as
the word, so `TaskDraft`'s `Option<String>`/`Option<u8>` cannot carry a type bd would reject;
`prompt.rs` then names the pinned fields as settled and hands the rest to the agent *by name*
("Decide the priority yourself"), because an agent told nothing about a field would have to invent
one anyway and would not know that inventing it was its job rather than a gap in the briefing.

Brainstorming's three positions: `Off` files it now; `On` requires a discussion first; `Auto` states
the test the agent applies and leaves the judgement to it, since nothing in the app has read the text
of the task and a heuristic on its length would misfire in both directions. How to file one
*properly* is not part of that question — an agent that files without discussing still has to file it
well — so the filing skill reaches the agent in all three positions, by name for `PluginDir` and as
text for `Inline`. `Auto` differs from `On` only in what it hands over for the brainstorming process:
a name for `PluginDir`, already loaded and costing one index line, against the absolute path to the
vendored `SKILL.md` for `Inline`, so a one-line change does not pay for 10 KB it will not use.

**Spec and Plan hang off it, and they cascade rather than sitting beside it.** They are the two
stages the filing session used to stop short of: writing down the design the discussion produced, and
writing the implementation plan (`superpowers:writing-plans`). Spec is a person's to choose only
while Brainstorming is `On`, and Plan only while Spec is — nothing for a design document to record
when no discussion happened, nothing for a plan to plan when no design was written. A stage nobody
may touch **reads as its parent rather than as a placeholder**, so the screen states exactly what
will be sent. The rule is `components/kanban/taskStages.js`, another of the `branchChoice.js` family,
and `agents::cascade` applies it again on the far side of the wire — not a duplicate to tidy away,
since what arrives there is a payload and a payload can carry a spec chosen under a discussion since
switched off. `prompt.rs` normalises before it writes any prose, so such a spec produces no words
about a spec at all. One `Stage` covers all three switches, matching `STAGES` on the front end, and
the collapse was the point: while Brainstorming had an enum of its own, a fourth position added to
`Stage` alone compiled perfectly and left the discussion switch a position short of its children.

The output is files, and the task is filed **last**: the design goes to
`.smetana/docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md` and the plan to
`.smetana/docs/plans/YYYY-MM-DD-<topic>.md` — superpowers' own layout moved under the folder
`runs/gitignore.rs` keeps out of the repository, so nothing is committed. Filing last means an
interrupted session leaves no card promising documents nobody wrote. The paths copied into the issue
are **absolute**, since an ignored file does not travel into the worktree `provisioning` cuts — and
the issue still has to say in prose what was decided, because the files are on one machine. Spec
needs no skill text of its own; Plan is its own skill and follows the trade Brainstorming's `Auto`
makes.

**What a filed task owes is set by the far end of the app, not by the dialog.** `provisioning` says
the description *is* the spec, and a description that never says what "done" looks like is not
something to start on — a thin task is not a smaller task, it is a supervised run stopping overnight
on a question or an automatic one parking the work. The two ends are held together by
`bd create --validate`, which refuses a description missing the sections its type requires
(`## Acceptance Criteria`, plus `## Steps to Reproduce` on a bug, `## Success Criteria` on an epic,
three headings on a decision, nothing at all on a chore). That flag is the whole mechanical part of
the standard, which is why `STANDARD` in `prompt.rs` names it in the prompt rather than leaving it to
the skill: an `Inline` harness may find no skill text to read. **It is a floor and not the standard**
— measured against the pinned sidecar it matches the wording of a heading and nothing else, so an
empty section, a `###` and lower case all pass. It converts "no acceptance criteria" from an
invisible default into something somebody has to do on purpose; judging whether the criteria are real
is `provisioning`'s job. `running-tasks` holds its own filing to the same skill and adds the test
that follows: a finding nobody can state acceptance criteria for is a digest line, not a task.

The other half is what the discussion produces. Brainstorming on `On` buys half an hour of narrowing
down what somebody meant, and none of it is anywhere but that conversation — the agent that picks the
task up months later has the person's original four sentences and nothing else. So `DISCUSS` requires
the outcome, rejected options included, to be written into the issue itself.
