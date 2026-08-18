---
paths:
  - "src-tauri/src/terminal/**"
  - "src-tauri/src/shell_env.rs"
  - "src/components/terminal/**"
  - "src/components/agent/**"
  - "src/stores/terminals.js"
---

# The terminal: agent sessions

The centre's `terminal` tab (`chat` before it grew a terminal — `ProjectState::validate` migrates the
old name on load, since files on people's disks carry it and without the substitution that tab would
fail the closed-list check and silently become the board) runs CLI coding agents under real PTYs, one
per session, listed in the sidebar's Agents view (`components/agent/AgentList.vue`) and started from
its "+ New agent" row, or from the task inspector's "Ask agent to edit". The reason the subsystem
exists at all is the second half of that sentence: it notices when an agent is waiting on a human,
including one in a tab nobody is looking at.

An agent started for a piece of work opens on it. What `terminal_create` takes is not a prompt but an
`Intent` — file this task, edit that issue, or nothing at all — plus the id of the agent to run; the
words are the profile's business (see `.claude/rules/agents.md`), and `build_command` in `pty.rs` adds
only what every agent alike needs: the working directory, `TERM`, and the bundled `bd` on the front
of `PATH`. Whatever prompt the profile does produce rides as the agent's positional argument. Not as
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
ending at a blank line or at the rule under a diff preview, and must end in a question mark. Layer B
is trusted only once the screen has held still for `SETTLE` (150 ms). And `idle` is deliberately
quiet: a finished agent and a waiting agent both simply stop producing output, so loudness comes only
from the bell or from a layer B match, never from silence alone.

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

`activeId` looks like it names one thing and actually names two, and conflating them was a real
defect: "which agent the human has selected" has to survive leaving the terminal tab, because
`AgentList.vue` highlights its row from this same field, while "which session the worker is streaming
output to" has to end the moment that view unmounts. While a single field served both, leaving the
tab cleared the selection and the terminal came back permanently blank. `detach(id)` takes the id it
is leaving: switching agents is two IPC calls with no ordering guarantee at the worker, so a nameless
detach arriving after the new attach would silence the session the human just switched to, with no
error anywhere. `detach` never touches `activeId` — selection is not the transport's to forget.

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
`needs-you` with a real permission question attached — the only way `?view=gallery` and `npm run dev`
can show that state with no Rust worker behind them — and `terminal_attach` replays a canned
transcript. Every write falls through to the same loud rejection the tracker's writes get.
`terminals.js` translates `NoAgent` into its own message naming what was looked for, rather than the
generic "nothing was created": it is the one failure in that list a person can act on, and since a
task is now filed by an agent, it is the difference between a missing convenience and no way to put a
card on the board. The names in it come from the error's own text, because Rust holds the only copy.
