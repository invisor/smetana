<script setup>
/* One picture, shown whole, filling the window it is the whole of.

   A thumbnail is a crop — 64 pixels of a screenshot say which one it is and
   nothing about what is in it — so the strip needs a way to see the picture
   itself. This is that way, and it is deliberately the smallest one: the image
   fitted into the space there is, its name under it, and Esc. No zoom, no
   panning, no rotation, no saving, and no stepping between the pictures next to
   it. A viewer that pages carries a question this one does not have to answer —
   what it shows when the open picture is taken out of the list — and the strip
   it was opened from is one window away.

   **It is the body of an OS window now, and that is its only form.** It used to
   be an overlay over the new-task modal's scrim, which was right for as long as
   that dialog was a modal over the board. When every dialog became a window of
   its own the scrim went with it, and `inset: 0` came to mean the viewport of a
   440-pixel window that cannot be resized — so "the picture, larger" was the
   picture at the size of the dialog it was opened from (smetana-msxp). The
   window it fills now is `views/ImageWindow.vue`, opens at 900x700 and is
   dragged and resized by its own frame.

   Three things went with the scrim rather than being kept behind a prop, since
   a second form nothing in the product draws is a second form nobody checks.
   There is no close button: the OS frame carries one. A click on the background
   closes nothing: the empty space around the picture is the only part of this
   window somebody can take hold of to drag it, and a click that closed the
   window would make it undraggable. And there is no `z-index`: this is the
   whole page, with nothing drawn after it to sit on top.

   `position: absolute; inset: 0` stays. In the window that is the whole page,
   and in `?view=gallery` it is exactly the frame the gallery puts around it —
   so the same component is checkable in both places, which is the only form of
   checking this project has for it. */
import { onBeforeUnmount, watch } from 'vue'

const props = defineProps({
  open: { type: Boolean, default: true },
  /* What the picture is drawn from — the `url` of an attachment record, which
     the window has already read back out of the store. Nothing here reaches the
     disk. */
  url: { type: String, required: true },
  /* The stored name, shown under the picture. It is an identifier the app made
     up rather than prose, so it is drawn in mono. */
  name: { type: String, default: '' }
})

const emit = defineEmits(['close'])

/* Esc closes, and the window it is in answers by closing itself. The listener
   is the document's rather than an element's because nothing here takes the
   keyboard — there is nothing in this window to focus. Put on at open and taken
   off at close and at unmount, the way `PointerMenu` does it: a document
   listener outlives the component that added it. */
const onKeydown = (event) => {
  if (event.key !== 'Escape') return
  event.preventDefault()
  emit('close')
}

watch(
  () => props.open,
  (open) => {
    document.removeEventListener('keydown', onKeydown)
    if (open) document.addEventListener('keydown', onKeydown)
  },
  { immediate: true }
)

onBeforeUnmount(() => document.removeEventListener('keydown', onKeydown))

/* The page this window holds: the app's own ground, the same token every other
   window of this app paints its root with. The padding is what keeps the
   picture off the frame — there is no share of the viewport in the ceilings
   below any more, so the margin is entirely this box's to give. */
const pageStyle = {
  position: 'absolute',
  inset: 0,
  background: 'var(--canvas)',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  gap: 'var(--space-5)',
  padding: 'var(--space-6)',
  overflow: 'hidden'
}

/* The picture takes whatever height is left over once the name has had its
   line, and `minHeight: 0` is what lets it: a flex item refuses to shrink below
   its content by default, and the content here is a screenshot that can be
   thousands of pixels tall. */
const frameStyle = {
  flex: 1,
  minWidth: 0,
  minHeight: 0,
  width: '100%',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center'
}

/* `100%` on both axes, which is the box this component was given and nothing
   else. It used to be `min(88vw, 100%)`: the viewport share was there to hold
   the picture off the edges of the *app* window, which this overlay covered
   whole, and in a window of its own that job belongs to the padding above. With
   `contain` under the ceiling, a picture larger than the space is fitted whole
   and a picture smaller than it is left at its own size — there is no `width`
   here to stretch one, only a maximum. */
const imageStyle = {
  display: 'block',
  maxWidth: '100%',
  maxHeight: '100%',
  objectFit: 'contain'
}

/* The name on a surface of its own rather than straight on the page. It is an
   identifier and not prose, and this system draws a standalone identifier as a
   bordered mono chip — the same shape it had over the scrim, kept because it is
   what separates the app's stamped file name from the picture above it. */
const captionStyle = {
  flexShrink: 0,
  maxWidth: '100%',
  overflowWrap: 'anywhere',
  padding: 'var(--space-2) var(--space-4)',
  background: 'var(--surface-overlay)',
  color: 'var(--text-primary)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-2)',
  boxShadow: 'var(--shadow-overlay)',
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-mono)'
}
</script>

<template>
  <div v-if="open" role="group" :aria-label="name || 'Image'" :style="pageStyle">
    <div :style="frameStyle">
      <img :src="url" :alt="name" :style="imageStyle" />
    </div>
    <div v-if="name" :style="captionStyle">{{ name }}</div>
  </div>
</template>
