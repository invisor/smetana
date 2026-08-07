<script setup>
import IconButton from '../core/IconButton.vue'
import Tooltip from '../core/Tooltip.vue'

/* What is attached to a task that has not been filed yet: a row of thumbnails,
   each with a way to take it back out.

   The name is a tooltip rather than a caption. By the time a picture reaches
   this strip it has been renamed to a stamped name of the app's own making —
   `20260806-121314-mock.png` — so a visible label would spend a line of the
   dialog on a string nobody chose and nobody reads. The picture is the label. */
const props = defineProps({
  /* { path, name, url } — `url` is what the thumbnail is drawn from, `path` is
     what identifies one for removal and what eventually reaches the agent. */
  items: { type: Array, default: () => [] },
  disabled: { type: Boolean, default: false }
})

defineEmits(['remove'])

/* A dimension, like Toast's width and KanbanColumn's — big enough to tell one
   screenshot from another, small enough that four of them fit across the
   dialog. */
const THUMB = '64px'

/* Two rows, and then it scrolls. Nothing bounds how many images are attached —
   the picker takes several at once and a drop carries every path in it — while
   the dialog has no scrolling of its own and the window may be as short as
   640px. Without a ceiling here the fifth row pushes Cancel and Create off the
   bottom of a screen with nothing to scroll back. The same answer the file tree
   and the log give: a maximum height and `auto`. */
const strip = {
  display: 'flex',
  flexWrap: 'wrap',
  gap: 'var(--space-4)',
  maxHeight: `calc(${THUMB} * 2 + var(--space-4))`,
  overflowY: 'auto',
  /* A wrapped flex container hands leftover height to its lines; without this
     one row of thumbnails would sit spread down a box two rows tall. */
  alignContent: 'flex-start'
}

const cell = {
  position: 'relative',
  width: THUMB,
  height: THUMB,
  borderRadius: 'var(--radius-3)',
  border: 'var(--border-w) solid var(--border-subtle)',
  background: 'var(--surface-sunken)',
  overflow: 'hidden'
}

/* `cover` rather than `contain`: a thumbnail of a screenshot is a reminder of
   which one it is, and a letterboxed strip of grey says less than a crop. */
const image = { width: '100%', height: '100%', objectFit: 'cover', display: 'block' }

const corner = { position: 'absolute', top: 'var(--space-1)', right: 'var(--space-1)' }
</script>

<template>
  <div v-if="props.items.length" :style="strip">
    <div v-for="item in props.items" :key="item.path" :style="cell">
      <!-- The name hangs on the picture, not on the cell. The remove button
           sits inside the cell and carries a hint of its own, and a cell-wide
           wrapper would still be hovered while the pointer was on the button —
           two panels over one thumbnail. Leaving the picture wraps the whole of
           the cell a person actually points at, and moving onto the button
           leaves it, so exactly one is ever open. -->
      <Tooltip :label="item.name" :style="{ width: '100%', height: '100%' }">
        <img :src="item.url" :alt="item.name" :style="image" />
      </Tooltip>
      <div :style="corner">
        <IconButton
          icon="x"
          variant="solid"
          size="sm"
          :label="`Remove ${item.name}`"
          :disabled="props.disabled"
          @click="$emit('remove', item.path)"
        />
      </div>
    </div>
  </div>
</template>
