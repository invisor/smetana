<script setup>
/* The General tab: how the app looks everywhere. Two settings, both about the
   whole window rather than about any one part of it — which is what makes this
   the tab they are on.

   Presentational, like every component here: it is handed the values and emits
   what a person picked. The window is what talks to the main window and to the
   file, so this renders in `?view=gallery` with nothing behind it. */
import Select from '../core/Select.vue'
import SettingsRow from './SettingsRow.vue'
import { FONT_SIZES, THEME_CHOICES } from '../../appearance.js'

const props = defineProps({
  theme: { type: String, default: 'dark' },
  uiFontSize: { type: Number, default: 13 }
})

const emit = defineEmits(['update:theme', 'update:uiFontSize'])

/* The number goes out as a number: a `<select>` hands back strings, and a "15"
   stored where a 15 belongs would fail its range check on the next read and
   silently take the default. */
const sizeOptions = FONT_SIZES.map((size) => ({ value: size, label: `${size} px` }))
</script>

<template>
  <div>
    <SettingsRow
      label="Theme"
      description="System follows the operating system and changes with it."
    >
      <Select
        :model-value="props.theme"
        :options="THEME_CHOICES"
        @update:model-value="emit('update:theme', $event)"
      />
    </SettingsRow>
    <SettingsRow
      label="Interface font size"
      description="Scales the whole app, the terminal included."
    >
      <Select
        :model-value="props.uiFontSize"
        :options="sizeOptions"
        @update:model-value="emit('update:uiFontSize', Number($event))"
      />
    </SettingsRow>
  </div>
</template>
