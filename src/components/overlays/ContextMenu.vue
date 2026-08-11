<script setup>
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'

const props = defineProps({
  /* {label, icon?, shortcut?, tone?, disabled?, children?} | {type:'separator'} | {type:'label', label}

     `children` marks a row that opens a submenu: this component draws the
     chevron and says which row the pointer is on, and the caller places the
     second panel — placement is the anchoring component's business, not this
     one's, and this component has never known where on the screen it is. */
  items: { type: Array, default: () => [] },
  width: { type: Number, default: 200 },
  /* Which row reads as current. -1 lets the component keep its own pointer
     tracking, which is what a bare ContextMenu in the gallery wants; a caller
     driving the keyboard passes the index instead, because a submenu opened by
     keyboard has to show which row it hangs off and the hovered row cannot be
     this component's secret. */
  cursor: { type: Number, default: -1 }
})

const emit = defineEmits(['select', 'hover'])

const hover = ref(-1)

const active = computed(() => (props.cursor >= 0 ? props.cursor : hover.value))

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
  const on = active.value === i && !it.disabled
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

/* The pointer's row is kept here as well as announced: a bare ContextMenu has
   nobody to announce it to, and an anchoring caller wants the same fact to
   drive its submenu. -1 on leaving, which is the row's own business — whether
   that closes anything is the caller's. */
const onHover = (i) => {
  hover.value = i
  emit('hover', i)
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
        @mouseenter="onHover(i)"
        @mouseleave="onHover(-1)"
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
        <!-- The only thing on the row that says there is more behind it.
             Colour is never the signal here, and a row that opened a second
             panel with nothing to announce it would be found by accident. -->
        <Icon v-if="it.children" name="chevron-right" :size="14" />
      </div>
    </template>
  </div>
</template>
