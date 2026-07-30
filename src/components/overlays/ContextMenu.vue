<script setup>
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'

const props = defineProps({
  /* {label, icon?, shortcut?, tone?, disabled?} | {type:'separator'} | {type:'label', label} */
  items: { type: Array, default: () => [] },
  width: { type: Number, default: 200 }
})

const emit = defineEmits(['select'])

const hover = ref(-1)

const menuStyle = computed(() => ({
  width: `${props.width}px`,
  padding: 'var(--space-2)',
  background: 'var(--surface-overlay)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-3)',
  boxShadow: 'var(--shadow-overlay)'
}))

const itemStyle = (it, i) => {
  const on = hover.value === i && !it.disabled
  return {
    display: 'flex',
    alignItems: 'center',
    gap: 'var(--space-4)',
    height: 'var(--row-h)',
    padding: '0 var(--space-4)',
    borderRadius: 'var(--radius-2)',
    background: on ? 'var(--surface-hover)' : 'transparent',
    color: it.disabled
      ? 'var(--text-muted)'
      : it.tone === 'danger'
        ? 'var(--status-failed-fg)'
        : 'var(--text-primary)',
    fontSize: 'var(--text-sm)',
    cursor: it.disabled ? 'not-allowed' : 'default'
  }
}

const labelStyle = {
  padding: 'var(--space-2) var(--space-4)',
  fontSize: 'var(--text-2xs)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}
const sepStyle = { height: '1px', margin: 'var(--space-2) 0', background: 'var(--border-subtle)' }

const onSelect = (it) => {
  if (!it.disabled) emit('select', it)
}
</script>

<template>
  <div role="menu" :style="menuStyle">
    <template v-for="(it, i) in items" :key="i">
      <div v-if="it.type === 'separator'" :style="sepStyle" />
      <div v-else-if="it.type === 'label'" :style="labelStyle">{{ it.label }}</div>
      <div
        v-else
        role="menuitem"
        :aria-disabled="it.disabled || undefined"
        :tabindex="it.disabled ? -1 : 0"
        :style="itemStyle(it, i)"
        @mouseenter="hover = i"
        @mouseleave="hover = -1"
        @click="onSelect(it)"
      >
        <span :style="{ width: '14px', display: 'flex', color: it.disabled ? 'var(--text-muted)' : 'var(--text-secondary)' }">
          <Icon v-if="it.icon" :name="it.icon" :size="14" />
        </span>
        <span :style="{ flex: 1, minWidth: 0, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }">
          {{ it.label }}
        </span>
        <kbd
          v-if="it.shortcut"
          :style="{ color: 'var(--text-muted)', fontSize: 'var(--text-2xs)', fontFamily: 'var(--font-mono)' }"
        >{{ it.shortcut }}</kbd>
      </div>
    </template>
  </div>
</template>
