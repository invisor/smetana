<script setup>
/* A thinking block, folded.

   Folded by default and `quiet` at every moment: this is the agent working
   something out, not the agent saying something, and a panel that opened every
   one of them would bury the turn's actual answer. Codex emits these; Claude
   Code does not, so on a Claude session the component simply never appears.

   `data-attention="quiet"` with `opacity: var(--attn-quiet-opacity)` is the
   system's third attention level written out — the same pair `StatusBadge`
   applies to `done`. The whole block dims, header included, so opening one
   costs no loudness at all.

   The text is markdown, drawn by the shared component like every other piece of
   prose here, and `open` is forwarded for the reason `AgentMessage.vue` gives:
   a link inside reasoning is still a link, and it must leave for the person's
   own browser rather than replace the app. */
import { ref, toRef, watch } from 'vue'
import Icon from '../core/Icon.vue'
import Markdown from '../markdown/Markdown.vue'
import { useInteractive } from '../core/interactive.js'

const props = defineProps({
  text: { type: String, default: '' },
  /* What it opens as, and nothing more — the fold is the component's own state
     from the first press onwards. Default `false`, so "folded by default" is
     the behaviour whether or not anybody passes this.

     It exists because `?view=gallery` is the only check these components get,
     and a state reachable only by clicking is a state the harness cannot show.
     `agent/ToolCall.vue` carries a prop of the same name with the same default
     and the same watch, and the gallery opens one of its three the same way.
     That file says nothing about why, so the shared motive is read off the two
     uses rather than quoted from it. */
  expanded: { type: Boolean, default: false }
})

const emit = defineEmits(['open'])

const open = ref(props.expanded)
watch(toRef(props, 'expanded'), (value) => { open.value = value })
const { hover, handlers } = useInteractive()

/* `--text-secondary` under the quiet opacity rather than `--text-muted` under
   it: the opacity is what makes this quiet, and stacking the two would put the
   header below the muted step the rest of the panel already calls faint. */
const root = {
  padding: 'var(--space-2) var(--panel-pad)',
  color: 'var(--text-secondary)',
  fontFamily: 'var(--font-sans)',
  opacity: 'var(--attn-quiet-opacity)'
}

/* A real button, so the fold is on the keyboard's path and wears the focus ring
   `tokens/base.css` draws. Hover is a surface step up and nothing else — never
   a colour and never a transform, so a row in a long journal cannot jump. */
const head = (hovered) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  width: '100%',
  height: 'var(--row-h)',
  padding: '0 var(--space-3)',
  background: hovered ? 'var(--surface-hover)' : 'transparent',
  border: 0,
  borderRadius: 'var(--radius-3)',
  color: 'var(--text-secondary)',
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-sans)',
  textAlign: 'left',
  cursor: 'default',
  transition: 'var(--transition-control)'
})

const body = { padding: 'var(--space-3) var(--space-3) var(--space-3) var(--space-6)' }
</script>

<template>
  <div data-attention="quiet" :style="root">
    <button
      type="button"
      :aria-expanded="open"
      :style="head(hover)"
      v-bind="handlers"
      @click="open = !open"
    >
      <Icon :name="open ? 'chevron-down' : 'chevron-right'" :size="12" />
      <span>Thinking</span>
    </button>
    <div v-if="open" :style="body">
      <Markdown :text="text" @open="emit('open', $event)" />
    </div>
  </div>
</template>
