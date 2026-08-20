<script setup>
import { computed } from 'vue'
import IconButton from '../core/IconButton.vue'
/* The rail's width is a layout rule rather than a look, and the rules live in
   one place because `panelWidths.js` does arithmetic with this number — what a
   collapsed neighbour costs, where a drag folds and unfolds. It was written out
   here as a `32px` literal as well, which is the same number in two files. */
import { RAIL, RAIL_CONTROL_MAX } from '../../views/panelWidths.js'

const props = defineProps({
  title: { type: String, required: true },
  /* One line under the title, in muted micro type — what this panel is about
     right now, e.g. "develop · 1 running". Its presence is also what
     switches the title from a section header's uppercase micro-caps to a name
     in ordinary mono: a project is *called* something, a section is labelled. */
  subtitle: { type: String, default: '' },
  side: { type: String, default: 'left' },
  collapsed: { type: Boolean, default: false },
  collapsible: { type: Boolean, default: true },
  /* What the header button says. It always means "close one step further" — the
     icon says so and does not change — but how far the next press goes is a
     caller's business: the left column walks a cycle and hides the project rail
     before it folds itself, so it words the two steps differently. A caller
     that says nothing gets "Collapse <side> panel". */
  toggleLabel: { type: String, default: '' },
  /* The same, for the button inside the folded rail. It exists because that
     button can bring back more than this panel: in the left column it returns
     the project rail with it, and a label saying only "Expand" would understate
     what pressing it does. */
  expandLabel: { type: String, default: '' }
})

/* Two events rather than one, and the split is load-bearing: `toggle` is the
   header button, drawn only while the panel is open, and `expand` is the button
   in the collapsed rail, drawn only while it is folded. They mean different
   steps of the left column's cycle — one closes further, the other opens
   everything — and one event for both had the folded column's button hiding the
   projects instead of unfolding. */
defineEmits(['toggle', 'expand'])

const edge = computed(() => ({
  borderRight: props.side === 'left' ? 'var(--border-w) solid var(--border)' : undefined,
  borderLeft: props.side === 'right' ? 'var(--border-w) solid var(--border)' : undefined
}))

const collapsedStyle = computed(() => ({
  width: `${RAIL}px`,
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: 'var(--space-3)',
  padding: 'var(--space-3) 0',
  background: 'var(--surface)',
  ...edge.value
}))

const style = computed(() => ({
  display: 'flex',
  flexDirection: 'column',
  minWidth: 0,
  minHeight: 0,
  background: 'var(--surface)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  ...edge.value
}))

/* The tab row's height is the floor rather than the height: the column headers
   line up with it, and a couple of pixels' difference reads as a misalignment
   rather than an accent — but two lines of type do not fit inside one tab, so a
   header carrying a subtitle grows past it instead of clipping. */
const headerStyle = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: props.subtitle ? 'auto' : 'var(--tab-h)',
  minHeight: 'var(--tab-h)',
  flex: '0 0 auto',
  padding: props.subtitle
    ? 'var(--space-3) var(--space-3) var(--space-3) var(--space-5)'
    : '0 var(--space-3) 0 var(--space-5)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}))
const headingStyle = {
  flex: 1,
  minWidth: 0,
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-1)'
}
/* A name rather than a label: 12px mono medium, not uppercase. */
const namedStyle = {
  font: 'var(--weight-medium) var(--text-sm)/1 var(--font-mono)',
  color: 'var(--text-primary)',
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
}
const subtitleStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
}
/* 10px uppercase mono: a label, not a sentence. The width and the ellipsis are
   the heading box's now, since the subtitle under it needs the same. */
const titleStyle = {
  fontSize: 'var(--text-2xs)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)',
  fontWeight: 'var(--weight-medium)',
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
}
/* Capped, not scaled: `--control-h-sm` grows with the app-wide font size and the
   rail does not, so at the top of the range this button was drawn 44px wide
   inside a 32px strip and hung over the column beside it. `min()` keeps both
   densities exactly as they are at the shipped size — 24 comfortable, 20 compact
   — and stops the growth at the rail's edge. The reasoning, and why the rail
   itself cannot grow, is on `RAIL_CONTROL_MAX`. */
const railButtonStyle = {
  width: `min(var(--control-h-sm), ${RAIL_CONTROL_MAX}px)`,
  height: `min(var(--control-h-sm), ${RAIL_CONTROL_MAX}px)`
}
/* One direction, one icon. The header button is only ever drawn on an open
   panel and only ever closes it further, so it points at its own edge whatever
   step the caller is on; the opening glyph belongs to the rail's button below,
   which is the only one that ever opens anything. */
const toggleIcon = computed(() =>
  props.side === 'left' ? 'panel-left-close' : 'panel-right-close'
)
const toggleText = computed(() => props.toggleLabel || `Collapse ${props.side} panel`)
const expandText = computed(() => props.expandLabel || `Expand ${props.side} panel`)

/* The folded rail's own copy of the title, and it follows the header's rule
   rather than keeping its own: a subtitle means the title is a *name*, and a
   name is not shouted. Uppercasing "smetana" into "SMETANA" would undo in the
   rail exactly the distinction the header is drawn to make. The size and the
   muted colour stay the rail's — 32px of folded chrome is quiet whatever is
   written down it. */
const railTitleStyle = computed(() => ({
  writingMode: 'vertical-rl',
  fontSize: 'var(--text-2xs)',
  letterSpacing: props.subtitle ? 'var(--tracking-normal)' : 'var(--tracking-caps)',
  textTransform: props.subtitle ? 'none' : 'uppercase',
  fontFamily: props.subtitle ? 'var(--font-mono)' : undefined,
  fontWeight: props.subtitle ? 'var(--weight-medium)' : undefined,
  color: 'var(--text-muted)'
}))
</script>

<template>
  <div v-if="collapsed" :style="collapsedStyle">
    <IconButton
      :icon="side === 'left' ? 'panel-left-open' : 'panel-right-open'"
      :label="expandText"
      size="sm"
      :style="railButtonStyle"
      @click="$emit('expand')"
    />
    <div :style="railTitleStyle">{{ title }}</div>
  </div>
  <div v-else :style="style">
    <div :style="headerStyle">
      <div :style="headingStyle">
        <div :style="subtitle ? namedStyle : titleStyle">{{ title }}</div>
        <div v-if="subtitle" :style="subtitleStyle">{{ subtitle }}</div>
      </div>
      <!-- Before `actions`, so a warning glyph never moves when a button
           appears beside it or goes. -->
      <slot name="marks" />
      <slot name="actions" />
      <IconButton
        v-if="collapsible"
        :icon="toggleIcon"
        :label="toggleText"
        size="sm"
        @click="$emit('toggle')"
      />
    </div>
    <div :style="{ flex: 1, minHeight: 0, overflow: 'auto' }">
      <slot />
    </div>
    <div
      v-if="$slots.footer"
      :style="{ flex: '0 0 auto', padding: 'var(--space-3) var(--space-5)', borderTop: 'var(--border-w) solid var(--border-subtle)' }"
    >
      <slot name="footer" />
    </div>
  </div>
</template>
