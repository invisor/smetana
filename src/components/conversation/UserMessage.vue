<script setup>
/* What the person said, and what went with it.

   The two halves of a conversation are told apart **by shape rather than by
   colour**: a raised surface with a bar down its left edge, against the plain
   ground `AgentMessage.vue` draws on. Colour would have to come out of the
   status range, which is spoken for, and the panel's one loud thing is the
   permission request — a person's own sentence is not an alert.

   The words are markdown too. A person pastes a path, a fenced snippet, a list
   of three things, and the agent receives it as markdown, so drawing it as
   anything else would show one thing and send another. `open` is forwarded for
   the reason `AgentMessage.vue` gives.

   An attachment is a path and nothing more — that is what `session_send` sends
   and what the journal keeps (`EventKind::UserMessage`). No thumbnail, then:
   the bytes behind the path are the desktop's to read, and a component that
   asked for them would be a component `?view=gallery` cannot draw with nothing
   behind it. `basename` is `src/paths.js`'s and is deliberately not written out
   again here — that function was three disagreeing copies once. */
import Icon from '../core/Icon.vue'
import Markdown from '../markdown/Markdown.vue'
import { basename } from '../../paths.js'

defineProps({
  text: { type: String, default: '' },
  /* Paths, in the order they were attached. */
  attachments: { type: Array, default: () => [] }
})

const emit = defineEmits(['open'])

const style = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  padding: 'var(--space-5) var(--panel-pad)',
  background: 'var(--surface-raised)',
  borderLeft: 'var(--accent-bar-w) solid var(--border-strong)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)'
}

const strip = { display: 'flex', flexWrap: 'wrap', gap: 'var(--space-2)' }

/* A file's name is an identifier, so mono; the chip is a label rather than a
   control, so it takes no hover and no focus. */
const chip = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  maxWidth: '100%',
  minWidth: 0,
  height: 'var(--control-h-sm)',
  padding: '0 var(--space-3)',
  background: 'var(--surface)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)'
}

const chipName = { minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }
</script>

<template>
  <div :style="style">
    <Markdown :text="text" @open="emit('open', $event)" />
    <div v-if="attachments.length" :style="strip">
      <span v-for="path in attachments" :key="path" :style="chip">
        <Icon name="paperclip" :size="11" />
        <span :style="chipName">{{ basename(path) }}</span>
      </span>
    </div>
  </div>
</template>
