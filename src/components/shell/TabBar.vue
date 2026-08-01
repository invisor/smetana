<script setup>
import IconButton from '../core/IconButton.vue'
import Tab from './Tab.vue'

const props = defineProps({
  tabs: { type: Array, default: () => [] },
  activeId: { type: [String, Number], default: undefined },
  overflowCount: { type: Number, default: 0 }
})

defineEmits(['select', 'close', 'promote'])

const barStyle = {
  display: 'flex',
  alignItems: 'stretch',
  height: 'var(--tab-h)',
  flex: '0 0 auto',
  background: 'var(--surface)',
  borderBottom: 'var(--border-w) solid var(--border)',
  minWidth: 0
}
const overflowStyle = {
  display: 'flex',
  alignItems: 'center',
  padding: '0 var(--space-3)',
  borderRight: 'var(--border-w) solid var(--border-subtle)'
}
</script>

<template>
  <div role="tablist" :style="barStyle">
    <div :style="{ display: 'flex', minWidth: 0, overflowX: 'auto', overflowY: 'hidden' }">
      <Tab
        v-for="t in props.tabs"
        :key="t.id"
        v-bind="t"
        :active="t.id === activeId"
        @select="$emit('select', t.id)"
        @close="$emit('close', t.id)"
        @promote="$emit('promote', t.id)"
      />
    </div>
    <div v-if="overflowCount > 0" :style="overflowStyle">
      <IconButton icon="chevrons-right" :label="`${overflowCount} more tabs`" size="sm" />
      <span :style="{ fontSize: 'var(--text-2xs)', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)' }">
        +{{ overflowCount }}
      </span>
    </div>
    <div :style="{ flex: 1 }" />
    <div
      v-if="$slots.actions"
      :style="{ display: 'flex', alignItems: 'center', gap: 'var(--space-2)', padding: '0 var(--space-3)' }"
    >
      <slot name="actions" />
    </div>
  </div>
</template>
