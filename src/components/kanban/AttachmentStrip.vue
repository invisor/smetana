<script setup>
import IconButton from '../core/IconButton.vue'
import Tooltip from '../core/Tooltip.vue'

/* What is attached to a task that has not been filed yet: a row of thumbnails,
   each with a way to take it back out, and a way to ask for the picture whole.

   Asking is all it does. The picture opens in an OS window of its own, which
   this component neither builds nor knows about — it says which picture, and
   its host does the rest. It used to hold the viewer itself, over the scrim of
   the modal it sat in; when the new-task dialog became a window of its own that
   scrim went, and an overlay pinned to `inset: 0` was pinned to a 440px window
   that cannot be resized (smetana-msxp).

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

/* `view` carries `{ path, name }` and this component does not open anything
   itself. The picture is shown in an OS window of its own now, and asking for
   one is `stores/app.js`'s to do — a store import in here would be a component
   that cannot be drawn in `?view=gallery` with nothing behind it, which is the
   one form of checking this project has. `views/DialogWindow.vue` answers this
   emit, exactly as it answers `attach`, `files` and `remove`.

   The path and the name and not the record: the window reads the file itself,
   and the `url` beside them is up to 8 MiB of base64 that would have to cross
   two process boundaries to say nothing the path does not. */
defineEmits(['remove', 'view'])

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

/* The picture is a button so that the keyboard can reach it: Tab lands on it,
   Enter and Space open it, and the ring in `tokens/base.css` says where the
   keyboard is. There is no hover state on it and there is nowhere to put one —
   this system's hover is a step up in surface, and every pixel of this
   control's surface is covered by the picture it is made of.

   The ring is pulled inside by its own width, which is the one line here that
   is a workaround. `base.css` draws it a pixel *outside* the element, and this
   button fills a cell that clips (`overflow: hidden`, for the rounded corners)
   inside a strip that clips as well (`overflow-y: auto`), so a ring drawn
   outside would be cut off on the top row and down the first column. Drawn over
   the picture's own edge instead, it is whole wherever the thumbnail sits. */
const thumb = {
  display: 'block',
  width: '100%',
  height: '100%',
  padding: 0,
  border: 0,
  background: 'transparent',
  cursor: 'default',
  outlineOffset: 'calc(var(--border-w-strong) * -1)'
}

/* `cover` rather than `contain`: a thumbnail of a screenshot is a reminder of
   which one it is, and a letterboxed strip of grey says less than a crop. The
   viewer is what `contain` is for. */
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
        <!-- Opening is the picture's own click and not the cell's. The remove
             button lies over the picture as a sibling rather than inside it, so
             its click never reaches here and taking an image out cannot open a
             window on it. Viewing stays available while the dialog is busy
             filing the task: it names a picture that is already attached and
             changes nothing, which is the whole of what `disabled` guards. -->
        <button
          type="button"
          :style="thumb"
          :aria-label="`View ${item.name}`"
          @click="$emit('view', { path: item.path, name: item.name })"
        >
          <img :src="item.url" :alt="item.name" :style="image" />
        </button>
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
