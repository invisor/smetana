<script setup>
/* The General tab: how the app looks everywhere, what it says when something
   finishes, and how the app starts. None of it is about any single part of the
   app, which is what makes this the tab these rows are on — the two sounds
   included, since a noise is a fact about a person and a room rather than about
   anything on the screen; the report switch above them, since whether a person
   wants that document put in front of them is a habit of reading rather than a
   fact about one project; and the Startup pair because it is about the app as a
   whole rather than about anything inside it. What the app does to a person's
   repositories on its own is the Git tab, next door.

   Startup goes last on purpose. The top of this tab is the theme and the type
   size, which is what somebody opens this window for; a pair of switches set
   once, on the day the app is installed, has no claim on that space.

   Presentational, like every component here: it is handed the values and emits
   what a person picked. The window is what talks to the main window and to the
   file, so this renders in `?view=gallery` with nothing behind it. */
import { computed } from 'vue'
import Dropdown from '../core/Dropdown.vue'
import Switch from '../core/Switch.vue'
import SettingsGroup from './SettingsGroup.vue'
import SettingsRow from './SettingsRow.vue'
import { FONT_SIZES, THEME_CHOICES } from '../../appearance.js'
import { SOUND_CHOICES } from '../../sounds.js'
import { chime } from '../../chime.js'

const props = defineProps({
  theme: { type: String, default: 'dark' },
  uiFontSize: { type: Number, default: 13 },
  /* Whether this build may register a login item at all. False in a
     development build and in a browser, where the row is drawn disabled with a
     sentence saying so rather than hidden — a row that is not there reads as
     "not built yet" to the next person, and this page is what they would be
     reading it on. */
  autostartSupported: { type: Boolean, default: false },
  /* Whether there is a login item right now, as the operating system reports
     it — never a copy kept in a settings file, for the reason
     `src-tauri/src/autostart.rs` records. */
  autostartEnabled: { type: Boolean, default: false },
  /* Whether the main window opens where it was left. Shipped on, mirroring
     Rust and `stores/settings.js`: a component that defaulted to off would draw
     the opposite of what the app is doing for the moment before the first value
     arrives. */
  restoreGeometry: { type: Boolean, default: true },
  /* A sound id or `off`. The two shipped ones, mirroring `sounds.js` and Rust:
     a component that defaulted to silence would draw the opposite of what the
     app is doing for the moment before the first value arrives. */
  notificationRunFinished: { type: String, default: 'sound-1' },
  notificationNeedsAttention: { type: String, default: 'sound-2' },
  /* Whether the two sounds above wait until the main window is in the
     background. Shipped on, mirroring Rust, `stores/settings.js` and
     `SettingsWindow.vue`: a component that defaulted to off would draw the
     switch in the position opposite to what the app is doing for the moment
     before the first value arrives. */
  notificationOnlyWhenUnfocused: { type: Boolean, default: true },
  /* Whether a run that has ended opens its own account in a tab. Shipped on,
     mirroring Rust, `stores/settings.js` and `SettingsWindow.vue`: a component
     that defaulted to off would draw the switch in the position opposite to
     what the app is doing for the moment before the first value arrives. */
  notificationShowReport: { type: Boolean, default: true }
})

const emit = defineEmits([
  'update:theme',
  'update:uiFontSize',
  'update:autostartEnabled',
  'update:restoreGeometry',
  'update:notificationRunFinished',
  'update:notificationNeedsAttention',
  'update:notificationOnlyWhenUnfocused',
  'update:notificationShowReport'
])

/* The number goes out as a number, and that is `Dropdown` doing it rather than
   a coercion here: it hands back the option's own value untouched, where a
   native `<select>` would have stringified it. It matters because `clampFont`
   demands an integer outright — a "15" arriving where a 15 belongs is not
   read as fifteen, it silently takes the shipped size instead. */
const sizeOptions = FONT_SIZES.map((size) => ({ value: size, label: `${size} px` }))

/* Choosing a sound plays it: the choice is the preview, so there is no play
   button beside the list. A third control in the row would be the only action
   button on any settings row outside Storage, and somebody who wants to hear it
   again picks it again. It also means every press is a gesture, which is the
   condition a webview's autoplay policy asks for — this is the one place the
   sound is certain to be allowed.

   It plays whatever the switch under it says, and deliberately: `chime` is
   called with no second argument, so `unlessFocused` stays false. A preview is
   somebody listening to a choice rather than the app announcing something, and
   this window is focused by definition at the moment of the press — obeying the
   option would make the whole list silent in exactly the place a sound is
   picked, which is the option switched on for everybody by default. */
function pick(event, value) {
  emit(event, value)
  chime(value)
}

/* One line under the label, and which one depends on whether the switch can do
   anything at all. Naming the reason is the whole point of drawing the row
   disabled instead of leaving it out: a control that refuses a press without
   saying why is worse than one that is not there. */
const autostartDescription = computed(() =>
  props.autostartSupported
    ? 'Opens Smetana automatically when you sign in.'
    : 'Not available in a development build.'
)
</script>

<template>
  <div>
    <SettingsRow
      label="Theme"
      description="System follows the operating system and changes with it."
    >
      <Dropdown
        :model-value="props.theme"
        :options="THEME_CHOICES"
        @update:model-value="emit('update:theme', $event)"
      />
    </SettingsRow>
    <SettingsRow
      label="Interface font size"
      description="Scales the whole app, the terminal included."
    >
      <Dropdown
        :model-value="props.uiFontSize"
        :options="sizeOptions"
        @update:model-value="emit('update:uiFontSize', $event)"
      />
    </SettingsRow>
    <!-- Above the Notifications caption rather than inside it: a notification
         is what the app says while nobody is looking, and this is what it opens
         when they are — a document put on the screen in front of somebody, not
         a noise or a card waiting to be noticed. It sat under the caption once
         and read as a third kind of announcement. The description says what the
         switch does and does not promise more than it can keep — a run that fell
         over without writing a document still has nothing to open, and the bell
         keeps that case. -->
    <SettingsRow
      label="Show run report"
      description="Opens a run's report in a tab as soon as the run finishes. With it off, nothing opens and no card waits in the bell."
    >
      <Switch
        :model-value="props.notificationShowReport"
        @update:model-value="emit('update:notificationShowReport', $event)"
      />
    </SettingsRow>
    <!-- What the app says when something finishes while nobody is looking at
         the screen: two sounds, one for each of the two moments worth reaching
         somebody out of the room for. The bell in the scope bar is the third,
         and it has no row here because it is not a preference. -->
    <SettingsGroup label="Notifications">
      <SettingsRow
        label="Run finished"
        description="Played when a run reaches its ending, whether its report opens in a tab or waits in the bell."
      >
        <Dropdown
          :model-value="props.notificationRunFinished"
          :options="SOUND_CHOICES"
          @update:model-value="pick('update:notificationRunFinished', $event)"
        />
      </SettingsRow>
      <SettingsRow
        label="Agent needs you"
        description="Played when an agent stops to ask something, in any open project."
      >
        <Dropdown
          :model-value="props.notificationNeedsAttention"
          :options="SOUND_CHOICES"
          @update:model-value="pick('update:notificationNeedsAttention', $event)"
        />
      </SettingsRow>
      <!-- Inside the group and under both lists, which is what says it is about
           them: this is one condition over the two sounds above rather than a
           third announcement. That is the opposite placing to Show run report,
           which sits above the caption because it is about a document. Shipped
           on: a sound is for the person who is not at the screen, and one
           played at somebody who is looking is noise. -->
      <SettingsRow
        label="Only when the main window is not focused"
        description="Plays the sounds above only while the main window is in the background. With it off, they play whether or not you are looking."
      >
        <Switch
          :model-value="props.notificationOnlyWhenUnfocused"
          @update:model-value="emit('update:notificationOnlyWhenUnfocused', $event)"
        />
      </SettingsRow>
    </SettingsGroup>
    <!-- Last on the tab, and the two halves of "how does this app come up":
         whether the operating system opens it, and where its window lands. -->
    <SettingsGroup label="Startup">
      <SettingsRow
        label="Launch at login"
        :description="autostartDescription"
      >
        <Switch
          :model-value="props.autostartEnabled"
          :disabled="!props.autostartSupported"
          @update:model-value="emit('update:autostartEnabled', $event)"
        />
      </SettingsRow>
      <!-- "Applies on the next launch" is the truth rather than a hedge: the
           decision is taken once, while the window is being shown. Turning this
           off stops the window being put back and never stops its place being
           remembered, so turning it on again returns it to where it was left. -->
      <SettingsRow
        label="Restore window position & size"
        description="Reopens the main window where you left it. Applies on the next launch."
      >
        <Switch
          :model-value="props.restoreGeometry"
          @update:model-value="emit('update:restoreGeometry', $event)"
        />
      </SettingsRow>
    </SettingsGroup>
  </div>
</template>
