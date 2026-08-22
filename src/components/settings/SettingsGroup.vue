<script setup>
/* A named group of settings rows: a caption over them, and a spine down their
   left edge saying how far the group reaches.

   A tab is a stack of `SettingsRow`s and every one of them looks the same, so
   until now the only thing marking where one group ended and the next began was
   a line of text in the flow — which reads as a row without a control rather
   than as a heading over what follows. Two marks replace it and they say
   different halves of the same thing: the caption says the group has a name,
   and the spine says which rows are under it. The spine is what the caption
   alone could never do — it has an end, so a person can see that Startup stops
   before the tab does.

   **The caption is not a control.** No press, no focus, no hover: it names the
   rows under it and does nothing else, which is what keeps the tab's tab order
   the list of things a person can actually change.

   The spine is a **border weight rather than a hue**, and deliberately: the
   left edge of a thing is where this app puts dependency and status meaning,
   and a group of settings is neither. `--border-strong` against the row rules'
   `--border-subtle` is enough to read as deliberate in both themes without
   spending any of the colour budget.

   One level of nesting, and no more. A group inside a group would need a second
   spine beside the first, and two vertical lines a few pixels apart is a
   different design question than this one — a tab that wants more structure
   takes another top-level group instead.

   The label is optional, and the headerless form is a real case rather than a
   fallback: the Kanban tab's lists of columns belong to the row above them
   rather than to a name of their own, so they want the spine and the indent
   with no caption over them. Without a label there is nothing to put a gap
   above either, so the top margin goes with it. */
import { computed } from 'vue'

const props = defineProps({
  /* The group's name, drawn in caps. Left out for a group whose own caption
     sits outside it, or which has no name at all. */
  label: { type: String, default: '' }
})

/* The gap belongs to the caption, not to the spine: a headerless group follows
   whatever it is subordinate to, and a `--space-8` between the two would break
   exactly the thing the indent is there to say. */
const wrapStyle = computed(() => ({
  marginTop: props.label ? 'var(--space-8)' : '0'
}))

/* Mono, caps, `--text-2xs`: the idiom this system already uses for a caption
   over a block — the gallery's own section heads and the panel captions — and
   as far from a row's label (medium sans, `--text-ui-size`) as the type scale
   goes, which is the whole point of it. */
const headerStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)'
}
const labelStyle = {
  flex: '0 0 auto',
  color: 'var(--text-secondary)',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase'
}
/* The rule takes whatever the words leave, so the caption reads as the start of
   a band across the tab rather than as a short word floating over a stack of
   rows. It is the row rules' own hairline, which is what keeps it quieter than
   the spine it introduces. */
const ruleStyle = {
  flex: '1 1 auto',
  height: 'var(--border-w)',
  background: 'var(--border-subtle)'
}
/* `--space-1` under the caption against `--space-8` over it: the group reads
   tighter inside than the gap that precedes it, which is what makes the caption
   belong to the rows below rather than to the rows above.

   The spine spans exactly this element, and this element holds exactly the
   group's rows — so it starts at the first row's top edge and ends on the last
   row's own bottom rule, with no arithmetic anywhere to keep in step. */
const bodyStyle = computed(() => ({
  marginTop: props.label ? 'var(--space-1)' : '0',
  borderLeft: 'var(--border-w) solid var(--border-strong)',
  paddingLeft: 'var(--space-6)'
}))
</script>

<template>
  <div :style="wrapStyle">
    <div v-if="props.label" :style="headerStyle">
      <span :style="labelStyle">{{ props.label }}</span>
      <div :style="ruleStyle" />
    </div>
    <div :style="bodyStyle">
      <slot />
    </div>
  </div>
</template>
