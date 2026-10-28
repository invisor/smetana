<script setup>
/* The sheet a tap on a board card opens: the desktop's card menu, drawn for a thumb.

   It draws the rows it is given and decides nothing about them. Which rows exist, in what
   order, which are grey and what a grey one says is `sheetRows` in ../taskSheet.js, which asks
   the desktop's own `taskMenuItems`; what a tap on a row does is `Workspace.vue`'s. A pointer
   menu would not do here: the desktop's `ContextMenu` is measured against a 212px card and
   opens sideways, and a phone has neither the room nor a pointer.

   Move to... is a second step in the same sheet instead of a submenu: the tap swaps the rows
   for the statuses, with Back on top. The sheet closes on a tap on the backdrop, on Escape and
   on the system's back gesture, which is a history step pushed while the sheet is up. A grey
   row is drawn as it is and does nothing; its label already carries the reason. */
import { onBeforeUnmount, onMounted, ref } from 'vue'
import Icon from '@src/components/core/Icon.vue'
import { inset } from './safeArea.js'

const props = defineProps({
  /* The board card the sheet was opened on: `id` and `title` are drawn. */
  card: { type: Object, required: true },
  /* From `sheetRows`. */
  rows: { type: Array, default: () => [] },
  /* One line under the rows explaining why some are grey, '' for none. */
  note: { type: String, default: '' },
  /* The tracker's last refusal, '' when there is none. There are no toasts on a phone. */
  error: { type: String, default: '' }
})
const emit = defineEmits(['close', 'act'])

/* The Move to... row being looked into, or null on the first step. */
const step = ref(null)

function pick(row) {
  if (row.disabled) return
  if (row.children) {
    step.value = row
    return
  }
  emit('act', { kind: row.kind, id: props.card.id, value: row.value })
}

/* The system's back: the sheet pushes one history entry while it is up, and the entry going
   is what closes it. Closing by any other way takes the entry off again. */
let popped = false
const onPop = () => {
  popped = true
  emit('close')
}
const onKey = (event) => {
  if (event.key === 'Escape') emit('close')
}
onMounted(() => {
  history.pushState({ taskSheet: true }, '')
  window.addEventListener('popstate', onPop)
  window.addEventListener('keydown', onKey)
})
onBeforeUnmount(() => {
  window.removeEventListener('popstate', onPop)
  window.removeEventListener('keydown', onKey)
  if (!popped && history.state?.taskSheet) history.back()
})

const backdrop = {
  position: 'fixed',
  inset: 0,
  zIndex: 'var(--z-modal)',
  background: 'var(--overlay-scrim)',
  display: 'flex',
  flexDirection: 'column',
  justifyContent: 'flex-end'
}
const sheet = {
  display: 'flex',
  flexDirection: 'column',
  maxHeight: '85dvh',
  background: 'var(--surface-raised)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  borderTop: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-3) var(--radius-3) 0 0',
  boxShadow: 'var(--shadow-overlay)',
  padding: `var(--space-4) ${inset('right', 'var(--space-4)')} ${inset('bottom', 'var(--space-5)')} ${inset('left', 'var(--space-4)')}`
}
const head = {
  flex: '0 0 auto',
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  padding: 'var(--space-4) var(--space-4) var(--space-5)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const idStyle = { font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)', color: 'var(--text-muted)' }
const titleStyle = {
  font: 'var(--weight-medium) var(--text-md)/var(--leading-snug) var(--font-sans)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
const list = { flex: '1 1 auto', minHeight: 0, overflowY: 'auto', padding: 'var(--space-3) 0' }
const separator = { height: 'var(--border-w)', margin: 'var(--space-3) 0', background: 'var(--border-subtle)' }
const rowStyle = (row) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-5)',
  width: '100%',
  /* A touch target: the desktop's row height is for a pointer, and a thumb wants about half as
     much again. */
  minHeight: 'calc(var(--row-h) * 1.6)',
  padding: '0 var(--space-4)',
  border: 0,
  borderRadius: 'var(--radius-2)',
  background: 'transparent',
  textAlign: 'left',
  font: 'var(--weight-regular) var(--text-md)/var(--leading-snug) var(--font-sans)',
  color: row.disabled ? 'var(--text-muted)' : row.tone === 'danger' ? 'var(--status-failed-fg)' : 'var(--text-primary)',
  cursor: row.disabled ? 'not-allowed' : 'pointer'
})
const gutter = (row) => ({
  flex: 'none',
  width: 'var(--space-6)',
  display: 'flex',
  color: row.disabled ? 'var(--text-muted)' : row.tone === 'danger' ? 'var(--status-failed-fg)' : 'var(--text-secondary)'
})
const label = { flex: 1, minWidth: 0 }
const failure = {
  margin: 0,
  padding: 'var(--space-4) var(--space-4) 0',
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  color: 'var(--status-failed-fg)'
}
const noteStyle = {
  margin: 0,
  padding: 'var(--space-3) var(--space-4) 0',
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-muted)'
}
const back = { kind: 'back', label: 'Back', icon: 'chevron-left' }
</script>

<template>
  <div :style="backdrop" @click.self="emit('close')">
    <div role="dialog" aria-modal="true" :aria-label="`Actions for ${card.id}`" :style="sheet">
      <div :style="head">
        <span :style="idStyle">{{ card.id }}</span>
        <span :style="titleStyle">{{ card.title }}</span>
      </div>
      <div role="menu" :style="list">
        <template v-if="step">
          <button type="button" role="menuitem" :style="rowStyle(back)" @click="step = null">
            <span :style="gutter(back)"><Icon :name="back.icon" :size="16" /></span>
            <span :style="label">{{ back.label }}</span>
          </button>
          <button
            v-for="row in step.children"
            :key="row.value"
            type="button"
            role="menuitem"
            :aria-disabled="row.disabled || undefined"
            :style="rowStyle(row)"
            @click="pick(row)"
          >
            <span :style="gutter(row)"><Icon v-if="row.icon" :name="row.icon" :size="16" /></span>
            <span :style="label">{{ row.label }}</span>
          </button>
        </template>
        <template v-else>
          <template v-for="(row, i) in rows" :key="i">
            <div v-if="row.type === 'separator'" :style="separator" />
            <button
              v-else
              type="button"
              role="menuitem"
              :aria-disabled="row.disabled || undefined"
              :style="rowStyle(row)"
              @click="pick(row)"
            >
              <span :style="gutter(row)"><Icon v-if="row.icon" :name="row.icon" :size="16" /></span>
              <span :style="label">{{ row.label }}</span>
              <Icon v-if="row.children" name="chevron-right" :size="14" />
            </button>
          </template>
        </template>
      </div>
      <p v-if="note" :style="noteStyle">{{ note }}</p>
      <p v-if="error" role="alert" :style="failure">{{ error }}</p>
    </div>
  </div>
</template>
