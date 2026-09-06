---
paths:
  - "src-tauri/src/agents/**"
  - "src-tauri/resources/**"
  # The front end's whole knowledge of the harnesses, and the one file in `src/`
  # this rule is about: it reads `agents::catalogue` once at startup, which is
  # what replaced four hand-written lists keyed by agent id.
  - "src/stores/agents.js"
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

**Part of that text has a second reader, and rewording it without knowing that is silent.** A prompt
is submitted as the session's first message, so it lands in Claude Code's transcript as a record the
person is down as having typed — and `sessions/kickoff.rs` reads it back out to answer the Sessions
tab's First prompt with what somebody actually wrote (`.claude/rules/terminal.md`). What it matches on
is `pub const` here rather than copied there: `CONVERSATION_TAIL`, which says the message is ours;
`NEW_TASK_OPENING`, `IMAGES_ONE`, `IMAGES_MANY`, `FIELDS_GIVEN`, `FIELDS_AUTO` with
`FIELDS_AUTO_TAIL`, `FOLLOW_UP` and `STANDARD`, which are where the person's words end. Nothing about
the prompt an agent reads changed for it — no marker, no separator, and the output is the same to the
character — and a test builds a real `NewTask` prompt and requires exactly `draft.text` back, which
is the only thing holding the two ends together.

| file | what it does |
|---|---|
| `mod.rs` | `Profile`, `Intent`, `Stage`, `SkillDelivery`, `ImageDelivery`, `TaskDraft`, `Autonomy`, `Launch` — the vocabulary, the registry, `cascade` and `IDS` |
| `library.rs` | where the bundled skills are, whether the person already has their own superpowers, and reading a `SKILL.md` for inlining |
| `prompt.rs` | an intent becomes the text the agent opens on — pure; the skill text, where one is needed, is read by the caller and passed in |
| `claude.rs` | Claude Code: `--plugin-dir`, and layer B, its permission dialog read off the screen |
| `codex.rs` | Codex: `Inline`, `-i` for images, `resume`/`fork`/`exec --json` as subcommands, its own stream translator, and its own layer B (smetana-603) |
| `codex_sessions.rs` | which conversation a Codex session opened, read back out of that CLI's own rollout file after the start |
| `commands.rs` | `agents_catalog`: every shipped harness and what it can do, asked of the profiles |

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
harness ran before they existed.

Both harnesses answer them now. Claude Code opens with `-p --verbose --output-format stream-json`;
Codex with `exec --json`, and the difference in *shape* is the thing to notice — `exec` is a
subcommand, not a flag, which is why `batch_args` goes in front of everything else on the line and
why a profile may answer with a word rather than a switch. **The two translators share no code**, and
that is deliberate rather than an omission: the vocabularies are unrelated — Claude Code's
`assistant`/`tool_use` against Codex's `thread.started`, `item.completed` with an `agent_message`, a
`command_execution` or an `error` inside it, and `turn.failed` — and an event name one harness happens
to use today is exactly what drifts. Both fail the same way round, which is the part worth copying
into a third: an event the translator has never heard of, and a line that is not JSON at all, draw
**nothing**. A missing row costs a person something they can still get out of the CLI's own logs; a
pane of raw JSON costs them the pane. Codex's translator also strips every control character, for the
reason Claude Code's records — `serde_json` decodes an escaped escape or bell into a live byte, and a
pane specified as plain text must not take colour, cursor movement and a bell out of a transcript.

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

`oneshot_answer` is its pair, and it exists for `parse_usage`'s reason one family over: a harness
that says how to ask a question without saying how to read the reply leaves the caller reading
somebody else's format. It takes the whole of stdout and answers with the one answer in it. **The
default is the whole of stdout**, which is today's behaviour to the letter and exactly right for a
harness whose one-shot mode prints the answer alone — Claude Code's `-p` does, so `claude.rs` does
not implement this at all. `codex exec` does not: it writes its own progress, and on the machine this
was measured on a models-cache warning, to the same stdout, so `codex.rs` takes the **last
`agent_message` in the stream** and skips every line that is not JSON. Without it the commit-message
button in the Git panel would have offered a paragraph of log. `oneshot::ask_raw` reads stdout
through this method rather than directly, which is the whole of what makes the difference invisible
to its callers.

It cost that function one invariant, and the trade is worth knowing before a third harness answers
`oneshot_args`. `ask_raw` used to read both pipes only after the child had gone, which was safe
because every *caller* bounded the answer in its own prompt — one line for a commit message, twenty
ids for a search. A per-harness `oneshot_args` breaks that in kind rather than in degree: the bound
was on what the model says, and `codex exec --json` wraps that answer in a stream of the harness's
own events, where a `command_execution` item can carry output nobody asked for. Past a pipe buffer
the child blocks on the write and the deadline kills a question that was answered in the first
second. So both pipes are now **drained on a thread apiece while the child runs**, which is the shape
`vcs::run` already had for git, and the invariant is gone rather than restated — it was a property of
prompts this function cannot see.

The half of that shape worth copying is the one that is easy to leave out. The readers are joined on
the ordinary path and **not** on the two that give up, because killing the child does not close the
pipes: anything the agent started inherited both descriptors, `ask_raw` signals one process rather
than a group, and a join on a pipe with a live writer never returns. A version that joined on the
timeout traded a bounded ninety-second failure for an unbounded hang — the caller never gets its
`Timeout` and the blocking thread is wedged for the life of the process, which is worse than the
stall the draining was added to fix. `a_descendant_still_holding_the_pipes_does_not_hold_the_deadline`
pins it, from the other side of the same case `vcs::run`'s
`the_kill_still_reaches_a_grandchild_that_refuses_to_stop` covers.

`resume_args` and `fork_args` are the newest of them and the pair a whole feature hangs off: how
this harness is told to pick a **recorded session** up again by its id, and how it is told to carry
one on in a *new* session instead, as the arguments in front of everything else on its command line.
Claude Code answers `--resume <id>` and `--resume <id> --fork-session`; Codex answers
`resume <id>` and `fork <id>`, read out of `codex resume --help` and `codex fork --help` at CLI
0.146.0. The capability and the arguments are one answer for `usage_command`'s reason: a profile that
said "yes" without saying how would leave the caller inventing somebody else's command line.

**The two harnesses spell the same capability in different shapes**, which is why these methods
answer with the whole of the arguments rather than a flag to be assembled: Claude Code's are options,
Codex's are **subcommands**, and a subcommand has exactly one legal position. That is also why
`terminal::service` puts whatever comes back in front of everything else on the line. Codex kept both
defaults of `None` until smetana-ti4d, and the reason was the rule rather than the answer — its
grammar is its own and this app does not get to guess it, which is the same rule `claude.rs`'s
`command` already follows about argument *order*. What changed was that somebody read the help.

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
somebody left. The front end greys the rows before that, and what it greys them from is
`agents::catalogue` — see **One catalogue** below. It used to be two lists of agent ids written out
in `components/agent/sessionMenu.js`; the module is still pure and still decides the wording, but the
capability now arrives as a `capable` boolean rather than as an id it looks up.

`clear_command` is the newest and sits beside those three, for their reason and with one difference
that decides everything else about it: **the app knows it wants a conversation forgotten and does not
know the words, and each CLI has its own.** Claude Code answers `/clear`, which is what its `/help`
documents; Codex keeps the default `None`, and that is a decision rather than a gap. It is the one
method of the family Codex still leaves alone, and it is left alone **because the help was read and
carried none**: `codex --help` at 0.146.0 lists no subcommand and no option that clears a
conversation or starts one over. Whatever that CLI's own composer accepts is not in anything it
prints, so there is nothing here to write down. `codex.rs`'s test carries that sentence beside the
version it was read at, which is what a later reader needs in order to know the question was asked
rather than skipped. What parts it from its three neighbours is *when* it is read — they are read
while a command line is being built, this one while a session is already running — so what it
produces is written into that session's input rather than spawned, and the carriage return that
submits it is `terminal::service`'s rather than the profile's, since a profile carrying one would be
describing a keyboard instead of a command.

**A guess here fails silently, which is why the rule is that it is read out of the harness's own help
or left `None`.** A slash command a CLI has never heard of is not an error there: it is ordinary
text, and it reaches the agent as the first line of a prompt with nothing anywhere to say it had
been. That is a worse outcome than the refusal, and the refusal is cheap — `TerminalError::NoClear`,
its own variant beside `NoResume` and `NoFork` and for their reason, since nothing was written and
nothing tried to be.

The caller is `terminal_clear`, which takes a session id and nothing else — the front end never
learns which harness a session runs, `agents::pick` may have substituted one, and a line composed on
that side would be a guess written straight into somebody's prompt. `Request::Clear`'s arm asks the
harness first and the session second, and the second question is the one the feature exists around:
**a session in `needs-you` is refused (`TerminalError::Busy`)**, because an agent waiting for an
answer reads the next line written into it as that answer, so the clearing command would be a pick in
somebody else's dialog rather than a command at all. It is deliberately *not* `terminal_run_capture`'s
stricter pair — that one also refuses on an unrung-out bell, because it writes a question and reads
the answer back, while this writes one line and reads nothing, and a bell is rung at the end of every
turn, which is exactly the moment somebody clears. `bell_pending` is left as it was rather than
cleared the way an ordinary `Write` clears it: nobody has answered anything, and taking the mark away
would spend the signal `run_capture` reads as its own guard.

**Nothing is deleted by any of this**, and that is what the menu row rests on: the transcript stays a
file, the Sessions tab goes on listing it, and the session, its tab and its row in the panel are all
the ones they were. So the row asks for no confirmation, where `DeleteSessionModal` asks and has to.
Its glyph is an eraser and deliberately not the bin beside it — the two verbs must not look alike.

The front end greys the row before anybody can press it, off `Profile::clear_command`'s own answer
carried by `agents::catalogue`. `components/agent/agentMenu.js` used to hold a third hand-written
table of agent ids for this; it now takes a `clearable` boolean instead and stays pure. It is asked
about the *configured* agent out of `settings.json` rather than about what actually started, since
nothing on screen ever learns what `pick` substituted. The two sides ask their refusals in the same
order, harness before row, so a row refused twice over is worded the same on the greyed label and in
the toast.

`discovers_session_id` and `session_id_after_start` are the other half of `session_id_args`, and the
question they answer is the one that method cannot: **what if the harness names its own
conversation?** Claude Code is told which one to write (`claude --session-id <uuid>`), so
`terminal::service::conversation_for` mints an id before the spawn and the record is written the
moment the process exists. Codex has no such flag at 0.146.0 and never will have this app's id —
it chooses its own and writes it into the first line of its rollout file,
`$CODEX_HOME/sessions/<yyyy>/<mm>/<dd>/rollout-<timestamp>-<uuid>.jsonl`, in a `session_meta` record
carrying `session_id` and `cwd`. So the id is **discovered rather than assigned**, and that is a
second capability rather than a fallback inside the first: being told an id and finding one out are
different things, a harness may have either, and a caller composing one out of the other would be
inventing somebody else's behaviour — the same rule `resume_args` and `fork_args` are two methods
for.

Three methods rather than one, and each is separate for a reason a reader will otherwise take for
redundancy. `discovers_session_id` is a plain question with a `false` default, asked **while a
session is being built** and answered without touching a disk; `session_id_after_start` reads one,
and a disk read standing in for a capability check would answer "no" on a machine that simply has
not written the file yet. `sessions_before_start` is the third and is explained below, because what
it is for only makes sense next to the thing it prevents. All three default to nothing, which is
what every harness but Claude Code recorded before any of them existed.

`agents::codex_sessions` is the implementation and it is **read-only, which is a boundary rather than
an implementation detail**: nothing of this app is written into a person's own Codex directory, the
same line `SkillDelivery::Inline` already draws. It walks the day directories under `$CODEX_HOME`
(`~/.codex` when unset) and reads **only the first line** of each rollout — these files reach tens of
megabytes and everything after that line is the conversation.

Three conditions pick the answer out of that walk, and the third is the one that had to be learnt.
`cwd` stops a Codex session running somewhere else entirely from being claimed by this card. The
spawn time stops a session that finished before this one started. Neither stops the case that
actually bites: **only a resumed session runs in a directory of its own** — every other intent runs
at the project root — so two agent sessions in one project share a `cwd` as a matter of course, and
an older one *still writing* has the newer modification time, matches on `cwd`, and was last touched
after the new session spawned. It would be handed over, and the damage is not a wrong row but a lost
one: the new session's `cwd`, work and start time would be written into `.smetana/agents.json` under
the older session's id, destroying its record, and closing the new session would then delete it
outright.

So `sessions_before_start` takes the walk **once, in front of `Pty::spawn`**, and
`session_id_after_start` considers only paths that were not in it: a file that did not exist before
the spawn cannot belong to a session that started before it. Paths and not a timestamp, because
`Metadata::created` is not answered on every filesystem this ships on.

*In front of the spawn* is load-bearing and not a figure of speech. Taken any later — inside the arm
that handles a successful spawn, say, after the session and its `Live` are built — the child is
already running, and a harness quick enough to get its rollout onto disk inside that window would
find **its own file** in the set it is being told to ignore, so its id would never be discovered at
all. The instant the search compares against is taken there for the same reason.

`codex resume --last` was the rejected alternative, and it was rejected because it is an ambiguity
taken as the design rather than as an edge. What remains after all three filters is a session
**started in the same folder inside the few seconds this is looking**, and that is accepted.

One thing the walk deliberately does not do is widen itself. `session_meta`'s payload carries
`session_id` at 0.146.0, with an `id` beside it holding the same UUID; an older CLI wrote `id` alone,
which is on disk on the machine this was measured on. Those files are skipped, and that is right
rather than a gap: the session being looked for was written moments ago by the installed CLI, so a
rollout only an older one could have produced is by construction somebody else's.

`terminal::service` spends it. `discovers_its_own_id` is the gate — the same one `conversation_for`
applies, plus the harness question the other way round: there is an id to find exactly when the
profile could not be told one and says it names its own, so a resume (which knows its id already)
and a fork (which deliberately records nothing) are both out. The lookup runs on a **thread of its
own**, ten tries half a second apart, because the file does not exist at the instant of the spawn and
the worker answers every other command in the app. What comes back is `Request::SessionIdFound`,
whose arm fills `Session.conversation` and writes the `.smetana/agents.json` record the spawn could
not write. **Giving up is silent**: a session with no id recorded is exactly the session Codex had
before, and a tab closed inside those five seconds is an ordinary outcome rather than a fault. The
sender that thread holds is a `WeakSender`, so nothing about this keeps the worker's own
"the senders are gone" arm from ever firing.

What it buys is one thing and it is worth naming precisely: a Codex conversation comes back as a
**restored-session row in the agents panel** after a restart, and pressing it runs `codex resume`
rather than starting a fresh agent in that worktree. It does **not** reach the Sessions tab.
`src-tauri/src/sessions/` reads `~/.claude/projects` and parses Claude Code's transcript format;
Codex writes a different format in a different place, so with Codex configured that tab lists nothing
and its cards' Resume and Fork rows are not the road this capability is reached by. A second source
for that tab is its own subsystem and deliberately not part of this. So is reading Codex's allowance:
it writes `rate_limits` — a `primary` five-hour window and a `secondary` weekly one, each with
`used_percent` and `resets_at`, which is exactly the pair `runs::usage::Usage` carries — into that
same rollout file, but that would be a **second kind of source on `Profile`** (a file rather than a
command) and a separate decision about how stale a reading may be. `usage_command` and `parse_usage`
therefore stay `None` for Codex, and the Subscription block says the allowance could not be read
rather than inventing zeroes (smetana-7rp).

## One catalogue, instead of four lists

`Profile::label` is the newest method and the **only one with no default**, deliberately: a harness
added to `IDS` without a name must not compile. It is this product's interface copy — "Claude Code",
"Codex" — and it lives beside the profiles rather than in the front end because the front end no
longer names agents at all.

`agents::catalogue()` is what that buys: one row per id in `IDS`, carrying the label and a
`Capabilities` of six booleans — `resume`, `fork`, `clear`, `usage`, `batch`, `oneshot` — every
one of them **derived by asking the profile** (`resume_args("probe").is_some()`, and so on) rather
than written out beside it. The probe string is never used; those methods are asked only whether
they answer at all. `agents::commands::agents_catalog` is the one command over it, and
`src/stores/agents.js` reads it **once at startup**, in `main.js`, before anything is mounted.

That once is the whole design. The front end used to keep four hand-written lists keyed by agent id —
labels in `settings/AgentSettings.vue`, ids that resume and fork in `agent/sessionMenu.js`, ids that
clear in `agent/agentMenu.js` — and a fifth of labels in `shell/usageFooter.js`. Each existed for a
good reason, which the catalogue keeps: **the answer has to be in hand while a row is being drawn**,
and a row greyed a round trip later is a row somebody has already pressed. Each was also a knowing
second copy of a fact Rust owns, free to drift in both directions in silence, and a third harness
meant five edits in two languages. Nothing in the catalogue changes while the app runs — the set of
shipped harnesses is fixed at build time — so one read at startup satisfies both.

The pure modules stay pure, which is what keeps them reachable by a test at all: `resumeAvailability`
takes `capable`, `agentMenuItems` takes `clearable` and `usageAgentLabel` takes a `nameFor`, each
handed in by the caller rather than looked up. A read that fails leaves the list empty and greys every
capability row, which is the safe direction — Rust refuses an unsupported verb with a sentence anyway
(`NoResume`, `NoFork`, `NoClear`), so being wrong here costs a row nobody can press rather than a
command written into somebody's prompt. `stores/mockBackend.js` answers `agents_catalog` with the
same shape, and it is now the **only place in `src/` that holds a list of what agents can do**: a
browser has no Rust to ask, and without it `?view=gallery` and `?view=settings&tab=agents` would
draw an empty picker.

That claim is about *capability lists* and deliberately not about the string `codex` appearing
anywhere, which would be a claim this tree does not support and nobody could keep: `settings.js`,
`SettingsWindow.vue` and `AgentSettings.vue` each default a field to `'claude'`, and `Gallery.vue`
names both ids in its own fixtures. A default and a fixture are one value apiece — wrong, they cost a
picker that opens on the wrong row — while a *list* is the shape that goes stale in silence and takes
a menu with it. The property to keep is the second one.

**A third harness is one file in Rust plus its id in `IDS`, and no list anywhere under `src/` to add
it to.** That is what to check a change against before putting anything keyed by agent id back into
the front end.

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
comes back as something else. The front end never learns the names either, and it no longer even
carries one across the boundary: `terminal_create` takes the project and the intent, and Rust reads
the harness off the file for that intent's role (`settings::role_model`, below). A configured agent
that is not on `PATH` falls back to the first one that is, and `Session.agent` carries what actually
started; nothing on screen reads it, so the substitution is silent and the terminal is the only way
to see it. When nothing at all is installed the session fails with `NoAgent`.

## Which agent, and on which model

`Profile` answers two questions about models, and the split is the one `usage_command`/`parse_usage`
already makes: what a harness offers, and how it is told which one to use.

`models()` is **the second method here with no default**, beside `label` and for its reason: a
harness added to `IDS` without a model list would ship an empty dropdown in the settings window,
which reads as a load that failed rather than as a decision anybody made, and the compiler is the
cheapest place to find that out. It answers `&[(id, label)]` — the id that goes on a command line and
the name a person reads beside it.

`model_args(model)` is how the harness is told, as the arguments themselves: `["--model", m]` for
Claude Code, `["-m", m]` for Codex. Its default **is** an empty vector, and that is a working answer
rather than a gap — the shape `autonomy` and `batch_args` keep: a harness with no such flag simply
cannot be told, so the setting is inert for it instead of broken.

**The ids are read off the installed CLI and never recalled**, which is this file's standing rule
about somebody else's vocabulary applied one field over. Claude Code's own `--help` names the
aliases outright, and aliases are what is offered rather than full names: an alias points at the
latest model of its family, where a full name written into somebody's `settings.json` pins a version
that goes stale where nobody looks. Codex's help documents the flag and **no ids at all**, so its
list comes from the model catalogue that CLI ships inside itself — the same table its own picker is
drawn from — taking every entry it marks visible, in that catalogue's own order, with its own display
names. Each profile's `MODELS` carries where and when it was read; a re-reading updates that note
along with the list.

The list travels to the front end as a field of the `agents_catalog` row, beside the capabilities and
for the reason recorded above: the front end draws a model picker per harness, and a hand-written
table over there would have been the fifth of the lists this command exists to have abolished.

### Role, and `role_of`

An `Intent` says why a session is being started; a **`Role`** says which row of the settings window
decides its harness and its model. There are five — `Tasks`, `Code`, `RunLead`, `ReviewBranch` and
`Default` — against eleven intents, and the count is the design rather than an economy. Eleven rows
is a settings screen nobody reads, and it would still not have separated a run's lead from the
subagents it delegates to, since both live behind `Run`.

`agents::role_of` is the mapping, pure and here rather than in `settings/` for the reason
`prompt::build` is pure: reading somebody's file is not a rule about intents, and this half has to be
testable without a disk. `NewTask`, `EditTask` and `ResolveTask` are `Tasks`; `FixTask` and
`ResolveConflict` are `Code`; `Run` is `RunLead`; `ReviewBranch` is its own; `Bare`, `Setup`,
`RepairTracker` and `ResumeSession` fall to `Default`. A test walks all eleven and names the role of
each, so a variant added to `Intent` meets a decision rather than a wildcard.

**The lead is its own role and not the code one**, which is the distinction the whole feature turns
on. A run's session is a lead: it reads the board, claims a batch, cuts the worktrees and delegates
the implementation to subagents, then reviews and merges. A `--model` flag on that session sets the
**lead's** model and nothing else, because the subagents are spawned by the lead inside its own
harness and take that harness's default. "Opus writes the code" and "Opus leads the run" are two
different requests and one flag cannot carry both.

**`ResumeSession` is never told a model**, whatever the file says, and it is guarded twice — the
shape the chosen session id already has. `settings::role_model` refuses this intent one, and each
profile's `command` refuses to put the flag on the line. The reason is `prompt::build`'s own for
declining that intent a prompt: `--resume` (and `codex resume`) continue a conversation that already
has a model, and this app arriving with a second opinion is talking over the person whose session it
is. Whatever was settled in there was settled before this window existed.

### One resolver, and the one place it is called

`settings::role_pair(app, role)` is the file half — a role's own pair where it names a harness, the
root pair whole where it does not — and `Settings::role_pair` beside it is the pure rule it wraps.
`settings::role_model(app, intent, chosen)` is what `terminal::service`'s `Create` arm calls while
building the `Launch`, in the same breath as `settings::languages(app)` and for the identical reason:
this is the one place every session in the app is built, so a person's session and a run's batch
cannot come to disagree about which harness and which model this kind of call gets. `settings::agent`
is gone — every caller now asks for the pair, because a harness read apart from its model is exactly
how a model chosen against one provider reaches another.

**`pick` is the third substitution and had to be guarded too.** `agents::pick_with_model` is `pick`
with the pair rule on it: the fallback to the first installed harness is untouched and still silent,
and the model is dropped whenever it fires, because a model id chosen against one provider is not one
the substitute has ever heard of. All three callers take it — the `Create` arm and both one-shots —
and it is one function rather than three `filter`s because the fourth caller added later is the one
that would forget. Without it the guard existed everywhere except where it mattered most: the
settings window offers every shipped harness whether or not it is on `PATH`, so choosing Codex and
one of its models on a machine with only Claude Code turned a fallback that used to work in silence
into `claude --model gpt-5.6-sol` and a session dead at its first argument.

`chosen` is the other seam, and it exists for a run. `None` is a caller with no opinion — the front
end, which knows nothing about roles. `Some(id)` is a harness the caller already holds and will not
give up: a run snapshots its own `RunLead` harness when it starts and carries it for the whole of
the run, so that the allowance gate and the batches cannot land on two different ones (smetana-3fi).
**A pinned harness that disagrees with what the file now says arrives without a model**, which is
the indivisible pair applied at that seam rather than a special case.

`Launch` carries the two answers: `model`, this session's own, and `worker_model`, the `code` role's,
which is meaningful only for `Intent::Run` and is `None` for every other intent rather than a value
nothing reads. The second reaches the agent as one line of the run policy and never as an argument —
`.claude/rules/runs.md` carries what that line says, that it is a request rather than a guarantee,
and why Solo does not get it.

The two one-shot calls have no session and therefore no `Intent` to ask with: the Git panel's
commit-message button and the tracker's semantic search both take `settings::default_pair`, which is
the `Default` row and therefore the root pair, asked through the same resolver so that one place
decides what "the default" means. `oneshot::ask` and `ask_raw` take the model beside the profile and
put `model_args` on the line in front of the positional prompt.

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
`settings::role_pair(app, …)` already does, and `terminal::service`'s `Create` arm calls it while building
the `Launch` — the one place every session in the app is built, so a person's session and a run's
batch get the same answer by construction. From the `Launch` the ids reach `prompt::build`,
which stays pure. The commit language has one reader outside a session, and it reads the same field
by the same road: `vcs_suggest_message` calls `settings::languages(&app).commit` for the Git panel's
button, so the message a person is offered and the messages a run writes overnight cannot disagree —
closing only one of the two was the rejected design, since a setting that lies about half its cases
is worse than none. Two costs come with reading it there and both are accepted: a session started in the same
fraction of a second as a language change reads the previous language (the front end writes on a
400 ms debounce, the lag the harness itself already lives with), and a run reads the languages
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
same quiet drift this file used to record about the front end's own lists of agent ids, before
`agents::catalogue` took them away. `Intent::Run` is the one exclusion: nobody
is in a run's conversation, so an instruction written for one would shape autonomous work overnight
with no one to correct it, on top of the four language paragraphs a run already opens with. That was
offered in the discussion and declined. `ResumeSession` is deliberately **not** named in the
predicate and never reaches it — `build` refuses it a prompt on its first line — and a clause for it
would be dead code wearing the clothes of a decision. `agents::oneshot` is outside all of this too:
it is one question with its answer on stdout, not a conversation anything can be carried into.

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
