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
   anything else would show one thing and send another. `open`, `open-local`
   and `open-image` are forwarded for the reason `AgentMessage.vue` gives — a
   person's own message can carry a local path or a figure exactly as the
   agent's can, and `root` is what makes a link operable.

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

   `.sm-prose` no longer wraps this message: the contract's own root is the
   journal `ConversationView.vue` draws, one flex column over every turn, and
   this component emits its bare `article` as a direct child of it. A wrapper
   here would nest one `.sm-prose` inside another and double the block rhythm
   the outer one already spends — `d865ac7`'s fix to `TaskInspector.vue` is the
   same mistake caught on a different root.

   A chip is a `button` now (smetana-4x3w) — when there is somewhere for a
   click to go. The whole of it, since a sent attachment carries no cross to
   sit beside and nothing here removes anything: clicking the one control
   raises `open-attachment` with the bare path, exactly what `Composer`'s own
   name button does before a message is sent. That is only ever true for
   *some* attachments, though — a picture opens in the image window wherever
   it sits, but anything else needs an editor tab, and there is one only for
   a path inside the open project (see `attachmentAction.js` beside
   `ConversationView.vue` for the full rule). This is a **library**
   component, exported from `components/index.js` and drawn in the gallery
   with no store behind it, so it may not work either half of that out for
   itself: `isImagePath` lives in `stores/attachments.js`, a store that knows
   Tauri exists, per this file's own note on `open`/`open-local`/`open-image`
   above and the identical rule stated in `Markdown.vue`'s own header, and
   knowing the project's own root is no better — that would still need
   `relativeTo` run against a value this component has no business reading
   off a store either. So `openAttachments` arrives as a prop instead, the
   subset of `attachments` `ConversationView.vue` has already decided is
   worth a click, and an attachment outside it is drawn as plain text: no
   button, no cursor, no hover, and nothing bound to a click or a key —
   `files_read` has nothing to open such a path into and `openExternal`'s own
   allow-list is `http`/`https` alone, so a control with nowhere to send a
   click would be the one lie a disabled state elsewhere in this system never
   tells.

   `aria-label="Open …"` carries a verb the visible name — ellipsized, and
   with no cross beside it here to read as a second action — does not. Not
   the native `title`: `IconButton.vue` rules that out for this system. */
import Markdown from '../markdown/Markdown.vue'
import { basename } from '../../paths.js'

defineProps({
  text: { type: String, default: '' },
  /* Paths, in the order they were attached. */
  attachments: { type: Array, default: () => [] },
  /* The subset of `attachments` worth drawing as a control — see the header
     above for why this component cannot work that out for itself. */
  openAttachments: { type: Array, default: () => [] },
  /* Passed straight through to `Markdown`, unread here — see its own header. */
  root: { type: String, default: '' },
  /* Also passed straight through, unread here: the session's own cwd — see
     `AgentMessage.vue`'s own header for why this rides beside `root` rather
     than being left to `Markdown.vue`'s fallback. */
  base: { type: String, default: '' }
})

const emit = defineEmits([
  'open',
  'open-local',
  'open-image',
  /* The bare path, raised only for an attachment `openAttachments` names —
     `ConversationView.vue` decides whether it is a picture or a file, or
     nothing at all, once it arrives. See the header above. */
  'open-attachment'
])
</script>

<template>
  <article data-turn="person">
    <Markdown
      :text="text"
      :root="root"
      :base="base"
      @open="emit('open', $event)"
      @open-local="emit('open-local', $event)"
      @open-image="emit('open-image', $event)"
    />
    <ul v-if="attachments.length" data-attachments>
      <li v-for="path in attachments" :key="path">
        <button
          v-if="openAttachments.includes(path)"
          type="button"
          :aria-label="`Open ${basename(path)}`"
          @click="emit('open-attachment', path)"
        >{{ basename(path) }}</button>
        <span v-else data-attachment-name>{{ basename(path) }}</span>
      </li>
    </ul>
  </article>
</template>
