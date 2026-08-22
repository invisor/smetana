<script setup>
/* The General tab: how the app looks everywhere, and the noise it makes. None
   of it is about any single part of the app, which is what makes this the tab
   these rows are on — the two sounds included, since a noise is a fact about a
   person and a room rather than about anything on the screen. What the app does
   to a person's repositories on its own is the Git tab, next door.

   Presentational, like every component here: it is handed the values and emits
   what a person picked. The window is what talks to the main window and to the
   file, so this renders in `?view=gallery` with nothing behind it. */
import Dropdown from '../core/Dropdown.vue'
import SettingsRow from './SettingsRow.vue'
import { FONT_SIZES, THEME_CHOICES } from '../../appearance.js'
import { SOUND_CHOICES } from '../../sounds.js'
import { chime } from '../../chime.js'

const props = defineProps({
  theme: { type: String, default: 'dark' },
  uiFontSize: { type: Number, default: 13 },
  /* A sound id or `off`. The two shipped ones, mirroring `sounds.js` and Rust:
     a component that defaulted to silence would draw the opposite of what the
     app is doing for the moment before the first value arrives. */
  notificationRunFinished: { type: String, default: 'sound-1' },
  notificationNeedsAttention: { type: String, default: 'sound-2' }
})

const emit = defineEmits([
  'update:theme',
  'update:uiFontSize',
  'update:notificationRunFinished',
  'update:notificationNeedsAttention'
])

/* The number goes out as a number, and that is `Dropdown` doing it rather than
   a coercion here: it hands back the option's own value untouched, where a
   native `<select>` would have stringified it. It matters because `clampFont`
   demands an integer outright — a "15" arriving where a 15 belongs is not
   read as fifteen, it silently takes the shipped size instead. */
const sizeOptions = FONT_SIZES.map((size) => ({ value: size, label: `${size} px` }))

/* The caption over a group of rows, the shape `KanbanSettings.vue` already
   uses for its two. */
const captionStyle = {
  display: 'block',
  marginTop: 'var(--space-5)',
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}

/* Choosing a sound plays it: the choice is the preview, so there is no play
   button beside the list. A third control in the row would be the only action
   button on any settings row outside Storage, and somebody who wants to hear it
   again picks it again. It also means every press is a gesture, which is the
   condition a webview's autoplay policy asks for — this is the one place the
   sound is certain to be allowed. */
function pick(event, value) {
  emit(event, value)
  chime(value)
}
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
    <!-- The two things worth hearing rather than seeing, and both happen while
         nobody is looking at the screen. The bell in the scope bar is the
         visual half; this is the other one. -->
    <span :style="captionStyle">Notifications</span>
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
  </div>
</template>
