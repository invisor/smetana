---
paths:
  - "src/components/notifications/**"
  - "src/stores/notifications.js"
  - "src/stores/updates.js"
  - "src/components/run/reportDelivery.js"
  - "src/components/run/reportTab.js"
  - "src/components/run/stopReason.js"
  - "src/sounds.js"
  - "src/chime.js"
---

# The bell: what the app has to say right now

The bell in the scope bar opens a panel of notifications, and the badge counts what is in it —
`components/notifications/` (the pure `notifications.js`, `NotificationPanel.vue`,
`NotificationCard.vue`) over `src/stores/notifications.js`. There are three sources — the attachment
store growing, a run that is over, and an update downloaded and waiting — and the badge counts one
card per stopped run whose report was not put straight in front of the person, beside the one the
storage source is ever allowed and the one the update source is ever allowed.

**The list is derived, not an inbox.** A notification is computed from the state of its source and
thrown away when that state goes away; nothing accumulates on disk — no history, no message log, no
read/unread ledger, and the bell's own label says "1 notification" rather than "unread" for that
reason. A durable inbox was considered and dropped: everything this app has any use for announcing is
a statement about something it can look at right now, so a stored copy is a second source of truth
that goes stale the moment the first one moves. The cost is named rather than discovered: there is
nothing to say about the past, and a source that genuinely needs history brings its own storage. A
card stands until it is answered or stops being true, and every measurement under it rewrites its
prose from the size just read.

The one thing that survives a restart is a number per project — `storageWarnedMib` in
`settings.json`. **A threshold is announced once and arms itself again when the size falls back below
it**: after *every* measurement the remembered number becomes the highest threshold the folder still
reaches, so crossing 10 MiB says so once and stays quiet for the next 40, while cleaning down to
3 MiB clears the memory and the next crossing of 10 speaks again. Dismissing is the same write, which
is why there is no dismissed flag: there is nothing a second one could express that this does not.

The ladder is 10, 50 and 100 MiB, weighed against **the active project's subdirectory** of the
attachment store rather than the whole of it, and that follows from the Storage tab: the clean-up
button reaches this project's folder and nothing else, so a warning summing in a neighbouring project
and the unreachable files in the store's root would name a number a person cannot bring down.
Announcing every project's folder was dropped as well, though it is honest: the only action for
somebody else's folder is "switch project first", and it needs the stable project key mapped back to
a path.

The size comes from `attachments_survey` — **the same command the Storage tab reads**, never a second
one, because two commands measuring one folder eventually disagree and the screen a person is sent to
would argue with the card that sent them. `projectBytes` in `settings/storage.js` is the reading of
it: `kept` plus `removable`, and `null` — not zero — whenever the board could not be read, since a
zero taken as a size would announce nothing about a folder that may be full, re-arm the ladder off a
number nobody measured, and offer a Clean up button Rust refuses. So an unreadable board changes
nothing: no card made, none taken away, the remembered threshold left where it is. Freshness is the answer the file tree and
the branch already give — no watcher: at start once the project is resolved, on a project switch (in
`projects.js`, after the new layout has landed *and* after `tracker_set_project`, since the survey is
answered against the worker's idea of the active project), on window focus, and after an attachment
is saved.

Clean up opens the settings window **on the Storage section**: `settings_window_open` takes a `tab`,
a window being built gets it as `?tab=storage` on the URL it already loads, and one already open —
focused rather than rebuilt — is told by the `settings:show` event. That event lives in
`stores/app.js` rather than in `settings.js`'s three-event contract because nothing about it reaches
`settings.json`: the main window is still the only writer.

**The second source is a run that has stopped, and it is what tells anybody the night is over.**
`runNotification` beside `storageNotification`, one card per stopped run keyed `run:<token>`, derived
from `runsState.runs` and gone when the run leaves that list — a project switch, or a run of the same
scope replacing it. `syncRunCards` is called from the three functions in `runs.js` that cover every
assignment of the list, one of them transitively (`startRun`'s filter of a replaced scope's stopped
run, through the `upsert` on the line after it), which is the one place an edit landing between the
two would leave a card up for a run no longer in the list. It rewrites that source's half of `items`
and leaves every other source's cards alone. Which source sits above which is a property of the list
rather than of who spoke last: `SOURCES` declares the order and both writers hand their result to
`arrange`, runs above storage, because a night that has ended is what somebody came back to read
while a folder that has grown will still be there tomorrow.

**The bell is one of three deliveries, and never two of them at once.** A card asks to be visited; a
report already open in front of somebody is the visit; and off is neither.
`components/run/reportDelivery.js` is the rule — the `branchChoice.js` family again — and since
smetana-qnt it asks **one** thing: `notifications.showReport`, the switch on the settings window's
General tab (`.claude/rules/settings.md`). On, a run that has ended opens its report in a tab there
and then. Off, nothing appears at all — not the tab and not the card, because a card is a button onto
that very document and leaving it up would answer somebody who asked not to be shown their reports
with a smaller version of the thing they declined. The sound is a separate answer and keeps playing,
and the run bar still says the run has stopped.

**One condition, and that is the whole point of it.** What stood here before was a second one: was
the agent that earned this report the one this person had selected at the moment the run stopped.
Everything about that check was defensible in itself — the selection rather than window focus, since
somebody who left the app with an agent selected comes back to that agent; and two absent sessions
never read as one agent, since a run too old to name its session met by a window with nothing
selected would match under the obvious equality. What was not defensible was the answer to "why did
my report not open this time", which was a window state nobody could see. So the check was **removed**
rather than put under the switch: kept, the switch would have been one condition of two and the
complaint would have survived it being on.

`Run.last_session` outlived that check and is still written, because the rest of the app reads it —
it is a second field beside `session` rather than a longer life for it, since `session` is cleared
the moment a run stops and must be, a row pointing at a dead session being worse than no row.
`Run::working_in` is the one write that fills both.

What the switch cannot cancel is physics rather than policy. With no document — a run that fell over
before writing one, or one lying outside the open project, which `showReport` in `DesktopApp.vue`
declines and logs — there is nothing for a tab to open, so a switched-on app still leaves the bell's
card, which says how the run ended rather than merely linking to a file. Switched off there is still
nothing at all.

Carrying it out is `DesktopApp.vue`'s, because opening a tab is the one thing no store can do, and
two details there are load-bearing. The watcher keeps its own set of tokens it has **decided** about
— the ones left to the bell as well as the ones opened — since `loadRun` replaces the list on every
focus and every project switch, and remembering only the tabs would open last night's document in
front of somebody who happened to select that agent hours later. And it rides the default `pre`
flush: `syncRunCards` makes the card inside `upsert`, so for the moment between that and this the
bell holds a card about to be taken back, and a `pre` watcher runs before this component's own render
in the same tick, so the badge never paints the number — which is also what keeps the switched-off
case honest: the card `syncRunCards` has just made is taken back before the frame, so the bell never
shows a count for one. `markRunDelivered` — named for the outcome rather than for the tab, since
there are now two ways to reach it — is called when the tab actually opened and when the answer was
`none`, and never for `bell`: suppressing the card on the strength of a tab that never appeared would
leave the person with neither.

**Beside the bell there is a sound, and it is the half that reaches somebody who is not looking.**
The bell is a badge on a bar somebody has to be looking at, and both things it carries happen when
nobody is: a run ends at three in the morning, and an agent inside one stops to ask a permission
question. `src/sounds.js` is what a sound may be — four ids, `off`, and the two shipped defaults —
and `src/chime.js` is the half that touches the DOM: **Web Audio, and never `new Audio`** — one
shared `AudioContext`, one decoded buffer per id, a fresh `AudioBufferSourceNode` per playback, with
every failure warned about and swallowed, because a webview may refuse audio no gesture asked for and
an app that throws over a noise is worse than a quiet one. The prohibition is the load-bearing half,
and the file's own header carries the account behind it together with the warning that the account
was never confirmed: an `HTMLMediaElement` in WKWebView plays through AVFoundation, opens a Now
Playing session, and macOS is believed to bill an unsandboxed app for that with a dialog asking for
the person's Apple Music and media library, raised at whatever moment a run happened to end. The TCC
log that would have proved it was not observed — this is smetana-i4w's hypothesis taken as the
diagnosis, which is why the header also splits what to do if the dialog survives. Either Web Audio
is the same trigger, and the reserve the task kept for exactly that case is playing the sound from
Rust (`NSSound`), at the price of IPC per noise and a platform fork where there is now one file for
every platform; or the fault is not the sound at all, which is the permissions audit smetana-i4w
left out of scope, and there the fork would buy nothing. The log is what tells the two apart, and it
is the first move either way. Which sound each event makes is the `notifications` section of
`settings.json` (`.claude/rules/settings.md`), edited on the settings window's General tab.

Three rules about where it fires, and all three are easy to break by accident. The run sound is
played in the `run:state` listener in `runs.js` — the only channel by which a run reaches `stopped`
in this window at all, since `startRun` hands back a run just started and `stopRun` one merely
`stopping` — and it is rung **above** the check that keeps `runsState.runs` the active project's, so
it plays **for every project** (smetana-0t0). That puts it beside the needs-you sound rather than
apart from it, on the same argument: a sound is the one delivery addressed to somebody who is not
looking at the screen, and which project they happened to leave open when they walked away says
nothing about which ending they want to be woken for — two projects running overnight is the case
both sounds exist for. It fires once per token, since the summary arrives seconds after the ending
and is another event about the same stopped run, and **one set of tokens covers every project**,
because a token is issued once per app process and is unique across projects as well as within one.
What stays *below* that project check is the visual half — the list, the bell cards `syncRunCards`
derives from it, and the report tab — since each of those is a statement about what this window is
looking at, and a card for another project's run would be a button onto a document `showReport`
declines to open. And **never from `loadRun`**, which replaces the list on every window focus and
every project switch and would announce this morning's run this afternoon. The needs-you sound is
played in the `terminal:state` listener in `terminals.js`, comparing the
mark already held with the state arriving and firing only on the transition *into* that state, so a
session re-announcing the same wait costs nothing — for every project, since the marks cover every
project and a person supervising two overnight is waiting on both; and **nothing on the first read
of `terminal_marks`** in `initTerminals`, since those sessions were already waiting before this
window opened. A watcher in `DesktopApp.vue`, the shape report delivery uses, was rejected for the
second of these: `terminalState.sessions` holds the active project only, so the sound would have
gone quiet for exactly the second project the rail exists for.

The third rule is **not a shell**, and it is the one the sound was written without at first. The
listener asks `isShellSession` before it rings, which is `projectStates`' rule by the same word,
because a shell reaches `needs-you` by the shortest path there is: any BEL byte sets `bell_pending`
in `terminal/service.rs`, and layer A of `terminal/detect.rs` turns that into `NeedsYou` with no
profile involved — and a shell has no profile. zsh's `LIST_BEEP` and bash's audible bell are both on
by default, so an ambiguous tab completion would have played the notification sound at somebody
typing into that very tab. The rail already skips shells and the footer's counter already filters
through the same function; a sound that did not would have been the third population and the loud
one, going off while both of those read zero with nothing on screen to explain it. It is asked as
"is a shell" rather than "is an agent" for the reason `isShellSession` gives — work this front end
has never heard of is an agent, and still rings.

The sound is also the one announcement the deliveries above do not divide, **the switch included**.
It plays whether the report went to a tab, to the bell, or nowhere at all, because it is about the
run having ended rather than about the document: a person who turned reports off asked not to have
one put in front of them, not to be left wondering whether their night finished.

**One thing can silence both sounds, and it is the only thing that can**:
`notifications.onlyWhenUnfocused`, the switch under the two sound lists on the General tab
(`.claude/rules/settings.md`), shipped **on**. With it on, either sound plays only while the main
window is not focused — the whole argument for a sound in the first place is the person who is not
looking at the screen, and one played at somebody who is looking is noise. The rule is
`shouldPlay` in `src/sounds.js`, pure and reachable by a test, and the gate is in `chime.js`, whose
signature is `chime(id, { unlessFocused })`: both call sites hand the setting over and neither asks
about focus itself, which is what keeps the DOM question in the one file that has a document.

**What counts as focus is `document.hasFocus()` in the main window, at the moment of the noise** —
no listener, no stored flag, no new IPC event and nothing about focus on the Rust side. Both call
sites live in the main window's stores, so the question means what it needs to mean by construction.
The consequence is named rather than discovered: the settings window is a second `WebviewWindow`
with a document of its own, so somebody working in an open settings window with the app behind it
still hears the sound. Reading both windows' focus is the honest reading of the word "app" and was
rejected for its price — the main window would have to be told about the second window's focus, a
channel of state bought for a word.

**The preview is deliberately outside all of this.** Choosing a sound in a dropdown calls
`chime(value)` with no second argument and plays it every time, at any position of the switch and
with the settings window plainly focused: a preview is somebody listening to a choice rather than
the app announcing something, and obeying the option would make the list silent in exactly the place
a sound is picked — for everybody, since the option ships on.

The import between the two stores is circular by construction — `notifications.js` reads `runsState`,
`runs.js` calls a hoisted function declaration — and **nothing in `notifications.js` may read
`runsState` at evaluation time**, only inside `syncRunCards`. That is not a style rule: the bundler
emits `notifications.js` first, before `runs.js` has evaluated and before the `const` exists, so the
natural-looking improvement — a module-scope `watch(() => runsState.runs)` replacing the explicit
calls — would throw on the built app's first line and leave a white window, while working perfectly
in `npm run dev`, where the browser's module order is the other way round.

**Nothing about it reaches disk**: dismissing adds the token to an in-memory `Set`, and a run no more
survives a restart than a session does, so this source needs no equivalent of `storageWarnedMib`. The
token is issued once per app process and never reused, which is what makes one set safe across
projects.

The card is short on purpose — the ending, `N closed · M parked`, the duration, and one button — and
everything else is in the report the button opens; a card that restated the document would be the
right panel's question block all over again (smetana-s4f). The ending's sentence and glyph come from
`components/run/stopReason.js`, the same table the bar draws, and they go into the **body** rather
than into the title, since several of those sentences carry an em dash of their own and folding one
into a title after a second dash reads as two sentences run together. Every entry in `REASONS` names
its own glyph, and so does the answer for an ending this build has never heard of — while they did
not, the bar and this card each kept their own `?? 'square'`, drifting invisibly.

**An unread board is never a zero** — `summary.tasks` of `null` says the board could not be read
instead of "0 closed, 0 parked", and no `Show details` is offered when there is no document.
**A run carrying no summary at all is a third case, not that one**: nothing has failed to read
anything, nothing has looked yet. `request_stop` ends a run with nothing in flight at once and the
account arrives seconds later through `Run::take_summary_from`, so every press of Stop between
batches passes through this state on its way to the real counts — a card announcing a board failure
there would state a failure that did not happen. It says the ending and nothing else, and is still
announced at all because this front end may also simply be older than the worker.

`Show details` opens the report as an ordinary centre tab, through the very `openFile` the file tree
calls. The one translation is `reportTabPath` in `reportTab.js`: the summary's path is absolute
because a worker that knows nothing of tabs has to name a file on disk, `openTabs` is
project-relative, and separators are normalised on both sides since `files.js` uses `/` while Rust
wrote the platform's. It answers `null` rather than guessing for anything not squarely inside this
project's reports folder.

**The third source is an update that has finished downloading**, and it is the shortest-lived of the
three: `updateNotification` beside the other two, one card keyed `update:<version>`, made from the
`ready` state of the machine in `src-tauri/src/updates.rs` and gone the moment the machine is in any
other state. `stores/updates.js` hands that state to `syncUpdateCard` on every change it adopts, and
the card is derived from it exactly as the other two are derived from theirs — installing restarts
the app, so the card cannot outlive what it is about even in principle.

**Only `ready` is news, and the other five states are deliberately silent.** Checking and downloading
are the app doing housekeeping it was not asked about; announcing either would interrupt somebody
with something they can neither act on nor stop, and the agreed behaviour is that the app fetches
quietly. `failed` is not news either — a check that could not reach GitHub is not a reason to put a
row in front of anybody — and it belongs on the About tab, where a person went looking. So the bell
has one thing to say about updating and it is the one thing a press answers.

The card is **ordinary loudness, never `needs-you`**: loud is budgeted at one or two rows on a screen
and belongs to an agent waiting for a person, while an update that has waited an hour is no worse off
for waiting another. Its button opens the settings window on About rather than installing from the
panel — the same bargain the storage card makes with Clean up, and for a sharper reason: installing
restarts the app over unsaved editor buffers and live terminals, so the press belongs beside the
sentence naming the version and the refusal that may come back from the run gate.

The import direction is the **opposite of the runs source's, on purpose**. `runs.js` and
`notifications.js` import each other, which is the cycle both files carry a warning about; nothing
forces one here, because the state travels *into* `syncUpdateCard` as an argument rather than being
read out of a store. `stores/notifications.js` therefore knows nothing of `stores/updates.js`, and
the natural-looking symmetry with runs — a module-scope watcher over `updatesState` — is exactly the
change that would create the cycle it was written to avoid.

Dismissing is remembered **by version, in memory only** (`dismissedUpdates`), which is the same
argument `deliveredRuns` makes one step smaller: the machine starts at `idle` on every launch, so a
remembered version would name a state nothing can be in. A card dismissed for 0.2.0 says nothing
about 0.3.0, and the next release speaks again.

Both windows keep their own `stores/updates.js`, because a second webview is a second module graph,
and the settings window asking Rust on its own is the whole of why one opened halfway through a
download draws the download. The consequence is named rather than discovered: that window's
notification store collects a card nobody draws there, which costs one object and no behaviour — the
bell lives in the main window alone.

There are no toasts. The bell is the whole surface: a folder that has grown is not a person waiting
on an answer, a run that has finished is not one either, and the loud budget on that screen is one or
two rows.
