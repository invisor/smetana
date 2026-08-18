<script setup>
/* The General tab: how the app looks everywhere, and the one thing it does on
   its own. Three settings, none of them about any single part of the app —
   which is what makes this the tab they are on. The background fetch is here
   for that reason rather than under a Git tab there is none of: what it settles
   is whether this machine opens a socket by itself, which is a fact about a
   connection and a person and not about a repository.

   Presentational, like every component here: it is handed the values and emits
   what a person picked. The window is what talks to the main window and to the
   file, so this renders in `?view=gallery` with nothing behind it. */
import Dropdown from '../core/Dropdown.vue'
import Switch from '../core/Switch.vue'
import SettingsRow from './SettingsRow.vue'
import { FONT_SIZES, THEME_CHOICES } from '../../appearance.js'

const props = defineProps({
  theme: { type: String, default: 'dark' },
  uiFontSize: { type: Number, default: 13 },
  gitAutoFetch: { type: Boolean, default: true }
})

const emit = defineEmits(['update:theme', 'update:uiFontSize', 'update:gitAutoFetch'])

/* The number goes out as a number, and that is `Dropdown` doing it rather than
   a coercion here: it hands back the option's own value untouched, where a
   native `<select>` would have stringified it. It matters because `clampFont`
   demands an integer outright — a "15" arriving where a 15 belongs is not
   read as fifteen, it silently takes the shipped size instead. */
const sizeOptions = FONT_SIZES.map((size) => ({ value: size, label: `${size} px` }))
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
    <!-- Off is for a metered connection, a VPN that is not always up, or a key
         with a passphrase that would fail on every sweep. The sentence says
         what it does not do as well: nothing about this changes a file. -->
    <SettingsRow
      label="Fetch from remotes automatically"
      description="Checks the Git panel's branches for new commits when the window comes back into focus, at most every few minutes. Nothing is merged."
    >
      <Switch
        :model-value="props.gitAutoFetch"
        @update:model-value="emit('update:gitAutoFetch', $event)"
      />
    </SettingsRow>
  </div>
</template>
