<script setup>
/* The Editor tab: the code editor in the centre column, and nothing else. Its
   size is deliberately not the app's — chrome and code are two questions, and
   the app-wide size on the General tab leaves this one where it was. */
import Select from '../core/Select.vue'
import SettingsRow from './SettingsRow.vue'
import { FONT_SIZES } from '../../appearance.js'

const props = defineProps({
  fontSize: { type: Number, default: 12 }
})

const emit = defineEmits(['update:fontSize'])

const sizeOptions = FONT_SIZES.map((size) => ({ value: size, label: `${size} px` }))
</script>

<template>
  <div>
    <SettingsRow
      label="Editor font size"
      description="Applies to the code editor only, not to the rest of the app."
    >
      <Select
        :model-value="props.fontSize"
        :options="sizeOptions"
        @update:model-value="emit('update:fontSize', Number($event))"
      />
    </SettingsRow>
  </div>
</template>
