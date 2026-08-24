<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Input from '../core/Input.vue'

const props = defineProps({
  streamState: { type: String, default: 'streaming' },
  follow: { type: Boolean, default: true },
  query: { type: String, default: '' },
  matches: { type: Number, default: null }
})

const emit = defineEmits(['toggle-follow', 'toggle-stream', 'update:query'])

const label = computed(() =>
  props.streamState === 'streaming' ? 'Streaming' : props.streamState === 'paused' ? 'Paused' : 'Ended'
)
const tone = computed(() => (props.streamState === 'streaming' ? 'var(--attn-live)' : 'var(--text-muted)'))

const barStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  height: 'var(--scope-bar-h)',
  flex: '0 0 auto',
  padding: '0 var(--space-3) 0 var(--space-5)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  background: 'var(--surface)'
}

const stateStyle = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  color: tone.value,
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase'
}))

/* "still running" pulse — the one place motion is allowed to loop */
const dotStyle = computed(() => ({
  width: '6px',
  height: '6px',
  borderRadius: '50%',
  background: 'currentColor',
  animation: props.streamState === 'streaming' ? 'sm-pulse var(--dur-pulse) var(--ease-in-out) infinite' : undefined
}))
</script>

<template>
  <div :style="barStyle">
    <span :style="stateStyle">
      <span :style="dotStyle" />
      {{ label }}
    </span>
    <!-- The search field is capped rather than fixed: `max-width` and not
         `width: 180px`. At a fixed width it cannot give anything back, so the
         follow-tail button at the end of the bar is clipped off as the
         inspector panel narrows — and that panel ships at 340px and drags down
         to 240 before it folds (`RIGHT_DEFAULT` and `RIGHT_MIN` in
         `src/views/panelWidths.js`). -->
    <Input
      size="sm"
      mono
      :model-value="query"
      placeholder="Search output"
      :style="{ flex: '1 1 auto', minWidth: 0, maxWidth: '180px', marginLeft: 'var(--space-4)' }"
      @update:model-value="emit('update:query', $event)"
    >
      <template #prefix><Icon name="search" :size="12" /></template>
    </Input>
    <span
      v-if="matches != null && query"
      :style="{ font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)', color: 'var(--text-muted)' }"
    >
      {{ matches }} matches
    </span>
    <span :style="{ flex: 1 }" />
    <IconButton
      :icon="streamState === 'streaming' ? 'pause' : 'play'"
      :label="streamState === 'streaming' ? 'Pause stream' : 'Resume stream'"
      size="sm"
      @click="emit('toggle-stream')"
    />
    <IconButton
      icon="arrow-down-to-line"
      label="Follow tail"
      size="sm"
      :selected="follow"
      @click="emit('toggle-follow')"
    />
  </div>
</template>
