<script setup>
/* A turn of the agent's prose, drawn as the markdown it arrived as.

   There is no second parser and no second pair of components: the text goes
   straight to `markdown/Markdown.vue`, which the task inspector already draws,
   so a fenced block here is the `<pre>` in `var(--font-mono)` that file already
   makes of one. Syntax highlighting is deliberately not this component's —
   CodeMirror was taken off the plan for it.

   No glyph and no caption. The person's half of the conversation is told apart
   by shape — a raised surface with a bar down its left edge, see
   `UserMessage.vue` — so the agent's half is the plain ground everything else
   is measured against, and a row of "agent" labels down the panel would say the
   same thing a second time.

   `open` is forwarded rather than answered. `Markdown` opens no link itself: it
   raises the href at every level of its tree, and whatever draws this binds it
   to `openExternal` in `stores/app.js`, because a navigation inside the webview
   would replace the app. Binding `:text` alone ships an agent's prose with
   links that do nothing, and no test in this project can catch that. */
import Markdown from '../markdown/Markdown.vue'

defineProps({
  /* Markdown as the agent wrote it — `session::model::EventKind::Text`. */
  text: { type: String, default: '' }
})

const emit = defineEmits(['open'])

const style = {
  padding: 'var(--space-5) var(--panel-pad)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)'
}
</script>

<template>
  <div :style="style">
    <Markdown :text="text" @open="emit('open', $event)" />
  </div>
</template>
