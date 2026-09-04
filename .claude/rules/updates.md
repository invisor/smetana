---
paths:
  - "src-tauri/src/updates.rs"
  - "src/stores/updates.js"
  - "src/components/settings/update.js"
  - "src/components/settings/AboutSettings.vue"
  - ".github/workflows/release.yml"
---

# Updates: what the app knows about a newer version of itself

The app checks a release feed for a newer version, downloads what it finds, and replaces itself when
somebody presses a button. `src-tauri/src/updates.rs` is the whole of the state machine and the three
commands over it; `src/stores/updates.js` mirrors that state into a window;
`src/components/settings/update.js` turns it into a sentence and one control; `AboutSettings.vue`
draws the row. The other end of it is not code the app runs at all —
`.github/workflows/release.yml` is what publishes the release the app reads, and
`src-tauri/tauri.conf.json` holds the endpoint and the public key it is verified against.

`lib.rs` registers `tauri_plugin_updater` and `tauri_plugin_process` — the fetch-and-replace and the
relaunch — manages `Updates::default()` and calls `updates::schedule`. `Cargo.toml` records what the
two cost in dependencies: 32 `[[package]]` entries for the updater (22 TLS, 6 archive, 4 the work
itself) and 1 for process, counted rather than guessed.

## The state machine is in Rust, and that is against this tree's habit

Everything else the app asks the desktop for is driven from a store, so this being a Rust module is a
decision. Two reasons, and either alone would be enough.

**The About tab lives in the settings window, which is a second OS window a person closes as soon as
they have read the version.** A download driven from that window's store dies with it — a hundred
megabytes half fetched and nothing left to say so. Here the download is a task the app owns, and
closing the window it was started from changes nothing about it.

**And the run gate below cannot be answered in the front end at all.** `runsState.runs` is filtered
to the active project, so the front end does not know whether a run is live in a neighbouring one.
The authority is the run worker's map, and it is reachable only from Rust.

What the module is inside is a `Mutex` rather than a worker, unlike `tracker/`, `terminal/` and
`runs/`. Those serialize calls that take seconds; nothing here is slow *behind* the lock — the
network is outside it and what it guards is three fields. `Updates::with` takes a **synchronous
closure**, which is what makes "the lock is never held across an `await`" structural rather than a
rule somebody has to remember. A poisoned lock is taken anyway: there is no invariant between those
fields a panic could break, and refusing to answer the version row for the rest of the session would
be the larger fault.

## The state travels whole, and the command is what a late window reads

`UpdateState` is one tagged value — `idle`, `checking`, `available`, `downloading`, `ready`,
`failed` — and never a set of flags a window has to reassemble. A tag is also what keeps a state this
front end has never heard of from silently reading as one it has: an unknown `kind` matches nothing,
where a missing boolean is indistinguishable from `false`.

Every change is emitted on `updates:state`; `updates_state` answers the same value on demand. **The
guarantee that a window opened halfway through a download draws the download is the command's, not
the event's** — so `stores/updates.js` subscribes *before* it reads and lets an event that arrived
first win over the read's older answer. That ordering looks like something to tidy and is not, and
the front end says so: `tests/stores/updates.test.js` pins it as "lets a state that arrived by event
stand against the first read". Nothing on the Rust side states it and nothing can — `updates_state`
answers the state it is in and has no idea who subscribed when.

Every transition on `Machine` is guarded by the state it comes from and a transition that does not
fit is ignored, which is what makes a late callback harmless — progress arriving after its flow has
failed finds a machine that is no longer downloading. **`failed` is the one exception and is
unguarded deliberately**: there is only ever one flow in flight, so a failure always belongs to the
state the machine is in, and a failure nobody is told about is the one thing worse than a failure. A
reader taking the guard rule literally would expect a failing flow to be swallowed once its own state
has moved, which is the opposite of what happens. `check` is accepted from `idle` and `failed`
only. `ready` refuses because a check from there would fetch the same release again over the one
being offered; `available` refuses for the sharper reason that it lasts only the two statements
between finding a release and asking for its first byte, and a second flow started in that window
would have its transitions swallowed by the first one's guards. Nothing ever rests in `available`, so
refusing from it costs nothing and buys "only one flow is ever in flight".

`FIRST_CHECK_DELAY` is a minute, `CHECK_INTERVAL` a day, `PROGRESS_TICK` 250ms — an event per
downloaded chunk would be a progress bar drawn more often than the screen refreshes, and the count
stays exact whatever the telling. `updateLine`'s idle sentence in `components/settings/update.js`
quotes that schedule in prose, and `tests/components/settings/update.test.js` pins the sentence, so a
changed interval fails a front-end test that names no constant. README's `## Releases` section used
to quote it too and no longer exists: smetana-t3y made README a product document and moved the
release mechanics to `RELEASING.md`, which describes what a release publishes and says nothing about
when the app checks — so this file and that sentence are now the two places the schedule is written
in prose.

## Downloading is automatic; installing never is

Reaching `ready` is something the module does by itself. Leaving `ready` is only ever
`updates_install`, because the app holds unsaved editor buffers and live terminals, and a relaunch
nobody asked for loses them.

## The run gate, and why a relaunch is destructive rather than rude

**An install is refused while any run is live anywhere**, including in a project nobody is looking
at. `live_runs` asks the run worker `Request::LiveProjects`; `live_projects` in `runs/service.rs`
answers with every project its map holds an entry for, sorted and deduplicated, **whatever state the
run is in** — filtering by `is_over` would let an install through in exactly the seconds a stop is
being carried out, when a batch is still in flight. It is the same count `runs/awake.rs` holds a
power assertion for.

The cost of getting it wrong is not that somebody is interrupted. `updates_install` ends with
`app.request_restart()`, and `request_restart` rather than `restart` deliberately: it goes through
the event loop whatever thread it is called from, so `RunEvent::Exit` fires in `lib.rs` and
`terminal::service::shutdown` hangs up and then kills every PTY child exactly as an ordinary quit
does. **A run is the app driving itself for hours with nobody watching, and its agents are those PTY
children.** Restarting under one does not pause it; it kills it. The direct `restart` skips the exit
event when it happens to be called on the main thread, which would orphan the same processes instead
— worse, not better.

The refusal names the projects (`UpdateError::RunLive { projects }`), because a button that will not
act and will not say why sends somebody to guess. A run worker that cannot be reached is
`UpdateError::Runs` and is **refused rather than allowed**: silence is not permission when the cost
of being wrong is an agent killed mid-task.

## One switch, over the timer alone

`updates.autoCheck` in `settings.json` decides whether `schedule` reaches the network by itself and
decides nothing else. `settings::updates_auto_check` is asked **at every tick** rather than read once,
which is the whole of why the switch takes effect with no restart; the timer keeps ticking either
way, so there is nothing to start up when it comes back on. A platform that will not name a config
directory, or a file that will not parse, answers `true` — the switch exists to *stop* a request, so
an unreadable file must not silently strand somebody on an old build. `.claude/rules/settings.md`
holds the rest, including the **four** copies of that default — Rust, `defaults()` in
`stores/settings.js`, `view` in `SettingsWindow.vue`, and the prop default in
`GeneralSettings.vue`. The fallback two sentences up is not a fifth copy: it derives from
`Settings::default()` rather than spelling `true` again. It is a fifth place the answer has to come
out right, and the one a sweep of the four walks past.

The press on About goes on working with the switch off, because a press is not the app acting on its
own, and anything already downloaded stays staged and installable.

## Not from a development build

Under `debug_assertions` no timer runs, a check answers `failed` with a sentence saying so, and an
install is refused. This is not caution. On macOS the plugin derives where to install from the
running executable's own path, and for `target/debug/app` that is `target/debug` itself — the install
would `remove_dir_all` the whole build directory and move an unpacked `.app` into its place.

## No ACL grant, deliberately

`capabilities/default.json` lists **nothing** for either plugin, and the absence is a decision the way
`autostart.rs` records the same one. The Rust side reaches the updater through `UpdaterExt`, a
`Manager` extension outside the ACL, and relaunches through `AppHandle::request_restart()`, a plain
method, so no permission is required by anything today. Added back "for consistency" nothing fails to
compile, no test goes red and `src-tauri/gen/` is git-ignored, while `plugin:updater|download_and_install`
becomes reachable from the webview — **the one route by which a page could replace the bundle without
passing the run gate.** The second half of that record lives in the `description` field of
`capabilities/default.json`, JSON having no other comment slot; tidying it back to "enables the
default permissions" deletes half of it with no diff anywhere else.

## The bell gets a third source, and only for `ready`

`stores/updates.js` calls `syncUpdateCard` on every state it adopts, including `null`. Only `ready`
becomes a card. **Checking and downloading are not news** — they are housekeeping nobody asked about
and can neither act on nor stop — and `failed` is not either: a check that could not reach GitHub is
not a reason to interrupt anybody, and it belongs on the About tab where a person went looking.

Nothing about the no-inbox rule in `.claude/rules/notifications.md` changes. The card is derived from
the state of its source exactly as the other two are, and it cannot outlive what it is about even in
principle: installing restarts the app and the list starts empty again. That file holds the rest —
ordinary loudness rather than `needs-you`, the button opening About rather than installing from the
panel, dismissal remembered by version in memory only, and the one-way import that keeps this source
out of the `runs.js` cycle.

## The signing key

`npm run tauri signer generate` produced a minisign pair, once, and it was given a password. The
public half is committed as `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`. **This signature
is Tauri's own and cannot be turned off**, which is a different thing entirely from the Apple code
signing below: one is what an installed copy checks before it replaces itself, the other is what
macOS checks before it will run the result, and neither substitutes for the other. The `check` job in
the workflow refuses to build while that field is empty, rather than publish a release nobody can
install as an update.

The private half and its password are the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. `RELEASING.md` is where the rest is written
down and the only place it is: the two live in the repository owner's password manager, neither has
ever been in this tree, and neither belongs in a run log. Nothing in the code can verify that, and
there is nowhere else in the tree it is recorded.

**Losing the private key means every already-installed copy can never be updated again.** There is no
way back: a copy already out there accepts only an update signed by the key whose public half it was
built with, so generating a fresh pair abandons everybody already running the app rather than
repairing anything.

One local consequence of a non-empty `pubkey`: `npm run tauri build` refuses with "A public key has
been found, but no private key", the bundler looking for something to sign the updater archive with.
That is the intended state — signing belongs to the runner that holds the secrets — and
`npm run tauri build -- --no-sign` is the way past it. `npm test`, `npm run build`, `cargo test` and
`npm run tauri dev` bundle nothing and are unaffected.

## One version, and a tag checked against it

**`version` in `src-tauri/tauri.conf.json` is the single source.** It is what `getVersion()` reports
inside the app and the number to quote in a bug report; `package.json` and `src-tauri/Cargo.toml`
carry their own and mean nothing.

`scripts/release.mjs` raises that one field, runs the three gates, commits, pushes `main` and tags
`v${next}`. `.github/workflows/release.yml` triggers on `tags: - 'v*'` and its `check` job strips the
same prefix (`${GITHUB_REF_NAME#v}`) and compares — otherwise `latest.json` would announce a version
the bundle does not carry, and every installed copy would try to update to it for ever. The two sides
fail unequally and the script's side is the quiet one: change the prefix there and the push matches
no trigger, the script prints its success line and nothing builds. `npm test` cannot see the workflow
and `cargo test` cannot see the script.

`bundle.createUpdaterArtifacts` and the action's `uploadUpdaterJson` are what put the `.app.tar.gz`,
its `.sig` and `latest.json` on the release. `plugins.updater.endpoints` points at
`https://github.com/invisor/smetana/releases/latest/download/latest.json`, which is why
`releaseDraft` and `prerelease` are both false: that URL resolves against the latest **published**
release only.

## The Developer ID, and what an update would otherwise throw away

The bundle is signed with a Developer ID Application certificate and notarized. The visible half of
that is the first launch — a downloaded copy opens on a double-click, so there are no Gatekeeper
steps left to write down anywhere, and README's `## First launch` section and the `releaseBody`
paragraph that were the two copies of them are both gone.

**The half that belongs in this file is what it does to an update.** A macOS permission is stored
against a code requirement rather than against a bundle identifier alone. An ad-hoc signature's
requirement is a cdhash — a different value for every build — so an update installed in place
invalidated every folder grant the person had given, and macOS, holding a stored decision, never
asked again. It is silent on screen: the log says `Failed to match existing code requirement for
subject com.invisor.smetana` and the folder simply cannot be read. A Developer ID's requirement is
the team, which does not change between builds, so the grant outlives the release. That is
smetana-fkt, and it is the reason this section is here rather than in `RELEASING.md` alone: the
updater is what made the ad-hoc signature expensive, because installing in place is exactly the
motion that broke the grant.

Two consequences worth keeping. `bundle.macOS.signingIdentity` is still `"-"` in
`src-tauri/tauri.conf.json` and is overridden on the runner by `APPLE_SIGNING_IDENTITY`; the field
stays because **without some identity in it the bundler never calls `codesign` at all**, and the
release then carries whatever the linker left on the arm64 executable — `adhoc, linker-signed`,
`Info.plist` unbound, `Sealed Resources=none`, which Gatekeeper reads as a *broken* signature rather
than an unknown developer, so a quarantined copy opens to "smetana is damaged and can't be opened"
with no way in but `xattr`. v0.1.1 shipped that way. And the failure is quiet in the other direction
too: given no notarization credentials the bundler logs `skipping app notarization` and finishes
green. Both are why the workflow checks the six secrets before it builds and re-reads the built
bundle with `codesign -dv`, `spctl` and `stapler validate` afterwards. `RELEASING.md` holds the
certificate, the key and the secrets.

**Releases up to and including v0.1.23 were ad-hoc signed**, so a grant given to one of those is
already bound to a cdhash and is lost on the next update whatever this change does. The repair for
one that has been lost is the app's own reset — `.claude/rules/tracker.md` and
`src-tauri/src/tracker/access.rs` — which is the mitigation and not a replacement.

One claim written before there was an updater is still standing: the `releaseBody`'s "There is no
in-app updater yet". There were four. README's `## First launch` and `## Releases` went when
smetana-t3y rewrote that document, and the build step's "Nothing verifies it yet — the app has no
updater" went with the comment this change rewrote — restating a sentence known to be false was not
available while authoring its replacement. The `releaseBody` survivor is smetana-j98's to settle, not
this file's; do not re-sync it by putting the claim back into README.

## The front end, in one paragraph

The wire vocabulary is a pair that has to move together: the six state tags and their fields, the
five `UpdateError` kinds under `{kind, detail}`, the event name and the three command names, against
`KINDS` and `installRefusal` in `components/settings/update.js`, the command names in
`stores/updates.js`, and `mockBackend.js`. A renamed state tag does not draw the wrong thing — it
becomes `update.js`'s seventh kind, `unavailable`, which is also what a browser's `null` comes to, so
About draws **nothing about updates at all**, indistinguishable on screen from running with no back
end. A renamed `UpdateError` kind is quieter still: it falls through to the generic "The update could
not be installed." and throws away the projects the run gate named, which is the one thing that
refusal exists to say. `.claude/rules/settings.md` holds what About draws from it, and
`.claude/rules/notifications.md` the bell's half.

## What is tested

`cargo test` covers the pure halves whole — `Machine`'s transitions and its guards, `gate`, the
framing of somebody else's error, and two tests that pin the JSON both types travel as
(`the_state_travels_tagged_and_whole`, `a_refusal_travels_as_a_kind_and_a_detail`) — plus
`the_first_check_waits_and_the_next_one_is_a_day_later` over the two schedule constants. `npm test`
covers `tests/stores/updates.test.js` and `tests/components/settings/update.test.js`. Nothing tests
the workflow, the signature or an actual install, and nothing can: the first release is the test.
