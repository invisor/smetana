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
  /* A ceiling, not a width. The panel is as wide as its widest row wants to be
     and no wider — a four-verb menu that took 400px because one of its rows
     *might* one day carry a sentence is 400px of empty panel over the board for
     the whole of the ordinary case. The number is what a row may not exceed,
     past which the label still clips with an ellipsis, which is the reason a
     ceiling exists at all: a row has no tooltip and no `title`, so a caller
     that lets a label grow without limit is a caller that hangs a menu off the
     screen. */
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
  /* Sized by the widest row and clamped at `width`. `max-content` under the
     system's border-box reset is the content's own width plus this panel's
     padding and border, so the ceiling means the same thing it always did — the
     whole panel, chrome included — and a row that reaches it clips exactly
     where it used to. */
  width: 'max-content',
  maxWidth: `${props.width}px`,
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

/* The 14px column either side of a label. `flex: none` rather than a bare
   width: these are flex items, so at the ceiling the default shrink would let
   them give ground before the label does — and the label is the one thing on
   the row with an ellipsis to fall back on. */
const gutterStyle = (it) => ({
  flex: 'none',
  width: '14px',
  display: 'flex',
  color: it.disabled ? 'var(--text-muted)' : 'var(--text-secondary)'
})

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
        <span :style="gutterStyle(it)">
          <Icon v-if="it.icon" :name="it.icon" :size="14" />
        </span>
        <span :style="{ flex: 1, minWidth: 0, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }">
          {{ it.label }}
        </span>
        <kbd
          v-if="it.shortcut"
          :style="{ color: 'var(--text-muted)', fontSize: 'var(--text-2xs)', fontFamily: 'var(--font-mono)' }"
        >{{ it.shortcut }}</kbd>
        <!-- The far gutter, and it is drawn on every row whether or not that
             row has a chevron to put in it. It mirrors the icon column, so a
             label sits between two equal margins instead of running up against
             the panel's edge — which is what a content-sized panel does to it
             otherwise, since the width is now the label's own and nothing is
             left over. The chevron is the only thing on the row that says there
             is more behind it: colour is never the signal here, and a row that
             opened a second panel with nothing to announce it would be found by
             accident. -->
        <span :style="gutterStyle(it)">
          <Icon v-if="it.children" name="chevron-right" :size="14" />
        </span>
      </div>
    </template>
  </div>
</template>
