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
import { priorityLabel } from './issueType.js'

const props = defineProps({
  /* `SessionWork::NewTask` as it arrives: { text, issueType, priority, parent }.
     Auto is null in the middle two, and `parent` is null for every task filed
     from "+ New task" rather than from a card's own menu. */
  draft: { type: Object, required: true }
})

/* `P1`, not a bare `1`, and from the same function the task inspector uses so
   the two cannot drift. Null is Auto here — the shared helper deliberately does
   not decide that word, because the inspector's answer to the same absence is
   to drop the row entirely. */
const priorityText = computed(() => priorityLabel(props.draft.priority) ?? 'Auto')

const body = { display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }

/* The same eyebrow the inspector's field labels use, so this reads as a label
   for the block rather than as a title competing with the text below it. */
const eyebrow = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}

/* The person's prose, pre-wrap: they typed those line breaks, and the breaks
   are part of what they said.

   TaskInspector draws its five prose fields as the markdown bd stores, and this
   deliberately does not — which is now the interesting thing about the pair
   rather than their sameness. Nothing has been filed yet, so what is on screen
   is somebody's own words handed back to them a moment after they typed them,
   and setting those words as markup nobody claimed to be writing would change
   what they said under them. */
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

/* TaskInspector's two-column shape, not its measurements. The label column is
   `max-content` in both, so it is as wide as the longest label each panel
   happens to hold — two labels here against eleven there, so this one comes out
   narrower and the value column starts further left. That is not worth fixing:
   the two never appear at once (a draft has no issue behind it, so
   `inspectedIssue` draws nothing while this is up), and pinning a width to make
   two panels agree that are never seen together would be a number to maintain
   for nothing.

   `center` rather than TaskInspector's `baseline`, and that one *is* deliberate:
   the Type cell holds a badge, and a badge baseline-aligned against a bare word
   hangs below it by the height of its own padding. */
const grid = {
  display: 'grid',
  gridTemplateColumns: 'max-content minmax(0, 1fr)',
  columnGap: 'var(--space-5)',
  rowGap: 'var(--space-3)',
  alignItems: 'center'
}

/* The badge's cell. Without it the wrapper would be a bare inline box at the
   root line-height, which is taller than either thing it can hold: the Type row
   measured 19.5px against Priority's 16.2px, so in a draft left on Auto the two
   identical words sat at different heights. */
const badgeCell = { display: 'flex', alignItems: 'center' }

const rowValue = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-sans)',
  color: 'var(--text-primary)',
  overflowWrap: 'anywhere'
}

/* An identifier, so mono — the same way TaskInspector draws an issue's id. The
   panel has no title to put beside it and deliberately does not fetch one: the
   parent is a card on the board a click away, and a copy here would be one more
   thing to keep in step. */
const parentValue = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-mono)',
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
      <!-- First in the grid, because it is the only row that is about another
           task rather than about this draft, and a person scanning the panel
           wants that before the type and the priority. Drawn only when there is
           one: an ordinary filing has no parent, and an empty row would say
           there was something to know. -->
      <template v-if="draft.parent">
        <span :style="eyebrow">Follow-up to</span>
        <span :style="parentValue">{{ draft.parent }}</span>
      </template>

      <span :style="eyebrow">Type</span>
      <!-- A badge when a type was chosen and the word when it was not: the same
           field in the same place either way, rather than a badge that
           disappears and takes its row with it. -->
      <span :style="badgeCell">
        <TypeBadge v-if="draft.issueType" :type="draft.issueType" size="sm" />
        <span v-else :style="rowValue">Auto</span>
      </span>

      <span :style="eyebrow">Priority</span>
      <span :style="rowValue">{{ priorityText }}</span>
    </div>
  </div>
</template>
