/* What the Caveman group offers, and what it says about how caveman stands on
   this machine. Nothing here about how any of it looks.

   The group is the **Skills & Plugins** tab's (`SkillsPluginsSettings.vue`),
   the tab immediately after Agents, and the line between the two is what is
   *installed* on this machine against how an agent talks: caveman is somebody
   else's software with a state and an install command, where the harness, the
   languages, the standing instruction and the run limits next door are all
   about what an agent says and what it spends. It stood on Agents until
   smetana-ekrl, on the older argument that a level is a way of talking.

   One of its lists is drawn somewhere else, and the file stays here anyway:
   `projectLevelOptions` is the project's own ladder, and its row is in the
   project settings window (`components/run/ProjectSettingsModal.vue`), off the
   project tile's right-click menu. It is imported across groups rather than
   copied, since a second list of the same rungs is exactly what this file
   exists to prevent — and it belongs with the rest of the caveman vocabulary
   rather than with the four scalars of a run configuration it happens to be
   drawn above.

   Another of the `subscription.js` / `usage.js` / `storage.js` family next door:
   the whole of one rule, pure, with no Vue and no DOM in it, out here because a
   `.vue` file is the one thing no test in this repository can reach. The group
   itself draws what these functions answer and decides nothing of its own.

   # The ladder is written out a third time, and this is the copy that owns it

   `CAVEMAN_LEVELS` in `src-tauri/src/settings/model.rs` is the first, and it is
   the authority: `CavemanSettings::validate` rewrites anything off it to `off`
   on the way to disk, silently, so a level only the front end knows is a choice
   that reverts at the next open with nothing on screen to say so. The
   obligation `SUBSCRIPTION_STEPS` and `SIDE_TABS` carry applies here word for
   word — **what this offers must stay a subset of what Rust accepts** — and
   `tests/components/settings/caveman.test.js` reads Rust's own array and pins
   it.

   It is doubled rather than fetched because each end needs it for a different
   job: Rust to refuse a hand-edited word, this end to have something for a list
   on screen to draw. `src/stores/settings.js` used to hold a third list of its
   own and now asks `isLevel`/`isProjectLevel` here, which is what keeps the
   guard on an incoming patch and the list on screen from ever disagreeing.

   # The four states are Rust's words, not ours

   `state` is one of `absent`, `binaries-only`, `wired` and `project-skill-only`
   — `CavemanState` in `src-tauri/src/caveman.rs`, spelled in kebab case by
   serde and read here by those spellings. A word this build has never heard of
   is an ordinary outcome and not an error: it says so and offers nothing, which
   is the same answer a reading that has not arrived yet gets.

   # The Install button installs nothing

   It opens a terminal in the active project and leaves the command in it,
   unrun. That is a decision rather than a shortcut: caveman's own installer
   rewrites `~/.claude/settings.json` and `~/.claude.json` and points the
   agent's traffic at a local proxy, and this app has no business doing any of
   that behind somebody's back. Typed rather than run, the person reads the
   exact command, presses Enter themselves, and sees the output and any failure
   live in a terminal they already know.

   The commands are caveman's own documentation and not an invention here, and
   they come from two places rather than one. `npm i -g @caveman-ai/cli` and
   `caveman setup --install` are the CLI's README, under "Getting the binaries".
   `caveman enable claude` is **not** in that README: it is what the CLI itself
   prints as its `next native:` remediation, in that order after
   `setup --install`, and it is the verb that writes the hook and the journal
   `caveman.rs` reads back — so what this button offers and what the line above
   it can see are the same act. The README's own `setup --agent-native claude`
   is a strict superset of it, installing a skills suite, cloud MCP servers and
   Core besides, and is deliberately not what is typed: the whole point of a
   command put in front of somebody is the smallest footprint they can read
   before pressing Enter. None of it is a network download piped into a shell —
   `curl … | bash` would hand a shell whatever the far end sends today, which is
   not a thing to type on somebody's behalf. */

/* The rungs, `off` first, exactly `CAVEMAN_LEVELS` in `settings/model.rs`. */
export const CAVEMAN_LEVELS = [
  'off',
  'lite',
  'full',
  'ultra',
  'wenyan-lite',
  'wenyan-full',
  'wenyan-ultra'
]

/* The one word a project's own level has and the global one does not: "as in
   every other project". A word and never a `null`, because `adopt()` in
   `views/SettingsWindow.vue` skips a field that arrives null and the previous
   project's level would go on standing on screen. */
export const CAVEMAN_INHERIT = 'inherit'

/* Labels for ids caveman already knows, the way the agent and language lists
   one tab over are labels for ids Rust knows. Sentence case, like every other word
   on this screen; wenyan is caveman's own name for the classical Chinese
   register and stays as it spells it. */
const LEVEL_LABEL = {
  off: 'Off',
  lite: 'Lite',
  full: 'Full',
  ultra: 'Ultra',
  'wenyan-lite': 'Wenyan lite',
  'wenyan-full': 'Wenyan full',
  'wenyan-ultra': 'Wenyan ultra'
}

/* What `inherit` reads as in a list. It names the setting it defers to — All
   projects, in the Caveman group on the settings window's Skills & Plugins
   tab — rather than saying
   "default", because what it inherits is a choice somebody made on a screen of
   this app and not a shipped value. The two rows are in two windows now, which
   is what makes naming the other one worth more than it cost when they were
   one above the other. */
const INHERIT_LABEL = 'Same as all projects'

/* The seven the global row offers. Built per call rather than frozen once: the
   list is small, and a shared array handed to a `Dropdown` is a thing a caller
   could sort in place. */
export function levelOptions() {
  return CAVEMAN_LEVELS.map((value) => ({ value, label: LEVEL_LABEL[value] ?? value }))
}

/* The eight the project row offers: the override first, then the same seven.
   First rather than last because it is the default and the commonest answer,
   and because a person reading down the list should meet "same as all projects"
   before they meet a level to depart to. Drawn in the project settings window
   rather than on the tab this file's own group is on — see the header. */
export function projectLevelOptions() {
  return [{ value: CAVEMAN_INHERIT, label: INHERIT_LABEL }, ...levelOptions()]
}

/* The guards `applyPatch` in `stores/settings.js` uses. A string and on the
   ladder — anything else is dropped and the previous value stands, since a
   patch arrives as an event and an event is not a response to anything. */
export function isLevel(value) {
  return typeof value === 'string' && CAVEMAN_LEVELS.includes(value)
}

export function isProjectLevel(value) {
  return value === CAVEMAN_INHERIT || isLevel(value)
}

/* One sentence per state, and one for having nothing to say yet. Read by state
   name rather than by a chain of booleans, so a fifth state added in Rust turns
   up here as the fallback line instead of as a wrong claim. */
const STATE_SENTENCE = {
  absent:
    'Not installed on this machine. Caveman is somebody else\'s layer between an agent and its provider: it shortens what the agent reads and writes, so a night of work costs fewer tokens.',
  'binaries-only':
    'Installed, and nothing in Claude Code calls it. It is switched off until it is wired in, and until then the level below reaches nobody.',
  wired:
    'Wired into Claude Code and working. It shortens every session on this machine, not only the ones started here.',
  'project-skill-only':
    'Only the skill in this project: one rule file in the repository, with no proxy and no hooks anywhere.'
}

const UNKNOWN_SENTENCE = 'There is no saying yet how caveman stands on this machine.'

/* `reading` is `caveman_state`'s answer whole, in Rust's own shape, or `null`
   before there has been one. */
export function stateSentence(reading) {
  return STATE_SENTENCE[reading?.state] ?? UNKNOWN_SENTENCE
}

/* What the journal says, for the one state it can be said about.
   `caveman.rs` reads the journal whatever the state is, so a machine that was
   wired last week and has since been unwired still has one — and repeating its
   file list under "installed and switched off" would claim those files are
   still rewritten when they may have been put back. So the facts are drawn for
   `wired` alone, where they describe what is true right now.

   The replaced files are here because they are the one thing this app can say
   that nothing else says: another installer rewrote two of somebody's own
   configuration files, and they are entitled to see which. Each is its own
   entry under the same name rather than one line of joined paths — a path is an
   identifier, drawn in mono, and two of them on one line is a string nobody can
   read. */
export function stateFacts(reading) {
  if (reading?.state !== 'wired') return []
  const facts = []
  if (reading.packVersion) facts.push({ name: 'Pack version', value: reading.packVersion })
  if (reading.detectedAgentVersion) {
    facts.push({ name: 'Applied to', value: reading.detectedAgentVersion })
  }
  for (const file of reading.replacedFiles ?? []) {
    if (file) facts.push({ name: 'Replaced', value: file })
  }
  return facts
}

/* The two states with something to offer: nothing installed, and installed with
   nothing calling it. `wired` has nothing to do and `project-skill-only` is a
   form of caveman that lives in the repository rather than on the machine, so
   neither draws the button at all — and neither does a reading that has not
   arrived, since a button offered and taken away again is worse than one that
   appears when there is something to press it for. */
export function offersInstall(reading) {
  return installCommand(reading) !== null
}

/* What is typed into the terminal, whole, and never sent: the newline is the
   person's. See the header for where each command comes from.

   `claude` is named in it rather than left for caveman to detect, and that is
   the honest half of a limit this app already has: the Agent row one tab over
   offers Codex and cannot select it, and `caveman.rs` reads Claude Code's own hooks
   file and nobody else's. A command that said `--detected` could wire an agent
   this screen would then be unable to say anything about. */
export function installCommand(reading) {
  if (reading?.state === 'absent') {
    return 'npm i -g @caveman-ai/cli && caveman setup --install && caveman enable claude'
  }
  if (reading?.state === 'binaries-only') return 'caveman enable claude'
  return null
}

/* The line under the Install label, and the one place this screen says out loud
   that the button types rather than runs.

   With no project open there is nothing to open a terminal in, so the button is
   drawn `disabled` and this sentence names the reason — the shape the Launch at
   login row of the General tab already has, where a control that refuses a
   press without saying why is held to be worse than one that is not there. */
export function installDescription(reading, projectOpen) {
  if (!projectOpen) {
    return 'No project is open, so there is no terminal to type this into. Open a project and come back.'
  }
  if (reading?.state === 'binaries-only') {
    return 'The binaries are already here; this wires them into Claude Code. It opens a terminal in the active project and types the command in — nothing runs until you press Enter.'
  }
  return 'Opens a terminal in the active project and types the command in. Nothing runs until you press Enter, and this app changes no configuration of its own.'
}
