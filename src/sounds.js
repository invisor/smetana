/* What a notification sound is: the four the app ships, the "no sound" value,
   and the two questions anybody asks about one — is this a sound at all, and
   what does this stored string come to.

   Pure — no Vue, no DOM, no Tauri — which is what makes it the part of this
   feature a test can reach. The half that actually makes a noise is `chime.js`
   beside it, the same split `appearance.js` and `views/useAppearance.js` make.

   It sits at the top of `src/` rather than under the part of the interface it
   is a rule about, for the reason `paths.js` gives about itself: two windows
   want it at once — the settings window draws the list, the app window plays
   what it chose — so there is no "under" to put it in.

   The ids are written out here and again as `SOUNDS` in
   `src-tauri/src/settings/model.rs`. What crosses the IPC boundary is validated
   there; this copy is what the dropdown draws. A sound added to one and not the
   other is the familiar silent failure — offered here, and quietly becoming
   something else on the way to disk. */

/* Chosen, and choosing silence. A value rather than an absence: a missing field
   is a file written before this existed, and it takes the shipped default. */
export const SOUND_OFF = 'off'

/* The shipped four, in the order they are drawn. The names are the file names
   under `assets/sounds/` and the strings `settings.json` holds; the labels
   below are separate, so renaming what a person reads never rewrites anybody's
   stored choice. `assets/sounds/README.md` records where each file came from
   and why the labels say nothing about how they sound. */
export const SOUND_IDS = ['sound-1', 'sound-2', 'sound-3', 'sound-4']

export const SOUND_CHOICES = [
  { value: SOUND_OFF, label: 'Off' },
  ...SOUND_IDS.map((id, index) => ({ value: id, label: `Sound ${index + 1}` }))
]

/* Shipped on, and with two different sounds. On, because a feature that does
   nothing until somebody finds a switch is a feature nobody finds — the
   argument `git.autoFetch` is on by default for, and it applies more here,
   since the person who benefits is by definition not looking at the screen the
   switch lives on. Different, because a run that ended can be read in the
   morning while an agent that is waiting is a night that has stopped moving,
   and one file for both would leave that difference in a settings screen nobody
   opened. Mirrored by `NotificationSettings::default()` in Rust: the two copies
   have to agree, or the dropdown draws a sound the app is not playing. */
export const NOTIFICATION_DEFAULTS = {
  runFinished: 'sound-1',
  needsAttention: 'sound-2'
}

export const isSound = (value) => value === SOUND_OFF || SOUND_IDS.includes(value)

/* Whether a chime asked for right now actually makes a noise. The whole of the
   rule, which is why it is here rather than inside `chime.js`: a `.vue` file
   and a module that touches the DOM are the two things no test in this
   repository can reach, and this is the part worth pinning.

   Focus arrives as an argument rather than being asked for. `document.hasFocus`
   is `chime.js`'s to ask — this file has no DOM in it and is the reason the
   split exists at all. What that question means is settled there too: the app
   has two windows, and the one this is asked in is the main one, so a person
   working in an open settings window with the app behind it still hears the
   sound. Named as a consequence rather than discovered later.

   `off` is silence whatever the option says, and so is anything that is not a
   sound at all — the option can only ever take a noise away, never add one.

   The option itself ships **on**, and it is the one default in this feature
   that changes today's behaviour rather than preserving it. `showReport` and
   `window.restoreGeometry` ship on by the opposite argument — they keep what
   the app already did to the letter — and the argument here is the other one,
   said out loud: a sound exists for the person who is not looking at the
   screen, and a sound played at somebody who is looking is noise. The four
   copies of that default are `NotificationSettings::default()` in `model.rs`,
   `defaults()` in `stores/settings.js`, `view` in `SettingsWindow.vue` and the
   prop default in `settings/GeneralSettings.vue`. It is deliberately not in
   `NOTIFICATION_DEFAULTS` above, which is the closed list of sounds and the two
   shipped ones: a boolean about when to play one has no business in it, which
   is the argument `showReport` already carries one struct over. */
export function shouldPlay(id, onlyWhenUnfocused, focused) {
  if (!isSound(id) || id === SOUND_OFF) return false
  return !(onlyWhenUnfocused && focused)
}

/* A stored or announced value, made safe. The fallback is `off` rather than a
   sound: a value nobody can account for should not become a noise nobody
   chose. Callers that know better — the settings store, restoring a field an
   event failed to change — pass their own. */
export function normalizeSound(value, fallback = SOUND_OFF) {
  return isSound(value) ? value : fallback
}
