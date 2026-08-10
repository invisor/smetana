<script setup>
/* One setting: what it is called, what it does, and the control that changes
   it. Every tab is a stack of these, so the label column lines up down the whole
   window and a person reads names rather than hunting controls.

   The control is a slot rather than a prop because the rows differ in what they
   hold — a dropdown today, a group of choices tomorrow — while the shape around
   them must not. */
const props = defineProps({
  label: { type: String, required: true },
  /* One line under the label, for what the control cannot say itself. Left out
     where the label is the whole story: a description that repeats the name
     spends a line saying nothing. */
  description: { type: String, default: '' },
  /* How wide the control column is. A dropdown of font sizes and a picker of
     two agents want different widths, and letting each row say so keeps the
     labels aligned regardless.

     In `ch` by default, like every other measure in this window: the control
     inside it is sized by the type, so a ceiling in pixels would start clipping
     the moment the app-wide font size grew. */
  controlWidth: { type: String, default: '24ch' }
})

const rowStyle = {
  display: 'flex',
  alignItems: 'flex-start',
  gap: 'var(--space-5)',
  padding: 'var(--space-4) 0',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const textStyle = { flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 'var(--space-2)' }
const labelStyle = {
  color: 'var(--text-primary)',
  font: 'var(--weight-medium) var(--text-ui-size)/var(--leading-snug) var(--font-sans)'
}
const descriptionStyle = {
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
</script>

<template>
  <div :style="rowStyle">
    <div :style="textStyle">
      <span :style="labelStyle">{{ props.label }}</span>
      <span v-if="props.description" :style="descriptionStyle">{{ props.description }}</span>
    </div>
    <div :style="{ flex: '0 0 auto', width: props.controlWidth, display: 'flex', justifyContent: 'flex-end' }">
      <slot />
    </div>
  </div>
</template>
