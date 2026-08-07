<script setup>
/* The task an agent is filing, before there is a task.

   It stands where TaskInspector stands and deliberately does not look like it.
   There is no id, no status, no delete and no "Ask agent to edit", because none
   of those exist yet: the agent has not run `bd create`, and when it does the
   card arrives on the board through the watcher with nothing tying it back to
   the session that filed it. So this panel is the person's own words handed
   back to them, read-only, and it says as much in one quiet line rather than
   leaving somebody to wonder why the panel is missing half its controls.

   Auto is drawn as Auto. The two fields arrive as null when the dialog was left
   on Auto — the invariant `TaskDraft` and `prompt.rs` hold on the other side —
   and naming the agent's eventual choice here would be inventing one. */
import { computed } from 'vue'
import TypeBadge from './TypeBadge.vue'

const props = defineProps({
  /* `SessionWork::NewTask` as it arrives: { text, issueType, priority }. Auto
     is null in both of the last two. */
  draft: { type: Object, required: true }
})

/* Priority is written the way the inspector writes it — P1, not "1" — so the
   same number means the same thing in both panels. */
const priorityText = computed(() =>
  props.draft.priority === null || props.draft.priority === undefined
    ? null
    : `P${props.draft.priority}`
)

const body = { display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }

/* The same eyebrow the inspector's field labels use, so this reads as a label
   for the block rather than as a title competing with the text below it. */
const eyebrow = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}

/* The person's prose, set as the inspector sets a description: pre-wrap,
   because they typed the line breaks and those are part of what they said. */
const textStyle = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-primary)',
  whiteSpace: 'pre-wrap',
  textWrap: 'pretty'
}

const noteStyle = {
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-muted)',
  textWrap: 'pretty'
}

/* TaskInspector's grid, to the pixel: the two panels take the same slot and a
   different label column would make switching between them jump. */
const grid = {
  display: 'grid',
  gridTemplateColumns: 'max-content minmax(0, 1fr)',
  columnGap: 'var(--space-5)',
  rowGap: 'var(--space-3)',
  alignItems: 'center'
}

const rowValue = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-sans)',
  color: 'var(--text-primary)',
  overflowWrap: 'anywhere'
}

const divider = { height: 'var(--border-w)', background: 'var(--border-subtle)' }
</script>

<template>
  <div :style="body">
    <span :style="eyebrow">Draft</span>

    <div :style="textStyle">{{ draft.text }}</div>

    <div :style="noteStyle">
      Not on the board yet. The agent writes the title and files it; the card
      appears when it does.
    </div>

    <div :style="divider" />

    <div :style="grid">
      <span :style="eyebrow">Type</span>
      <!-- A badge when a type was chosen and the word when it was not: the same
           field in the same place either way, rather than a badge that
           disappears and takes its row with it. -->
      <span>
        <TypeBadge v-if="draft.issueType" :type="draft.issueType" size="sm" />
        <span v-else :style="rowValue">Auto</span>
      </span>

      <span :style="eyebrow">Priority</span>
      <span :style="rowValue">{{ priorityText ?? 'Auto' }}</span>
    </div>
  </div>
</template>
