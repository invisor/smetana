<script setup>
/* A turn of the agent's prose, drawn as the markdown it arrived as.

   There is no second parser and no second pair of components: the text goes
   straight to `markdown/Markdown.vue`, which the task inspector already draws,
   so a fenced block here is the `pre > code` in `var(--font-mono)` that file
   already makes of one. Syntax highlighting is deliberately not this
   component's — CodeMirror was taken off the plan for it.

   `sm-prose.css` (`docs/design_handoff_conversation_panel/markup-contract.md`,
   section 1) owns the whole visual: `.sm-prose` on the root for the panel's
   padding and type, `article[data-turn="agent"]` on the turn itself for "no
   container, no avatar, no caption" — the plain ground the panel is measured
   against, so a row of "agent" labels down the panel would say the same thing
   a second time. No `:style` on either element; nothing here is a colour, a
   radius or a spacing value any more, it is markup over a known contract.

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
</script>

<template>
  <div class="sm-prose">
    <article data-turn="agent">
      <Markdown :text="text" @open="emit('open', $event)" />
    </article>
  </div>
</template>
