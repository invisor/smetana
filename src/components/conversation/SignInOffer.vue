<script setup>
/* The way out of a dead end: the agent's last turn failed because the person is
   signed out, and a composer that sends another message into the same error is
   the only other control on the panel.

   A row of buttons and nothing else, drawn under the journal line that says so
   (`ConversationView.vue` decides which line — `signInHint.js`). Each one asks
   for the harness's own login dialog in a terminal tab; this component neither
   knows how that is started nor draws any of it. The first variant leads as the
   primary, every other is secondary, which is `PermissionRequest`'s idiom for
   the same reason: one obvious default, no hue.

   Quiet, not `loud`. The failure line above is already drawn at `failed`, and
   `loud` is budgeted at one or two rows a screen; an offer to fix it does not
   need to shout over the thing it fixes.

   Which buttons exist is the catalogue's answer (`signIn` on a harness's
   capabilities), handed in as `variants`; an empty list draws nothing at all. */
import { computed } from 'vue'
import Button from '../core/Button.vue'
import { signInLabel } from './signInHint.js'

const props = defineProps({
  /* The harness id whose session failed — `claude` or `codex`. */
  agent: { type: String, required: true },
  /* Any of `browser`, `deviceCode`, in the order they are drawn. */
  variants: { type: Array, default: () => [] }
})

defineEmits(['sign-in'])

const row = {
  display: 'flex',
  flexWrap: 'wrap',
  gap: 'var(--space-3)',
  padding: 'var(--space-3) 0'
}

const label = (variant) => signInLabel(props.agent, variant, props.variants.length)

const shown = computed(() => props.variants.length > 0)
</script>

<template>
  <div v-if="shown" :style="row">
    <Button
      v-for="(variant, index) in variants"
      :key="variant"
      size="sm"
      :variant="index === 0 ? 'primary' : 'secondary'"
      @click="$emit('sign-in', variant)"
    >{{ label(variant) }}</Button>
  </div>
</template>
