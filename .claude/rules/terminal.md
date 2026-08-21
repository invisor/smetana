---
paths:
  - "src-tauri/src/terminal/**"
  - "src-tauri/src/shell_env.rs"
  - "src/components/terminal/**"
  - "src/components/agent/**"
  - "src/stores/terminals.js"
  # Half of the tab row is derived from the sessions this file is about — the
  # Agent tab and one tab per shell — so the rule has to load for whoever is
  # editing that half, not only for whoever is editing the worker.
  - "src/stores/tabs.js"
  - "src/components/shell/**"
---

# The terminal: agent sessions, and one shell

The centre's `terminal` tab (`chat` before it grew a terminal — `ProjectState::validate` migrates the
old name on load, since files on people's disks carry it and without the substitution that tab would
fail the closed-list check and silently become the board) runs CLI coding agents under real PTYs, one
per session, listed in the sidebar's Agents view (`components/agent/AgentList.vue`) and started from
its "+ New agent" row, from the `+` button beside the pinned tabs, or from the task inspector's "Ask
agent to edit". The reason the subsystem exists at all is the second half of that sentence: it notices
when an agent is waiting on a human, including one in a tab nobody is looking at.

**That tab is drawn on demand and is not stored anywhere.** It exists exactly while the project has an
agent session — live in `terminalState.sessions`, or still coming up in `terminalState.starting` —
and `hasAgentTab` in `src/stores/tabs.js` is the whole of the rule, over `hasAgentSession` in
`terminals.js`. Before that it was pinned beside the board, so a project opened on an empty folder
offered a tab whose entire content was an empty terminal. A *start* counts and not only a session:
a spawn takes about a second, and a tab that appeared only when the worker answered would leave the
button somebody pressed with no visible effect for that second — the same reason `starting` exists for
the panel at all. When the last agent goes the tab goes with it, and a person standing on it lands on
the board; `dropAgentTab` is that rule and a `watch` in `DesktopApp.vue` is what notices, because
`tabs.js` is one half of an import cycle with `settings.js` and a module-scope `watch` there would
read this store at evaluation time — the failure `notifications.js` carries its own note about.

The one seam that costs something: `project.activeTab` **is** remembered, so a project last left
watching an agent comes back naming a tab that cannot exist yet, sessions deliberately not surviving a
restart. `ProjectState::validate` passes `terminal` through unchanged on purpose — it does not know
how many sessions there are, and it is the only place the `chat` migration can land — so the repair
is `restoreTabs` in `tabs.js`, beside the identical repair for a diff tab, and it is guarded rather
than unconditional: a project *switch* can arrive at a project whose agents are running, and taking
somebody to the board then would be repairing what was not broken.

An agent started for a piece of work opens on it. What `terminal_create` takes is not a prompt but an
`Intent` — file this task, edit that issue, or nothing at all — plus the id of the agent to run; the
words are the profile's business (see `.claude/rules/agents.md`), and `build_command` in `pty.rs` adds
only what every session alike needs: the working directory, and then `apply_environment` — `TERM`, the
locale, and the bundled `bd` on the front of `PATH` — which the shell branch below shares with it.
Whatever prompt the profile does produce rides as the agent's positional argument. Not as
bytes written after the spawn — the agent takes a moment to come up, and anything sent into an input
that is not reading yet is lost with no acknowledgement to wait for and no way to tell that it went.

That `PATH` line is load-bearing rather than tidy. Filing a task is the agent running `bd create`,
and this app's bd is a sidecar in the bundle: on a machine that never installed one there is nothing
on `PATH` to find, and the flow works in `npm run tauri dev` only because a development machine
happens to have bd of its own. `sidecar_dir` derives the directory as tauri-plugin-shell does,
`dirname(current_exe())`, so it is the same directory `app.shell().sidecar("bd")` resolves to by
construction. It goes in front of the inherited value, never behind: the app pins a bd version and
checks it, and an agent that found some other bd first would be writing to the board through a
version that handshake never verified.

What that directory goes in *front of* is not the `PATH` this process inherited, and
`src/shell_env.rs` is why. A bundled app on macOS is handed launchd's environment: `open smetana.app`
gives it whatever `launchctl getenv PATH` says, which on a stock machine is nothing, so it falls back
to `/usr/bin:/bin:/usr/sbin:/sbin`. Everything a person installs — `~/.local/bin`,
`/opt/homebrew/bin`, nvm's shims — reaches `PATH` from `~/.zshrc` or `~/.zprofile`, which only a
shell ever reads. So the app asks a login shell once (`$SHELL -i -l -c`, the value fenced between
markers because an interactive rc file writes shell-integration escapes into the same stream), and
that answer is what everything that starts a program works from — `agents::pick` and `build_command`
here, `runs/usage.rs` and `runs/preflight.rs` in the run worker. Whether an agent is installed and
the environment it is started with are one question, and answering only the first would trade "no
agent is installed" for an agent that cannot find `git` or `node`. `-l` alone is not enough: the
machine this was written on adds cargo and the rest from `~/.zshrc`, which only `-i` reads. Every
failure — no shell, a five-second timeout, unrecognisable output — falls back to the inherited value.
The bug is invisible in development, which is why it is a module rather than a line: `npm run tauri
dev` starts the binary from a terminal, so the process already has the full `PATH`.

## Not every session is an agent: the shell

`SessionWork::Shell` is the one entry in that enum with no `Intent` behind it, no profile and nothing
this app asked the process to do. It is the person's own `$SHELL` (`shell_env::shell`, `/bin/sh` when
launchd hands the bundle no environment) in the project's root, reached through `Request::CreateShell`
and the `terminal_shell` command, beside `terminal_create` and deliberately not inside it: that one
takes an agent id and turns it into a command line through a profile, and none of those words mean
anything here.

**A shell profile in `src-tauri/src/agents/` was the rejected design, and the reason is `agents::pick`.**
That module is about intents, prompts and the quirks of named harnesses; a shell has none of them, and
a profile for one would join the cascade `pick` walks, where the "take whatever is installed" fallback
could pick it — or, worse, where it could stand in front of a real agent. So the shell is its own
branch through the same worker instead.

**The environment is one piece of code and not two.** `pty::apply_environment` is what both branches
call: `TERM`, the locale rules, and `PATH` with the sidecar's directory in front. The reason it is
shared rather than copied is the third of those, and the cost of drift is silent — a shell built from a
second copy would sooner or later start without the bundled `bd` in front, and then it runs whatever
`bd` the machine has against a board whose version this app pins and checks. `build_shell_command`
passes no flag: a PTY is what makes a shell interactive, and `-l` would be a second reading of somebody's
profile files that their own terminal application does not do. What is *not* shared is everything above
that line — no autonomy variables and no `BEADS_ACTOR`, both pinned by their own test in `pty.rs`: a
person's own bd commands belong in the audit trail under their own name.

`Pty::spawn` and `Pty::spawn_shell` differ in exactly the command they build; the PTY, the reader
thread, the ring and the kill path are one `Pty::start` below both. In particular **there is no second
way to end a session**: closing a terminal tab is `removeSession` → `terminal_remove`, the path that
already exists, and what that path does is `Pty::kill` — the child is killed outright and the session
leaves the worker's map. It is not the graceful path: that one is `kill_all`, described below, which
hangs up the whole process group and waits, and which runs when the window closes rather than when a
row or a tab is closed. Whatever the shell itself started outlives a `terminal_remove` exactly as an
agent's own children do; that is the existing behaviour of this command and not something a shell
introduces.

Detection runs over a shell and is welcome to. Layer A is agent-independent and there is nothing about
it to switch off; a shell that rings the bell has rung it for the person sitting in front of it, and
nothing in this app acts on a shell's state — it has no row in the agents panel, it is not counted by
the scope bar or by the project rail, and notifications are raised by a run rather than by a session
going `needs-you`. **That sentence is a property to keep true rather than an observation**: it was
false for as long as `SessionMark` carried no work kind, since a shell ringing the bell lit its
project's tile loud (smetana-low), and anything new that reads a session's state owes the same
filter — `isShellSession` over a session, `mark.kind` over a mark. Layer B is skipped, because
`DetectInput::profile` is `Option` and a shell's is `None`: layer B is one named harness's interface
being read, and handing a shell's screen to whichever profile happened to be configured would be a
reading of something that is not there. **Do not close that hole with a stub profile** — see the
paragraph above for where it would end up.

On the front end `terminals.js` keeps both kinds in one `sessions` list, because the worker does and a
second list would be one to hold in agreement with it. `isShellSession` is the whole of the
difference, and where its readers are is the thing to know rather than how many there are — a number
written here is wrong by the next commit. They are the derivations at the top of `terminals.js` —
`agentSessions` and `shellSessions` read it, `lastAgent` and `hasAgentSession` read those — which
between them decide that a shell has no row in the panel, that it has a tab of its own, that the
selection repair never lands on one, and that a project holding only shells has no Agent tab. The one
reader outside that file is `noSessions` in `TerminalView.vue`, which asks the same question about an
empty state; everything else goes through a derivation rather than the predicate. `createShell` mints
no start ticket, unlike `createSession`: a ticket buys the second before the worker answers, and it
buys it for a panel that would otherwise draw an empty state over a row somebody just asked for — a
tab that is not there yet draws nothing at all, so there is nothing to cover.

## A shell's tab

Each shell gets its own centre tab, and those are **derived from the sessions too** — `terminalTabs` in
`tabs.js`, after the diffs, one per `SessionWork::Shell`. A second array beside `diffTabs` was the
rejected design for the plain reason that the list already exists in `terminalState.sessions`, and
deriving it is also what makes a project switch free: `loadSessions` brings the new project's sessions,
the tabs follow, and there is nothing to reset. The id is a zero byte, `term:` and the session id —
zero byte for the reason the diff ids have one (these sit in the tab row beside file paths and can land
in `project.activeTab` with them, and no filesystem allows one in a name), and the session id so the
tab and the shell behind it cannot come apart. The label is `Terminal` plus the shell's position among
the shells, which is what a person counts; closing the first renumbers the second, and that is the
honest reading of a position rather than a bug.

**Closing the tab kills the shell.** They are one act, exactly as closing a terminal window is: a tab
that only hid a live shell would leave a process nobody can see and nobody will remember to stop, and
the tab is a view of a session anyway — there is nothing else in it to close. `closeTerminalTab` works
the neighbour out before the kill and moves the selection only if the removal actually took, so a
refusal leaves both the shell and its tab where they were.

**The other direction is deliberately not symmetric.** A shell the person exits themselves — `exit`, or
Ctrl-D — reaches `Exited` and stays in the worker's map, which only `Request::Remove` empties, and the
tabs are derived by `work.kind` rather than by state: so the tab stands, with a dead PTY behind it and
its last screen still in the ring, until somebody closes it. That is the intended behaviour and not an
oversight — it is what a terminal emulator with "close on exit" switched off does, it leaves the last
words of whatever was running there to be read, and closing it is the same one gesture as before.

What such a tab draws is `TerminalView.vue` with a `sessionId` prop, and the Agent tab is the same
component with `terminalState.activeId` passed in. The prop is what the pane attaches to, sends
keystrokes to and resizes; the pane reads no selection of its own. That is the other half of the
`activeId` split above — and the whole reason two shells do not open on one scrollback, since the pane
holds one xterm instance per *view* and refills it from the ring of whichever session it was named.

Nothing about a shell reaches `settings.json`: it is not in `openTabs`, which is paths, and a tab id
that lands in `activeTab` is rejected on the next launch by `validate` (it is neither of the two names
nor a path) and again by `restoreTabs`.

## Dropping a file on the panel

Dropping a file on the terminal panel types its absolute path into the session's input and stops
there, the way iTerm and Terminal.app do it. **Return is not sent**: a path is nearly always part of a
sentence — "look at X and fix it" — rather than the whole of one, and a message sent on somebody's
behalf cannot be taken back out of an agent. The text ends in one space so the person goes on typing
around what landed. Both places `TerminalView.vue` is drawn get this at once, the Agent tab and a
shell's tab, because it is the pane's behaviour rather than the tab's.

Nothing is copied anywhere. That is the difference from a task's attachments, which `attachment_import`
copies because a path written into a filed issue has to outlive somebody tidying `~/Downloads` next
week; a live session has no such duty — the agent reads the file within the minute — and a copy would
cost storage, narrow the gesture to the four image formats `sniff` knows, and leave rubbish nothing
refers to.

**Three pieces, and the split is the point.** `components/terminal/dropPaths.js` is the whole of the
text rule, pure and outside the component because a `.vue` file is the one thing no test here can
reach (`tests/components/terminal/dropPaths.test.js`). A path goes in bare and takes single quotes only
when it needs them — shlex's own safe set — since the ordinary case is a path with nothing special in
it and a bare one reads better in the middle of a half-typed sentence; an inner quote is written
`'\''`. **Quoting is not the whole of the text rule**, because this string is typed into a PTY rather
than parsed by a shell: a control character is read by the line discipline and the TUI before any
shell sees a word, and single quotes do not reach it — a line feed or a carriage return in a filename
*is* a Return, the one keystroke the gesture exists not to press. Such a path is refused outright and
per path, never stripped or escaped: a repaired path would no longer name the file somebody dropped,
and the other files of the same drop still go in.

`watchSessionDrops` in `terminals.js` is the subscription, over the webview's `onDragDropEvent`,
mirroring `watchDrops` in attachments.js down to the browser case — `getCurrentWebview` throws there,
which is an ordinary mode and gets one debug line. It converts the event's *physical* position with
`toLogical(devicePixelRatio)` and hands over CSS pixels, and no opinion about whose drop it is.

**Whose drop it is is a hit test, not layout arithmetic.** `TerminalView.vue` asks
`document.elementFromPoint` at that point and takes the drop only if what is drawn there is inside its
own xterm host. The same question settles the argument with the new task dialog for free: with that
modal open the point lands on its scrim, so the panel refuses of its own accord and the two subscribers
on the one window event never need to know about each other — which is why there is no dispatcher
between them, and why a future overlay is separated by the same property rather than by a list of
exceptions. The pane's own drop response is a sibling of the host and carries `pointerEvents: 'none'`
for exactly this reason: taking pointer events would make the response itself the answer to the hit
test, and it would switch off the instant it appeared.

The response — a frame and one line of caption over the terminal — is drawn only while a live session
is behind the panel. `send` already drops what is written to a session still coming up, so there is
nothing to promise in that state and nothing is promised. Without any response at all the gesture is
invisible and indistinguishable from the broken state this replaced, which is the reason it exists.

| file | what it does |
|---|---|
| `model.rs` | `Session`, `SessionState`, `Question`, `TerminalError` — the vocabulary, and the pure rules for entering and leaving each state (`Session::apply`, `finish`) |
| `transcript.rs` | a batch's machine-format output cut into lines and handed to the profile's own rendering, before anything downstream sees a byte of it |
| `ring.rs` | the raw-byte scrollback ring, trimmed on overflow to a line boundary |
| `screen.rs` | a `vt100` grid built from the same bytes — the text a person would actually see |
| `detect.rs` | layer A: bell and silence, a pure function of the screen, the bell flag and the timings |
| `pty.rs` | the only file that touches the OS: spawns, reads, writes, resizes, kills; also assembles the child's environment |
| `service.rs` | the worker: one owner of mutable state, request queue, output and state events |
| `commands.rs` | thin `#[tauri::command]`s, shaped exactly like the tracker's |

`service.rs` is a single tokio task, the same shape as the tracker's worker and for the same reason:
commands, PTY output arriving from per-session reader threads, and a 16 ms flush tick all meet in one
`select!`. A session starts at a fixed 120×30 before any view has attached to it; the first
`TerminalView.vue` that does replaces that with the pane's real geometry through `terminal_resize`,
which also feeds the new size into `screen.rs` — the app is obliged to read the screen at the size a
person actually sees.

**One stream, two models.** Every chunk from a PTY goes into `ring.rs`, a raw byte buffer for the
human — exactly what xterm.js repaints itself from on attach — and, separately, into `screen.rs`, a
`vt100` grid for the app. The raw stream is cursor moves and repaints with nothing findable in it; a
`\r` overwriting "thinking..." with "done" is two writes in the ring and one line on the screen.
Detection reads the screen, never the ring. xterm.js is a third, independent emulation fed the very
same bytes, so the person's picture and the app's agree by construction rather than by hand. A
batch's chunk is translated before any of the three has seen it, inside `absorb` itself, and that
position is the whole of why it is safe: one translation ahead of the fork leaves all three reading
one identical stream, where translating for the pane alone would set them arguing.

`seq` plays the part `generation` plays for the tracker: every flushed output event carries a
monotonic number, `terminal_attach` hands back the ring's snapshot plus the `seq` to continue from,
and `terminals.js` re-attaches on an out-of-sequence event. Attaching clears whatever that session
had queued to flush — it is already in the snapshot just handed over.

Output only flows to the front end for the **active** session — `flush()` drops a background
session's pending bytes on the floor every tick, because nobody is rendering them. **State flows for
every session, active or not** — `reassess()` walks all of them — and that asymmetry is the entire
point: a background agent's row can turn `needs-you` while its bytes never leave the worker.

Detection is two layers that degrade in one direction only. Layer A (`detect.rs`) is
agent-independent — a bell, or three seconds of stillness — and has nothing in it to break. Layer B
is `Profile::question`, so it lives with the agent it reads rather than in this subsystem:
`agents/claude.rs` reads Claude Code's own interface, and a version bump to that CLI can break it. It
did: the dialog was a box until 2.1, and the frame was what told it apart from any numbered list in
the agent's own output. Today it is fenced by horizontal rules with bare lines, so two other
properties carry that weight — the options number themselves 1, 2, 3 … and the **last** such block on
the screen is the dialog, since anything merely printed sits above it; and exactly one option carries
the cursor, which prose never does. The question is the run of text directly above the options,
ending at a blank line or at the rule under a diff preview, and must end in a question mark. And
`idle` is deliberately quiet: a finished agent and a waiting agent both simply stop producing output,
so loudness comes only from the bell or from a layer B match, never from silence alone.

That last rule is a rule **plus one named exception**: Claude Code's one-off folder-trust dialog,
whose question is not the paragraph above the options — a link caption is (smetana-xh7). The second,
narrower reading opens **only after the generic one has declined** and only under a heading from
`const HEADINGS`, a literal table of strings such a dialog prints and ordinary output does not.
Neither guard is relaxed for it: the search stays fenced between heading and options, and the
question mark is still required, because dropping either would let a diff preview or a numbered list
in the agent's own prose turn a session `needs-you` against a budget of one or two loud rows a
screen. `claude.rs` carries the rules and the refusals it was measured against, over fixtures
captured under a PTY off claude 2.1.226. A wording change on Claude Code's side loses the reading and
leaves layer A in place, which is how the rest of that file already fails.

**`SETTLE` (150 ms of a still screen) is a condition for *entering* `needs-you` and never for staying
in it**, and that asymmetry is load-bearing rather than an optimisation. A dialog is not drawn
instantly, and a half-drawn frame would match a truncated question — so a session that has not been
loud is read by layer B only once its screen has held still. A session that *is* already `needs-you`
is asked whatever the screen is doing, and **what makes that safe is the match itself rather than any
earlier reading**: layer B answering at all is the evidence that the dialog is still standing there,
and a session already this loud cannot be made louder by reading one. Often no settled reading came
first — `needs-you` is reachable from the bell alone, with layer B never consulted, and Claude Code
rings as it *starts* drawing its dialog, so the ordinary sequence is bell, then layer A's `needs-you`
on a half-painted screen, then that very frame handed to layer B on the next tick because `was` is
already `needs-you`. That is where the threshold really is bypassed, and the cost is bounded: the
state is `needs-you` either way, so what a partial frame can get wrong is the question's text and its
option list in the right-hand panel, for a tick or two, corrected as soon as the screen settles —
and `claude.rs`'s own guards (a question mark required, the last numbered block, exactly one option
carrying the cursor) make even that unlikely. What forced the split is the person: typing an
answer redraws the screen on every keystroke, so it never settles while they type, layer B went
unasked on those ticks, layer A answered `running` — and the agent row, the scope bar's counter, the
project header and the project tile all flickered yellow to blue and back at the speed of typing
(smetana-4a6). The dialog had not moved; only the input row under it had. What releases the hold is
layer B failing to match rather than any clock: the moment somebody presses Return and the agent
wipes the dialog, layer A has the very next tick, with no ceiling to wait out. Holding it
unconditionally while the screen is unsettled was the rejected version — a working agent repaints its
spinner oftener than every 150 ms, so yellow would stick for the whole of a run. The named price is
one tick: a half-drawn frame the profile cannot read mid-typing dips the state to `running` for ~64
ms, which is why the current state reaches `detect` as `DetectInput::was`, filled by `reassess` from
`live.session.state` before this tick's `apply`, and why `detect` is still a pure function with no
clock and no memory in it.

**Quiet is measured on the screen, not on the byte stream**, and that is what `Quiet` in `detect.rs`
exists for. An agent that is waiting can still be talking: Claude Code 2.1 repaints an open
permission dialog about every 0.61 s for as long as it stands there, and while quiet meant "no bytes
arrived", every one of those chunks restarted the clock — so a session waiting on a human read as
`Running` for as long as it waited and `IDLE_AFTER` was unreachable (`smetana-8h7`). A repaint that
draws the same text changes nothing a person could act on, so what gets timed is the picture they
see. The rule cuts the other way too, deliberately: a session whose screen holds still for
`IDLE_AFTER` is called idle even while bytes pour in, which is the honest reading and cheap to be
wrong about.

**That second half is a rule about a screen a harness draws for a person, and one kind of session
sits outside it**: a run's batch, whose pane is a rendered transcript of a machine-format stream
(`Live::transcript` in the worker, `DetectInput::transcript` beside it in `detect.rs`). Such a
harness emits bytes only when a tool call begins or ends, so the picture holds still for the whole of
a five-minute `cargo test` with the agent working flat out — stillness meaning the opposite of what
it means on a TUI. **That mechanism is read off the stream's own event types rather than watched on
a live Autopilot batch under this build**, the same standing the smetana-8h7 fix below has. Layer A
therefore never calls such a session idle; the bell and layer B sit above it untouched, so
`needs-you` is still reachable from either. The price is named and small: a batch wedged dead reads
as running until its process exits, and nothing waits on that state — a run waits on the exit code,
and the only readers of a batch's idle were the dot in the agent list and `configFreshness`, which
is the pair the rule exists for (smetana-07o). The marker is `transcript` and not `agents::is_batch`
on purpose, and that is the narrower fact rather than the tidier one: Codex runs its batches
interactively, has no translator, and a still screen of its means exactly what a person's session's
does.

`Quiet` keeps a hash rather than the screen — this runs for every live session on every detection
tick, and holding the previous screen would mean copying kilobytes per session per tick. **The
fingerprint deliberately covers the plain text of the visible rows and nothing else**: no colour, no
bold or reverse, no cursor. So an attribute-only repaint, or the cursor moving over unchanged text,
counts as stillness — and **feeding attributes into it would bring the bug straight back**, since an
agent waiting on a person redraws its dialog to keep the highlight under the selected option alive,
which is a colour repaint of identical text. Getting it wrong that way is silent: a session needing a
human reads as busy. Getting it wrong the other way, for a spinner that animates purely in colour,
costs a dashed dot instead of a spinning one.

**Half of `smetana-8h7` is fixed and half is not, and the difference matters when changing this.**
The silence half is closed **for repaints that redraw identical text — the mechanism the fix assumes,
and not one that has been observed on the dialog it was aimed at**; the live check could not reach a
permission dialog without spending model quota, and the trust dialog is no stand-in, having been
measured emitting zero bytes after the first 0.6 s. The bell half is not closed: Claude Code still
rings none on a permission prompt. What an unmatched layer B produces is `Idle`, which reaches the
front end as `ready`, whose loudness is `live` — so in a session a person started, the whole visible
cost of a waiting agent no profile could read is a dashed dot instead of a spinning one, and in a
run's batch, which layer A never calls idle at all, it is the opposite dot: a spinning one for as
long as the process lives. **Nothing shouts, nothing dims, and nothing else in the app acts on the
state at all**, and `NeedsYou` comes only from a bell or from a profile's own match.

An agent that has genuinely finished still reaches `Idle` at about three seconds, but not to the
millisecond, and the drift goes both ways: earlier, because the last bytes a CLI writes are often
invisible ones the old clock counted and this one does not; later, because the clock is stamped when
the worker next looks rather than when the screen changed, so it lags by up to one detection interval
(`REASSESS_EVERY` × `FLUSH`, ~64 ms today). Lengthening that interval lengthens this error with it.

`terminal_run_capture` — the call an automated flow uses to drive a session and read back its settled
screen — refuses with `busy` when the session is `needs-you`, and also when a bell is still unrung
even if state hasn't caught up yet (state lags the fact by up to `SETTLE` plus a tick; the bell flag
is that same fact arriving sooner). Writing into an open permission dialog would answer, on a human's
behalf, a question the app never read and the human never saw. **What that guard cannot catch is the
other half of `smetana-8h7`**: a dialog whose agent rang no bell and whose profile failed to read it.
Layer A calls that session `Idle`, which is the truth and not a refusal — and `Running` for a run's
batch, which it never calls idle at all, no more of a refusal than the other. An idle session is
exactly what a capture expects to write into, so `Idle` can never join this guard without breaking
the ordinary case, and layer B is therefore the whole of the protection here. The capture's own
settle is the one place the stream is still the right thing to measure, and deliberately the opposite
of what layer A does: a capture has just written into the session and is waiting for an answer to
arrive at all, so a screen that happens to look unchanged mid-answer is not a settled one, and
reading a half-finished reply as finished would hand a caller the wrong text with nothing to say so.

Sessions do not survive a restart, and nothing about them is written to `settings.json` — a session
row with a dead process behind it is worse than an empty list. `RunEvent::Exit` calls
`terminal::service::shutdown`, and the worker ends every session the way closing a terminal window
does: `SIGHUP` to the session's process group — which reaches whatever the agent itself started, as
`SIGKILL` to the direct child would not — then a short wait, then a kill for what is left. The two
seconds `shutdown` itself waits are the ceiling on a *wedged worker*, the same one `settings.js` puts
on its close-time flush: the window always closes, and a worker that never answers costs the cleanup,
not the app. Anything that outruns that, or that the app never got a chance to signal, is an orphan;
for the sessions a *run* started, the next launch finds them again through the registry in
`.claude/rules/runs.md`, the one place a session's pid is written down.

`src/stores/terminals.js` keeps the same cost-driven split as the worker: `sessions` and `agentRows`
hold every session's state, cheap and needed for a background row's colour; output bytes go only to
the callbacks registered through `subscribeOutput` — in practice the one live `TerminalView.vue`.
That register is a `Set` and every subscriber gets every chunk: a single field would tie
unsubscribing to who mounted last, exactly the ordering the rest of this subsystem refuses to depend
on.

`liveAgentCount` reads that same session state and is the scope bar's agents counter
(`.claude/rules/git-head.md`): the agent list minus the rows that have finished, which is every
session whose state is not `exited`, plus the starts `visibleStarts` says belong to this project.
`needs-you` counts, which is the whole decision: an agent waiting for an answer is why somebody is
looking at the bar, and a counter that fell by one the moment attention was demanded would point away
from the thing it exists to point at. It is a count of its own rather than `agentRows.value.length`
because those rows carry elapsed times off a thirty-second clock, and this number has no business
being recomputed by it.

`activeId` used to name two things, and conflating them was a real defect: "which agent the human has
selected" has to survive leaving the terminal tab, because `AgentList.vue` highlights its row from
this same field, while "which session the worker is streaming output to" has to end the moment that
view unmounts. While a single field served both, leaving the tab cleared the selection and the
terminal came back permanently blank. **They are two fields now**, and what forced the split is the
shell below: a pane drawing a session that has no row in the agents panel would have moved the
highlight onto a row that does not exist and taken the Agent tab off the agent it was showing.
`activeId` keeps the one meaning it is named for — the agent a person picked — and the transport's
half is `streaming`, a module-scope variable in `terminals.js` beside `seq` and `attaching`, written
by `attach` and cleared by `detach`, which nothing draws and nothing outside that file can see. The
output listener filters on it rather than on `activeId`; filtering on the selection would drop every
byte of a shell on the floor. `detach(id)` takes the id it is leaving: switching sessions is two IPC
calls with no ordering guarantee at the worker, so a nameless detach arriving after the new attach
would silence the session the human just switched to, with no error anywhere — and `detach` forgets
`streaming` only when it still names the session being left, which is the front-end half of that same
rule. Neither function touches `activeId`: selection is not the transport's to forget, and attaching
is not the same act as choosing.

A session's row is captioned by the **work** it was started for, never by the process behind it, and
`SessionWork` in `terminal/model.rs` is what an `Intent` reduces to for that purpose — which of its
payload is drawn and which was only a briefing for the agent. `Intent::work()` lives in
`agents/mod.rs` rather than in `terminal::model` because it is knowledge about `Intent`, and the
answer moves whenever a variant does: a `NewTask` carries its prose, type and priority across for the
draft panel to draw and leaves its `images` and its Brainstorming, Spec and Plan switches behind,
since those are instructions to the agent and nothing on screen would show them.

`SessionWork::Run` carries nothing at all, and that absence is honest rather than lazy: **which
issues a batch has taken cannot be known here.** The agent claims one by running `bd update <id>
--claim` itself, which the app hears about only as the tracker changing under the watcher — there is
no channel that says "this session took this issue". So `claimedBy` in `terminals.js` reconstructs it
from the two halves already on the front end: the run knows which session is working, the tracker
knows what is `in_progress`. An explicit report from the agent would be steadier and needs the agent
to send one; until then this is the reconstruction, written down as one.

`loadSessions` guards against its own stale response the same way `files.js`'s `stale` guards a
buffer: called twice in flight, without the guard the *last response* would win rather than the *last
call*, and the list could end up showing one project's sessions under another project's name — after
which the remove button in `AgentList.vue` would kill the wrong project's agent, silently. A test in
`tests/stores/terminals.test.js` pins this.

Beside that list, and separate from it on purpose, `terminals.js` keeps a second structure about
**every** project: `marks`, a map of session id → `{id, project, state, kind}`, exported as the computed
`projectStates` (path → `{state, live, loud}`) and the `projectState(path)` accessor. The project
rail draws one dot per project and cannot ask `sessions`, which is one project's by design — a row
for a project this window is not pointed at would offer a button that kills somebody else's process.

Three things feed the map and there is no polling: `terminal_marks`, read once in `initTerminals`,
and the `terminal:state` / `terminal:removed` listeners, which the worker already emits for every
session of every project — `upsert` throws the foreign ones away, so the mark is set *before* it
runs. The first read is wrapped in a `try`: a rail of grey dots is a smaller loss than an
`initTerminals` that threw and took the agents panel with it. Marks rather than a total per project,
because the events arrive one session at a time and a store holding only counters could not tell
whether the session that just left `needs-you` was the last loud one where it lived. `loud` beats
`live`; `starting` counts as live for the reason it counts in `hasAgentSession`; `idle` counts as
neither, being a live process with nothing to say. **A shell counts as nothing at all**, dropped by
`kind`, which is the one field of the mark that is not simply `Session`'s: `SessionMark` in
`terminal/model.rs` is its own type rather than `Session` for the reason `Request::Group` gives about
the pid — every project's sessions cross here, and `Session` carries `work`, which for a filing agent
holds the whole of the person's own draft prose. So the mark takes the variant of that work and none
of its payload, as `WorkKind` beside it, whose words are `SessionWork`'s own tags and are held to
them by a test. Both paths that build a mark fill it in — `Request::Marks` for the first read, and the
`terminal:state` listener for every session opened after it; a mark built without the kind is an
agent as far as the rail is concerned.

`TerminalView.vue`'s pane and its host both carry `minWidth: 0`, and that is not decoration next to
the `minHeight: 0` beside it. A flex item defaults to `min-width: auto` and refuses to shrink below
its own content — here, xterm.js at whatever width it was last fitted to — so narrowing the centre
column left the pane painted over the task panel, and converging visibly as `ResizeObserver` →
`fit()` → new cols → redraw fed each other. `KanbanBoard` and `FileEditor` never showed it only
because `overflow: auto`/`hidden` zeroes that automatic minimum for them already.

`TerminalView.vue` hosts one `Terminal` instance per view, not per session — switching agents calls
`reset()` and refills from the new ring snapshot, so returning to an agent lands at the end of its
output rather than wherever it was scrolled to. An instance per session, the way `editor/states.js`
keeps one `EditorState` per file, would fix that; it is not built because the lack has not been shown
to matter. `AgentList.vue` reads `attentionLevel` the same as the board's status badges, but draws it
with a triangle for `needs-you` against a dot for everything else — colour is never the only signal
here either. That triangle is the *whole* of what the app says about a waiting agent, and
deliberately: the right panel used to draw the selected agent's question with a button per option
above the task card, and it was removed (smetana-s4f) because it repeated what the terminal a few
centimetres away already showed, pushed the card the panel exists for down the column, and its option
labels — whole sentences, in a permission dialog — did not fit the panel's width. A person answers in
the terminal. The question still travels: `Session.question` is what layer B fills in and what puts
the session in `needs-you`, and `terminal_run_capture` still refuses to write into one; nothing draws
it, and `answer()` in `terminals.js` went with the block rather than being left as a write path
nothing calls.

In a browser, `mockBackend.js` answers `terminal_list` with one fixture session already sitting in
`needs-you` with a real permission question attached (there is no shell in that fixture: nothing in a
browser could stand behind one) — the only way `?view=gallery` and `npm run dev`
can show that state with no Rust worker behind them — and `terminal_attach` replays a canned
transcript. Every write falls through to the same loud rejection the tracker's writes get.
Both commands that *start* something — `terminal_create` and `terminal_shell` — are among those
writes deliberately: a session handed back with no PTY behind it would put a row in the panel or a tab
in the centre whose terminal could never say a word. `terminals.js` translates `NoAgent` into its own
message naming what was looked for, rather than the
generic "nothing was created": it is the one failure in that list a person can act on, and since a
task is now filed by an agent, it is the difference between a missing convenience and no way to put a
card on the board. The names in it come from the error's own text, because Rust holds the only copy.
