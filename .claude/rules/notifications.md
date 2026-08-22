---
paths:
  - "src/components/notifications/**"
  - "src/stores/notifications.js"
  - "src/components/run/reportDelivery.js"
  - "src/components/run/reportTab.js"
  - "src/components/run/stopReason.js"
  - "src/sounds.js"
  - "src/chime.js"
---

# The bell: what the app has to say right now

The bell in the scope bar opens a panel of notifications, and the badge counts what is in it —
`components/notifications/` (the pure `notifications.js`, `NotificationPanel.vue`,
`NotificationCard.vue`) over `src/stores/notifications.js`. There are two sources — the attachment
store growing, and a run that is over — and the badge counts one card per stopped run whose report
was not put straight in front of the person, beside the one the storage source is ever allowed.

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

**The bell is one of two deliveries, though, and never both.** A card asks to be visited; a report
already open in front of somebody is the visit. `components/run/reportDelivery.js` is the rule — the
`branchChoice.js` family again — and it asks one thing: was the agent that earned this report the one
this person has selected. The selection and nothing else, deliberately: not window focus, since
somebody who left the app with an agent selected comes back to that agent, and not the centre tab
either, since `activeId` survives leaving the terminal because `AgentList.vue` highlights its row
from it. Two absent sessions are never one agent — a run from a worker too old to name its session
met by a window with nothing selected would match under the obvious equality and open a tab neither
asked for — and with no document there is no tab to open at all, which is the same case the card
already draws without a button.

Which run's agent that was is `Run.last_session`, a second field beside `session` rather than a
longer life for it: `session` is cleared the moment a run stops, and must be, because a row pointing
at a dead session is worse than no row — while this decision is about a run that is over by
definition. `Run::working_in` is the one write that fills both, since two assignments at the loop's
one call site would compile perfectly with the second missing and the cost would be invisible: every
report would simply go to the bell, which is a legitimate outcome of this very rule.

Carrying it out is `DesktopApp.vue`'s, because opening a tab is the one thing no store can do, and
two details there are load-bearing. The watcher keeps its own set of tokens it has **decided** about
— the ones left to the bell as well as the ones opened — since `loadRun` replaces the list on every
focus and every project switch, and remembering only the tabs would open last night's document in
front of somebody who happened to select that agent hours later. And it rides the default `pre`
flush: `syncRunCards` makes the card inside `upsert`, so for the moment between that and this the
bell holds a card about to be taken back, and a `pre` watcher runs before this component's own render
in the same tick, so the badge never paints the number. `deliveredInTab` is called only when the tab
actually opened, since suppressing the card on the strength of a tab that never appeared would leave
the person with neither.

**Beside the bell there is a sound, and it is the half that reaches somebody who is not looking.**
The bell is a badge on a bar somebody has to be looking at, and both things it carries happen when
nobody is: a run ends at three in the morning, and an agent inside one stops to ask a permission
question. `src/sounds.js` is what a sound may be — four ids, `off`, and the two shipped defaults —
and `src/chime.js` is the half that touches the DOM, one `Audio` per id, with a rejected `play()`
warned about and swallowed, because a webview may refuse audio no gesture asked for and an app that
throws over a noise is worse than a quiet one. Which sound each event makes is the `notifications`
section of `settings.json` (`.claude/rules/settings.md`), edited on the settings window's General
tab.

Two rules about where it fires, and both are easy to break by accident. The run sound is played in
`upsert` in `runs.js` — the one place a run's state ever changes — once per token, since the summary
arrives seconds after the ending and is another event about the same stopped run; and **never from
`loadRun`**, which replaces the list on every window focus and every project switch and would
announce this morning's run this afternoon. The cost is named rather than hidden: a run stopping
while the window is pointed at another project is not announced at all, because `run:state` is
filtered to the active project, and silence about another project's run is the better failure of the
two. The needs-you sound is played in the `terminal:state` listener in `terminals.js`, comparing the
mark already held with the state arriving and firing only on the transition *into* that state, so a
session re-announcing the same wait costs nothing — for every project, since the marks cover every
project and a person supervising two overnight is waiting on both; and **nothing on the first read
of `terminal_marks`** in `initTerminals`, since those sessions were already waiting before this
window opened. A watcher in `DesktopApp.vue`, the shape report delivery uses, was rejected for the
second of these: `terminalState.sessions` holds the active project only, so the sound would have
gone quiet for exactly the second project the rail exists for.

The sound is also the one announcement the two deliveries above do not divide. It plays whether the
report went to a tab or to the bell, because it is about the run having ended rather than about the
card: somebody who had that agent selected still gets a document appearing in a tab they were not
watching.

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

There are no toasts. The bell is the whole surface: a folder that has grown is not a person waiting
on an answer, a run that has finished is not one either, and the loud budget on that screen is one or
two rows.
