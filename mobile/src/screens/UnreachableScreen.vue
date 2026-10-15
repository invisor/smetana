<script setup>
import { computed } from 'vue'
import Frame from './Frame.vue'
import EmptyState from '@src/components/core/EmptyState.vue'
import Button from '@src/components/core/Button.vue'
import { connect, lastSeenText } from '../bridge/index.js'

const props = defineProps({
  lastSeen: { type: Number, default: null },
  project: { type: String, default: null },
  tasks: { type: Number, default: null }
})

const text = computed(() => lastSeenText(props.lastSeen))
const copyStyle = computed(() => ({
  margin: 0,
  textAlign: 'center',
  color: 'var(--text-muted)',
  fontSize: 'var(--text-sm)'
}))
</script>

<template>
  <Frame>
    <EmptyState icon="triangle-alert" title="Workstation unreachable" :description="text" />
    <p v-if="props.tasks !== null" :style="copyStyle">
      Read only: {{ props.project ?? 'the last project' }}, {{ props.tasks }} tasks when last read.
    </p>
    <Button icon="refresh-cw" full-width @click="connect">Try again</Button>
  </Frame>
</template>
