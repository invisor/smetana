<script setup>
import IconButton from '../core/IconButton.vue'
import { WINDOW_CONTROLS, controlIcon, controlLabel } from './windowChrome.js'

/* The three buttons a window draws when the system has stopped drawing them:
   Windows and Linux, where there is no `titleBarStyle` to overlay and the only
   way to reach the top of the window is to have taken the decorations off it.

   It reaches no window itself and knows about no platform. Both are the
   consumer's business — a component in this repository sees stores and props
   and nothing else — which is also what lets the gallery draw it. */
defineProps({
  /* Which of the two the middle button is right now. */
  maximized: { type: Boolean, default: false }
})

defineEmits(['minimize', 'toggle-maximize', 'close'])

const style = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-1)',
  flex: '0 0 auto'
}
</script>

<template>
  <span :style="style">
    <IconButton
      v-for="control in WINDOW_CONTROLS"
      :key="control.action"
      :icon="controlIcon(control, maximized)"
      size="sm"
      :label="controlLabel(control, maximized)"
      @click="$emit(control.action)"
    />
  </span>
</template>
