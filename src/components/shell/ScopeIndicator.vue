<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'

/* The scope bar answers "where am I working" before anything else on screen:
   repo / worktree @ branch, then the two live counters. */
const props = defineProps({
  repo: { type: String, required: true },
  worktree: { type: String, default: '' },
  branch: { type: String, default: '' },
  dirtyCount: { type: Number, default: 0 },
  agentsActive: { type: Number, default: 0 },
  notifications: { type: Number, default: 0 }
})

defineEmits(['notifications', 'settings'])

const barStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  height: 'var(--scope-bar-h)',
  flex: '0 0 auto',
  padding: '0 var(--space-5)',
  background: 'var(--scope-bar)',
  borderBottom: 'var(--border-w) solid var(--border)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}

const segStyle = (strong) => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  minWidth: 0,
  color: strong ? 'var(--text-primary)' : 'var(--text-secondary)',
  fontWeight: strong ? 'var(--weight-medium)' : 'var(--weight-regular)'
})
const truncate = { whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }
const counter = (color) => ({ display: 'inline-flex', alignItems: 'center', gap: 'var(--space-2)', color })

const scopeName = computed(() => props.worktree || props.branch)
const badgeStyle = {
  position: 'absolute', top: '1px', right: '1px', minWidth: '12px', height: '12px', padding: '0 3px',
  display: 'flex', alignItems: 'center', justifyContent: 'center',
  background: 'var(--attn-loud)', color: 'var(--attn-loud-contrast)',
  border: 'var(--border-w) solid var(--scope-bar)', borderRadius: 'var(--radius-pill)',
  font: 'var(--weight-semibold) var(--text-2xs)/1 var(--font-mono)'
}
</script>

<template>
  <div :style="barStyle">
    <span :style="segStyle(true)">
      <Icon name="folder-git-2" :size="12" :style="{ color: 'var(--text-muted)' }" />
      <span :style="truncate">{{ repo }}</span>
    </span>
    <span :style="{ color: 'var(--border-strong)' }">/</span>
    <span :style="segStyle(true)">
      <Icon name="git-branch" :size="12" :style="{ color: 'var(--text-muted)' }" />
      <span :style="truncate">{{ scopeName }}</span>
    </span>
    <span v-if="branch && worktree" :style="{ color: 'var(--text-muted)' }">@{{ branch }}</span>

    <span v-if="dirtyCount > 0" :title="`${dirtyCount} uncommitted files`" :style="counter('var(--git-modified)')">
      <Icon name="file-pen" :size="12" />{{ dirtyCount }}
    </span>
    <span v-if="agentsActive > 0" :title="`${agentsActive} agents running`" :style="counter('var(--attn-live)')">
      <Icon name="bot" :size="12" />{{ agentsActive }}
    </span>

    <span :style="{ flex: 1 }" />

    <span :style="{ position: 'relative', display: 'inline-flex' }">
      <IconButton
        icon="bell"
        size="sm"
        :label="notifications > 0 ? `${notifications} unread notifications` : 'Notifications'"
        @click="$emit('notifications')"
      />
      <span v-if="notifications > 0" aria-hidden="true" :style="badgeStyle">{{ notifications }}</span>
    </span>
    <IconButton icon="settings" size="sm" label="Settings" @click="$emit('settings')" />
  </div>
</template>
