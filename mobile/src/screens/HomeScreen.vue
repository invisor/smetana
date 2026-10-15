<script setup>
/* The placeholder for the working screens, which are the next stage's: the name
   of the project open on the desktop and how many tasks it holds. */
import { computed } from 'vue'
import Frame from './Frame.vue'
import Button from '@src/components/core/Button.vue'
import { dismissRecommended } from '../bridge/index.js'
import { openStore } from '../open.js'

const props = defineProps({
  project: { type: String, default: null },
  tasks: { type: Number, default: null },
  banner: { type: Boolean, default: false },
  storeUrl: { type: String, default: '' }
})

const bannerStyle = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 'var(--space-3)',
  padding: 'var(--space-3) var(--space-4)',
  background: 'var(--surface-raised)',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-2)',
  fontSize: 'var(--text-sm)',
  color: 'var(--text-secondary)'
}))
const labelStyle = computed(() => ({
  margin: 0,
  color: 'var(--text-muted)',
  fontSize: 'var(--text-xs)'
}))
const nameStyle = computed(() => ({
  margin: 0,
  fontFamily: 'var(--font-mono)',
  fontSize: 'var(--text-lg)'
}))
const countStyle = computed(() => ({ margin: 0, fontSize: 'var(--text-2xl)' }))
</script>

<template>
  <Frame>
    <div v-if="props.banner" :style="bannerStyle">
      <span>A newer version of the app is available.</span>
      <span>
        <Button size="sm" variant="ghost" @click="openStore(props.storeUrl)">Update</Button>
        <Button size="sm" variant="ghost" @click="dismissRecommended">Dismiss</Button>
      </span>
    </div>
    <p :style="labelStyle">Project</p>
    <p :style="nameStyle">{{ props.project ?? 'No project is open on the desktop' }}</p>
    <p :style="labelStyle">Tasks</p>
    <p :style="countStyle">{{ props.tasks ?? '…' }}</p>
  </Frame>
</template>
