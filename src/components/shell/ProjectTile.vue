<script setup>
/* One project on the rail: a two-letter monogram, 28×28, with a state dot in
   its bottom-right corner.

   Selection is carried by the light fill alone. The left marker bar and the
   focus ring were both taken off during the design review, and neither is to be
   restored here without asking — which is why the outline is turned off
   explicitly rather than left to the stylesheet's `:focus-visible`.

   The dot tells a project that is working from one that is waiting on somebody
   by hue and by a pulse, and `prefers-reduced-motion` takes the pulse away —
   which leaves two hues on an 8px circle, the one thing `status/status.js`
   refuses everywhere else. So the state is said in words in the tooltip as
   well. A tooltip is not as good as a glyph, and the alternative on a tile this
   size is a second dot beside the first, which is the confusion the whole
   status system is written to prevent.

   The three warning marks a project row used to carry are in the panel header
   now, where there is room for a glyph and a sentence. The one of them that is
   true of a project this window is *not* pointed at — no bd tracker — is said
   in this tooltip, since it is the only place it can be said at all. */
import { computed } from 'vue'
import Tooltip from '../core/Tooltip.vue'
import { useInteractive } from '../core/interactive.js'
import { monogram } from './monogram.js'

const props = defineProps({
  /* `{path, name, tracked}`, a row of `projectRows` in stores/projects.js. */
  project: { type: Object, required: true },
  active: { type: Boolean, default: false },
  /* 'live' | 'loud' | 'idle', from `projectStates` in stores/terminals.js. */
  state: { type: String, default: 'idle' },
  /* How that state reads out loud, e.g. "1 waiting on you". Handed in
     rather than worked out here so the header's summary and this tooltip come
     from the one module — see `projectState.js`. */
  stateLabel: { type: String, default: 'idle' },
  branch: { type: String, default: '' }
})

const emit = defineEmits(['select', 'menu'])

const { hover, handlers } = useInteractive()

/* The two numbers the design record settles outright: a 28px tile and an 8px
   dot. Neither is a token because neither is a step on any scale — the rail is
   fixed chrome, for the reason `PROJECT_RAIL` gives, and what sits in it is
   fixed with it. */
const SIZE = 28
const DOT = 8
/* How far the dot hangs off the tile's corner, which is also the width of the
   ring that cuts it out of the rail — half of it inside the tile, half out. */
const OVERHANG = 2

const tileStyle = computed(() => ({
  position: 'relative',
  width: `${SIZE}px`,
  height: `${SIZE}px`,
  padding: 0,
  display: 'grid',
  placeItems: 'center',
  borderRadius: 'var(--radius-3)',
  border: `var(--border-w) solid ${props.active ? 'var(--action-primary-bg)' : 'var(--border)'}`,
  background: props.active
    ? hover.value
      ? 'var(--action-primary-bg-hover)'
      : 'var(--action-primary-bg)'
    : hover.value
      ? 'var(--surface-active)'
      : 'var(--surface-raised)',
  color: props.active
    ? 'var(--action-primary-fg)'
    : hover.value
      ? 'var(--text-primary)'
      : 'var(--text-secondary)',
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  /* Taken off deliberately; see the note at the top of this file. */
  outline: 'none',
  cursor: 'default',
  transition: 'var(--transition-control)'
}))

const dotStyle = computed(() => ({
  position: 'absolute',
  right: `${-OVERHANG}px`,
  bottom: `${-OVERHANG}px`,
  width: `${DOT}px`,
  height: `${DOT}px`,
  borderRadius: 'var(--radius-pill)',
  /* Cut out of the rail's own ground rather than the tile's: the dot straddles
     the tile's corner, and a ring in the tile's colour would draw a notch in
     the rail whenever the tile is selected. */
  border: 'var(--border-w-strong) solid var(--surface-sunken)',
  background:
    props.state === 'loud'
      ? 'var(--attn-loud)'
      : props.state === 'live'
        ? 'var(--attn-live)'
        : 'var(--border-strong)',
  /* Only the live dot moves, and `tokens/motion.css` stops even that under
     `prefers-reduced-motion`. */
  animation:
    props.state === 'live' ? 'sm-pulse var(--dur-pulse) var(--ease-in-out) infinite' : undefined
}))

const label = computed(() => monogram(props.project.name))

const hint = computed(() =>
  [
    props.project.name,
    props.branch,
    props.stateLabel,
    props.project.tracked === false ? 'no bd tracker' : ''
  ]
    .filter(Boolean)
    .join(' · ')
)

/* Ctrl+click is the secondary click on macOS, and WebKit — which is the engine
   this app actually runs in — dispatches a `click` for it as well as a
   `contextmenu`, where Chromium suppresses the click. Without this guard the
   menu opens on a tile and the active project switches under it a moment later:
   the panel was drawn for a project the window was not pointed at and silently
   turns into another one's. `ProjectList` carried the same guard, and the same
   note about where it can be seen — only `npm run tauri dev` on macOS, never
   `?view=gallery`, which is Chromium and sends no click at all. */
const onClick = (event) => {
  if (event.ctrlKey) return
  emit('select', props.project.path)
}
</script>

<template>
  <!-- `flex: 0 0 auto` on the tooltip rather than on the button: the tooltip's
       own span is the rail's flex child, and a column of tiles that could shrink
       would draw ovals once the list is longer than the rail is tall. -->
  <Tooltip :label="hint" side="right" :style="{ flex: '0 0 auto' }">
    <!-- `.prevent` stops the browser's own menu over this one; it stops no
         propagation, which is what `src/nativeMenu.js` still needs. -->
    <button
      type="button"
      :style="tileStyle"
      v-bind="handlers"
      @click="onClick"
      @contextmenu.prevent="emit('menu', project, $event)"
    >
      {{ label }}
      <span :style="dotStyle" />
    </button>
  </Tooltip>
</template>
