<script setup>
/* A thinking block, folded.

   Folded by default: this is the agent working something out, not the agent
   saying something, and a panel that opened every one of them would bury the
   turn's actual answer. Codex emits these; Claude Code does not, so on a
   Claude session the component simply never appears.

   `details[data-reasoning]` (`docs/design_handoff_conversation_panel/markup-contract.md`,
   section 6) is `sm-prose.css`'s own disclosure, not a `Button`-shaped one: no
   `:style` on the root or on `summary` any more, and no chevron `Icon` either —
   the marker is drawn in CSS, two borders rotated 45°, because the platform
   triangle is hidden (`list-style:none` on `summary`,
   `::-webkit-details-marker{display:none}`). It is dimmed by colour and size
   rather than by opacity, which is why there is no `data-attention="quiet"`
   here any more either — that pairing was this component's own, and the
   contract's version clears the contrast floor when open, which stacking
   `--attn-quiet-opacity` on top of `--text-muted` would not have.

   The text is markdown, drawn by the shared component like every other piece of
   prose here, and `open` is forwarded for the reason `AgentMessage.vue` gives:
   a link inside reasoning is still a link, and it must leave for the person's
   own browser rather than replace the app.

   `summary` says `Reasoning`, the word the contract's own example spells —
   `<summary>Reasoning<time>18s</time></summary>` — and not this component's
   older `Thinking`. There is deliberately no `<time>` here yet: the elapsed
   clock is section 6's "the agent is working" strip, which this task does not
   own (smetana-epzb), and this component has no elapsed value to put in one —
   `expanded` is the only prop past the text. Whoever picks that task up adds
   it here rather than finding a `<summary>` that already looks finished
   without it.

   The `.sm-prose` here is interim too, for the reason `AgentMessage.vue`'s
   header gives: the contract's own root is the journal `ConversationView.vue`
   draws, which is smetana-e3mc's, not this task's. Until it lands this block
   carries its own `.sm-prose` around its one `details` child. */
import { ref, toRef, watch } from 'vue'
import Markdown from '../markdown/Markdown.vue'

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

/* `<details>` toggles itself natively on a click of `<summary>`; this only
   keeps `open` — and so `:open` below — in step with what the element just
   did, the same controlled-native-element shape `v-model` uses elsewhere. */
function onToggle(event) {
  open.value = event.target.open
}
</script>

<template>
  <div class="sm-prose">
    <details data-reasoning :open="open" @toggle="onToggle">
      <summary>Reasoning</summary>
      <Markdown :text="text" @open="emit('open', $event)" />
    </details>
  </div>
</template>
