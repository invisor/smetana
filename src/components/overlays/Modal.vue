<script setup>
import { computed, inject } from 'vue'
import IconButton from '../core/IconButton.vue'

/* Destructive confirms state the consequence, not an apology:
   "Discard worktree?" / "wt/bd-a1b2 has 3 uncommitted files and 1 agent still running." */
const props = defineProps({
  open: { type: Boolean, default: true },
  title: { type: String, required: true },
  description: { type: String, default: '' },
  width: { type: Number, default: 440 },
  closable: { type: Boolean, default: true },
  /* How much room the body and the footer take, for a guest whose own design
     asks for more than the shared one. Both default to exactly what they were
     hardcoded as, so the eleven other dialogs pass neither and did not change —
     the same shape `DiffView`'s column captions took when the compare window
     needed different ones.

     `review-changes` is the one that passes them: it is 720 wide rather than
     440, and a dialog at that width laid out on the padding of a 440 one reads
     as a form that was stretched rather than designed. They are token names
     rather than lengths, and there is nowhere in this component that would let
     a caller write a pixel into one. */
  bodyPadding: { type: String, default: '' },
  footerPadding: { type: String, default: '' }
})

defineEmits(['close'])

/* Whether this dialog is being drawn inside a window of its own rather than over
   the app. `views/DialogWindow.vue` provides it; everywhere else — the app
   window, the gallery — the default answers and nothing had to change.

   In a window there is no scrim to draw and no header either: the OS frame
   carries the title and the close button, and drawing our own under them would
   be the title written twice. The border, the radius and the shadow go with the
   header, because they are that frame's and the frame is the operating
   system's now. What stays is the description, the body and the footer, because
   those are the dialog.

   `inject` rather than a prop, and that is what keeps the eight guests
   unrewritten: none of them has to pass a flag through to say where it is
   being drawn. */
const inWindow = inject('smDialogWindow', false)

/* In the app this is the scrim: the layer that dims the board and centres the
   dialog over it. In a window there is nothing to dim and nothing to centre
   against — the window is the dialog's own — so it is an ordinary block that
   simply holds what is inside it. */
const scrimStyle = computed(() =>
  inWindow
    ? { display: 'block' }
    : {
        position: 'absolute',
        inset: 0,
        zIndex: 'var(--z-modal)',
        background: 'var(--overlay-scrim)',
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'center',
        paddingTop: '8vh'
      }
)

const dialogStyle = computed(() => ({
  /* The window is already the width the registry gave it, so the dialog takes
     all of it: a fixed 440 inside a 440 window would leave the difference of
     any future width sitting empty beside it. */
  width: inWindow ? '100%' : `${props.width}px`,
  maxWidth: inWindow ? 'none' : '92%',
  background: 'var(--surface-overlay)',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-body-size)/var(--leading-normal) var(--font-sans)',
  border: inWindow ? 'none' : 'var(--border-w) solid var(--border-strong)',
  borderRadius: inWindow ? '0' : 'var(--radius-4)',
  boxShadow: inWindow ? 'none' : 'var(--shadow-modal)'
}))

/* Without the header above it the body owes its own top padding: the header's
   bottom padding was carrying it. The description, when there is one, is drawn
   with that same header padding and takes the job back. */
const bodyStyle = computed(() => ({
  padding:
    props.bodyPadding ||
    (inWindow && !props.description ? 'var(--space-5)' : '0 var(--space-5) var(--space-5)')
}))

const footerStyle = computed(() => ({
  display: 'flex',
  justifyContent: 'flex-end',
  alignItems: 'center',
  gap: 'var(--space-4)',
  padding: props.footerPadding || 'var(--space-4) var(--space-5)',
  borderTop: 'var(--border-w) solid var(--border-subtle)',
  background: 'var(--surface)'
}))
</script>

<template>
  <div v-if="open" :style="scrimStyle">
    <!-- `aria-modal` is a claim about the rest of the page being inert, and in a
         window of its own there is no rest of the page for it to be true of. -->
    <div role="dialog" :aria-modal="!inWindow" :aria-label="title" :style="dialogStyle">
      <div
        v-if="!inWindow"
        :style="{ display: 'flex', alignItems: 'flex-start', gap: 'var(--space-5)', padding: 'var(--space-5) var(--space-5) var(--space-4)' }"
      >
        <div :style="{ flex: 1, minWidth: 0 }">
          <div :style="{ fontSize: 'var(--text-md)', fontWeight: 'var(--weight-semibold)' }">{{ title }}</div>
          <div
            v-if="description"
            :style="{ marginTop: 'var(--space-2)', fontSize: 'var(--text-sm)', color: 'var(--text-secondary)' }"
          >{{ description }}</div>
        </div>
        <IconButton v-if="closable" icon="x" label="Close" size="sm" @click="$emit('close')" />
      </div>
      <!-- The title went with the header, to the OS frame; the description did
           not, because a frame has nowhere to put a sentence. -->
      <div
        v-else-if="description"
        :style="{ padding: 'var(--space-5) var(--space-5) var(--space-4)', fontSize: 'var(--text-sm)', color: 'var(--text-secondary)' }"
      >{{ description }}</div>
      <div v-if="$slots.default" :style="bodyStyle">
        <slot />
      </div>
      <div v-if="$slots.footer" :style="footerStyle">
        <slot name="footer" />
      </div>
    </div>
  </div>
</template>
