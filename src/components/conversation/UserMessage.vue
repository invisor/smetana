<script setup>
/* What the person said, and what went with it.

   The two halves of a conversation are told apart **by shape rather than by
   colour**: `sm-prose.css` gives `article[data-turn="person"]` a raised
   surface, held off the right edge, with its bottom-right corner squared —
   the corner is the tail, and it points back at the sender. See
   `docs/design_handoff_conversation_panel/markup-contract.md`, section 1;
   colour would have to come out of the status range, which is spoken for, and
   the panel's one loud thing is the permission request — a person's own
   sentence is not an alert.

   The words are markdown too. A person pastes a path, a fenced snippet, a list
   of three things, and the agent receives it as markdown, so drawing it as
   anything else would show one thing and send another. `open` is forwarded for
   the reason `AgentMessage.vue` gives.

   An attachment is a path and nothing more — that is what `session_send` sends
   and what the journal keeps (`EventKind::UserMessage`). No thumbnail, then:
   the bytes behind the path are the desktop's to read, and a component that
   asked for them would be a component `?view=gallery` cannot draw with nothing
   behind it. The contract's `ul[data-attachments]` is mono chips carrying the
   file name and nothing else — no glyph, so no `Icon` here any more — always
   last inside the person's own `article`, which is what lets it and the prose
   above it share one surface instead of sitting in two boxes. `basename` is
   `src/paths.js`'s and is deliberately not written out again here — that
   function was three disagreeing copies once.

   The `.sm-prose` here is interim, for the reason `AgentMessage.vue`'s header
   gives: the contract's own root is the journal `ConversationView.vue` draws,
   which is smetana-e3mc's, not this task's. Until it lands each message
   carries its own `.sm-prose`, one turn per root. */
import Markdown from '../markdown/Markdown.vue'
import { basename } from '../../paths.js'

defineProps({
  text: { type: String, default: '' },
  /* Paths, in the order they were attached. */
  attachments: { type: Array, default: () => [] }
})

const emit = defineEmits(['open'])
</script>

<template>
  <div class="sm-prose">
    <article data-turn="person">
      <Markdown :text="text" @open="emit('open', $event)" />
      <ul v-if="attachments.length" data-attachments>
        <li v-for="path in attachments" :key="path">{{ basename(path) }}</li>
      </ul>
    </article>
  </div>
</template>
