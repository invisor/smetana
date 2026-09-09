<script setup>
/* The one control on an html tab: the page, or the markup it is made of.

   It sits in the top-right corner of whatever the tab is drawing — over the
   sandboxed document, over the editor — because there is nowhere else for it to
   go. The tab row above belongs to the tabs and is dragged; a toolbar strip
   under it would be a permanent band of chrome across every file tab in the app
   for the sake of one control that is right about `.html` and about nothing
   else. So the corner, and so the position lives here rather than at the call
   site: where this control is is part of what it is, and a second caller placing
   it somewhere else would be a second answer to a question that has one.
   Whatever draws it therefore has to be a positioned box — in the app that is
   the centre column's content, in the gallery it is a box made for the purpose.

   **Translucent at rest and whole under the pointer**, which is the whole of its
   manners. It is drawn on top of somebody's document, so at full strength it
   would be a permanent blot on the corner of every report; at
   `--attn-quiet-opacity` it is legible enough to be found and quiet enough to be
   forgotten, which is the same standing the cut rows in the file tree take under
   that same token. Opacity and not a colour: the ground behind it is an
   arbitrary document nothing here chose, so a value picked to recede against
   `--surface` would be picked against the wrong thing.

   Nothing else about it moves. There is no transform and no scale on hover —
   `core/interactive.js`'s rule, which this file keeps by leaving the surface step
   to `IconButton` inside it and taking only the opacity for itself. */

import { computed, ref } from 'vue'

import IconButton from '../core/IconButton.vue'
import { useInteractive } from '../core/interactive.js'

const props = defineProps({
  /* `document` while the sandboxed page is on screen, `source` while the editor
     is. A string rather than a boolean because both states have names a reader
     of the call site can check against what is drawn, where `:source="false"`
     reads as a double negative at exactly the place the two branches meet. */
  mode: { type: String, default: 'document' }
})

defineEmits(['toggle'])

const { hover, handlers } = useInteractive()

/* Focus is tracked beside the hover and deliberately not folded into it.
   `useInteractive` is about the surface a pointer is over, and the two states
   have different jobs here: the hover is what makes the control readable when
   somebody goes looking for it, and this is what stops a control reached with
   the Tab key from wearing its focus ring at two-thirds strength. `focusin` and
   not `focus`, because the thing being focused is the button inside the box and
   only the bubbling pair reaches the box. */
const focused = ref(false)

/* The glyph names what the press gives, never what is on screen: `code` while
   the document is drawn, `eye` while the source is. The label says the same
   thing in words, which is what `IconButton` puts in its tooltip and what a
   screen reader reads — an icon-only control has no other prose about it. */
const showsDocument = computed(() => props.mode !== 'source')
const icon = computed(() => (showsDocument.value ? 'code' : 'eye'))
const label = computed(() => (showsDocument.value ? 'Show source' : 'Show document'))

const style = computed(() => ({
  position: 'absolute',
  top: 'var(--space-3)',
  right: 'var(--space-3)',
  /* Over the document and under anything that opens on top of the app. The
     frame and the editor are ordinary content, so `--z-sticky` is the step that
     says "stays put over what scrolls" without reaching into the dropdown and
     modal range. */
  zIndex: 'var(--z-sticky)',
  display: 'flex',
  /* Its own ground and border, which the button inside deliberately does not
     have: a ghost `IconButton` is transparent until it is hovered, and a bare
     glyph over an arbitrary document is a glyph over whatever colour that
     document happens to paint under it. */
  background: 'var(--surface-overlay)',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-3)',
  opacity: hover.value || focused.value ? 1 : 'var(--attn-quiet-opacity)',
  transition: 'opacity var(--dur-fast) var(--ease-out)'
}))
</script>

<template>
  <div
    :style="style"
    v-bind="handlers"
    @focusin="focused = true"
    @focusout="focused = false"
  >
    <IconButton :icon="icon" :label="label" size="sm" @click="$emit('toggle')" />
  </div>
</template>
