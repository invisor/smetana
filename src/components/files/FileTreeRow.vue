<script setup>
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'

const GIT = {
  modified: { c: 'var(--git-modified)', l: 'M' },
  added: { c: 'var(--git-added)', l: 'A' },
  deleted: { c: 'var(--git-deleted)', l: 'D' },
  untracked: { c: 'var(--git-untracked)', l: 'U' },
  conflict: { c: 'var(--git-conflict)', l: '!' },
  ignored: { c: 'var(--git-ignored)', l: '' }
}

const props = defineProps({
  name: { type: String, required: true },
  depth: { type: Number, default: 0 },
  kind: { type: String, default: 'file' },
  expanded: { type: Boolean, default: false },
  selected: { type: Boolean, default: false },
  git: { type: String, default: undefined },
  readOnly: { type: Boolean, default: false }
})

const emit = defineEmits(['toggle', 'select'])

const hover = ref(false)
const g = computed(() => (props.git ? GIT[props.git] : null))

const style = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  paddingLeft: `calc(var(--space-4) + ${props.depth} * var(--tree-indent))`,
  paddingRight: 'var(--space-4)',
  background: props.selected ? 'var(--surface-selected)' : hover.value ? 'var(--surface-hover)' : 'transparent',
  color: props.git === 'ignored' ? 'var(--text-muted)' : 'var(--text-primary)',
  opacity: props.git === 'ignored' ? 0.7 : 1,
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  cursor: 'default',
  transition: 'var(--transition-control)'
}))

const nameStyle = computed(() => ({
  flex: 1,
  minWidth: 0,
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  textDecoration: props.git === 'deleted' ? 'line-through' : undefined
}))

const onClick = () => emit(props.kind === 'dir' ? 'toggle' : 'select')
</script>

<template>
  <div
    role="treeitem"
    :aria-expanded="kind === 'dir' ? expanded : undefined"
    :aria-selected="selected"
    :tabindex="selected ? 0 : -1"
    :style="style"
    @mouseenter="hover = true"
    @mouseleave="hover = false"
    @click="onClick"
  >
    <span :style="{ width: '12px', display: 'flex', color: 'var(--text-muted)' }">
      <Icon v-if="kind === 'dir'" :name="expanded ? 'chevron-down' : 'chevron-right'" :size="12" />
    </span>
    <Icon
      :name="kind === 'dir' ? (expanded ? 'folder-open' : 'folder') : 'file'"
      :size="13"
      :style="{ color: 'var(--text-muted)' }"
    />
    <span :style="nameStyle">{{ name }}</span>
    <Icon
      v-if="readOnly"
      name="lock"
      :size="11"
      title="Locked by an agent"
      :style="{ color: 'var(--status-blocked-fg)' }"
    />
    <span
      v-if="g && g.l"
      :title="git"
      :style="{ color: g.c, width: '9px', textAlign: 'center', fontWeight: 'var(--weight-semibold)' }"
    >{{ g.l }}</span>
  </div>
</template>
