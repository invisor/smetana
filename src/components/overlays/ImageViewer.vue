<script setup>
/* One picture, shown whole, over whatever it was opened from.

   A thumbnail is a crop — 64 pixels of a screenshot say which one it is and
   nothing about what is in it — so the strip needs a way to see the picture
   itself. This is that way, and it is deliberately the smallest one: the image
   fitted into the space there is, its name under it, and a way out. No zoom, no
   panning, no rotation, no saving, and no stepping between the pictures next to
   it. A viewer that pages carries a question this one does not have to answer —
   what it shows when the open picture is taken out of the list — and the strip
   it was opened from is two clicks away.

   `position: absolute` rather than `fixed`, which is `Modal.vue`'s own choice
   and for its reason: `views/Gallery.vue` frames every dialog in a box that is
   `position: relative` and `overflow: hidden`, and a fixed overlay would climb
   out of that box and cover the gallery — breaking the one form of checking
   this project has. Absolute puts it over exactly the area of the nearest
   positioned ancestor, which is the modal's scrim in the app and the frame in
   the gallery.

   The z-index is `--z-modal` rather than nothing, and that is the one thing
   here worth reading twice. A descendant of the modal is inside the stacking
   context its scrim already makes, so painting order settles the question for
   everything drawn before this in the dialog — but not for what is drawn after
   it. `Dropdown`'s root is `position: relative`, and the dialog that opens this
   has five of them below the images: at `z-index: auto` they are painted in
   tree order after this overlay and would sit on top of the scrim. This is not
   a new step in the scale — the token is the one its own ancestor uses — and no
   new one is wanted: `--z-popover` exists for the opposite case, an overlay
   forced to leave its stacking context. */
import { onBeforeUnmount, watch } from 'vue'
import IconButton from '../core/IconButton.vue'

const props = defineProps({
  open: { type: Boolean, default: true },
  /* What the picture is drawn from — the `url` of an attachment record, which
     is already in hand by the time a thumbnail is on screen. Nothing here reads
     anything off the disk. */
  url: { type: String, required: true },
  /* The stored name, shown under the picture. It is an identifier the app made
     up rather than prose, so it is drawn in mono. */
  name: { type: String, default: '' }
})

const emit = defineEmits(['close'])

/* Esc is free in the tree this opens in: the new-task dialog does not close on
   it and nothing above it listens for it. The listener is the document's rather
   than an element's because nothing here takes the keyboard — the focus stays
   on the thumbnail that opened this, which is where a person wants it back when
   the picture goes away. Put on at open and taken off at close and at unmount,
   the way `PointerMenu` does it: a document listener outlives the component
   that added it. */
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

const scrimStyle = {
  position: 'absolute',
  inset: 0,
  zIndex: 'var(--z-modal)',
  background: 'var(--overlay-scrim)',
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

/* Two ceilings on each axis, and both are needed. The `vw`/`vh` half keeps the
   picture off the edges of the window in the app, where the overlay is the
   whole of it; the `100%` half is what holds it inside a frame smaller than the
   window — the gallery's box, and the modal's own scrim in a narrow window.
   With `contain` over the smaller of the two, a picture larger than the space
   is fitted whole and a picture smaller than it is left at its own size: there
   is no `width` here to stretch it, only a maximum. */
const imageStyle = {
  display: 'block',
  maxWidth: 'min(88vw, 100%)',
  maxHeight: 'min(84vh, 100%)',
  objectFit: 'contain'
}

/* The name sits on a surface of its own rather than straight on the scrim, the
   same answer `TerminalView`'s drop label gives: a scrim is a translucent layer
   over whatever happens to be under it, so no text colour reads well on it in
   both themes, and the system has no token for one that would. */
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

/* `solid` rather than the ghost default: the button stands on the scrim with
   nothing behind it, and a ghost one would be a glyph floating over a picture. */
const cornerStyle = {
  position: 'absolute',
  top: 'var(--space-4)',
  right: 'var(--space-4)'
}
</script>

<template>
  <!-- `.self` on both boxes, and that is the whole of "click the darkening to
       close": the scrim's own padding and the empty space around the picture
       are two different elements, and a click on the picture, on its name or on
       the button is not a click on either of them. -->
  <div
    v-if="open"
    role="dialog"
    aria-modal="true"
    :aria-label="name || 'Image'"
    :style="scrimStyle"
    @click.self="$emit('close')"
  >
    <div :style="frameStyle" @click.self="$emit('close')">
      <img :src="url" :alt="name" :style="imageStyle" />
    </div>
    <div v-if="name" :style="captionStyle">{{ name }}</div>
    <div :style="cornerStyle">
      <IconButton icon="x" variant="solid" label="Close" @click="$emit('close')" />
    </div>
  </div>
</template>
