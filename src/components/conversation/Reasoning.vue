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
   older `Thinking`. **The `<time>` is here now** (smetana-epzb): a reasoning
   block carries no duration of its own on the wire —
   `session::model::EventKind::Reasoning` is `{ text }`, full stop — so what it
   shows is `journal.js`'s own `elapsedSince`, the gap between the turn's own
   `turn-start` and this event's timestamp, which is the honest reading of "how
   long the agent had been going when it said this" available from two
   timestamps the wire already sends. `ms` is `null` where there was no open
   turn to measure against, which draws no `<time>` at all rather than a
   guessed zero — the same omission `TurnResult.vue` gives a cost the harness
   did not report. The spelling is `elapsed.js`'s `formatElapsedClock`, the
   same compact, whole-second voice the strip's own `waiting` and `failed`
   moments use, and deliberately not `formatReceiptDuration`'s decimal one: this is a
   number read once, after the fact, the way `waiting`'s ticking clock is read
   while it moves, not the receipt's own tenth-of-a-second precision.

   `.sm-prose` no longer wraps this block, for the reason `AgentMessage.vue`'s
   header gives: the contract's own root is the journal `ConversationView.vue`
   draws now, so `details[data-reasoning]` is emitted bare, as a direct
   sibling of the other turns under that one root, rather than inside a second
   `.sm-prose` of its own. */
import { computed, ref, toRef, watch } from 'vue'
import Markdown from '../markdown/Markdown.vue'
import { formatElapsedClock } from './elapsed.js'

const props = defineProps({
  text: { type: String, default: '' },
  /* How long the turn had been going when this was said, in milliseconds —
     `journal.js`'s `elapsedSince`, or `null` where there was no open turn to
     measure against. */
  ms: { type: Number, default: null },
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

const elapsedText = computed(() => (props.ms == null ? '' : formatElapsedClock(props.ms)))
</script>

<template>
  <details data-reasoning :open="open" @toggle="onToggle">
    <summary>Reasoning<time v-if="elapsedText">{{ elapsedText }}</time></summary>
    <Markdown :text="text" @open="emit('open', $event)" />
  </details>
</template>
