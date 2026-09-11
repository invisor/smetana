<script setup>
/* One of bd's prose fields, drawn as the markdown it is.

   The rules are all in `markdown.js`; this file only turns a block into the
   element `sm-prose.css` (`docs/design_handoff_conversation_panel/markup-contract.md`,
   section 2) already knows how to paint. No class and no `:style` on anything
   below — the styling keys on the element name and on the handful of `data-`
   attributes the contract names, and the whole tree is meant to sit inside an
   ancestor carrying `class="sm-prose"`, which this component does not draw
   itself: a quote and a list item are prose without being a root, and the root
   belongs to whatever wraps a whole message or a whole field.

   The template has no wrapping element of its own, on purpose: a `<div>` around
   the blocks would sit between, say, `<article data-turn>` and its `<p>`
   children, and `sm-prose.css`'s `li > :where(p,pre,blockquote,…)` and
   `:first-child`/`:last-child` rules select on that direct relationship. Vue 3
   templates may have more than one root node, which is what lets the blocks
   below land as direct children of whatever this is used inside.

   No `v-html`, deliberately and permanently: the tree is drawn as Vue nodes, so
   an issue's text can never become markup and no sanitiser is needed. A link is
   emitted rather than opened, the way `AboutSettings` does it — the library
   knows nothing about Tauri, and the view binds the app's one link path.

   Read-only, all of it. A task item's box is drawn by `sm-prose.css` off
   `li[data-checked]`, never a Lucide glyph and never `<input type="checkbox"
   disabled>` — see the contract for why the native control is refused.

   The one control on the tree — the code block's copy button (contract
   section 4) — stays read-only in the same sense: it acts on the clipboard,
   never on the prose, and answers with nothing this component keeps. It calls
   the browser's own `navigator.clipboard` directly rather than importing a
   store: this file draws markup for five different owners (the task inspector,
   three turn kinds of the conversation panel, the gallery) and must never
   import `@tauri-apps/api` itself, and threading a callback through every one
   of those five just to prefer `stores/app.js`'s `copyText` — which takes the
   very same browser branch outside a Tauri build — is a cost with no payoff
   anywhere but the live app. An owner that already imports a store, such as
   `conversation/ConversationView.vue`, is free to override that later if the
   plugin's better reliability on a real WebKitGTK build turns out to matter
   here too; nothing below forecloses it. */
import { computed, onBeforeUnmount, ref } from 'vue'
import Icon from '../core/Icon.vue'
import MarkdownInline from './MarkdownInline.vue'
import { parseMarkdown } from './markdown.js'

const props = defineProps({
  /* The source. Ignored when `blocks` is given, which is what the recursive
     calls below pass — a quote and a list item are already parsed, and parsing
     them again would be the same work done twice per level of nesting. */
  text: { type: String, default: '' },
  blocks: { type: Array, default: null }
})

const emit = defineEmits(['open'])

const tree = computed(() => props.blocks ?? parseMarkdown(props.text))

/* `data-task` lives on the `<ul>`, not per item, so it is decided once for the
   whole list, off a single item — `markdown.js`'s `takeList` is what actually
   decides this, and its guarantee is what makes checking one enough: a list's
   items carry a checked state either all together or not at all, never a mix,
   so `sm-prose.css`'s unconditional `ul[data-task] > li::before` never lands
   on a plain bullet. This file does not re-derive that homogeneity, only
   trusts it — an ordered list is never a task list either, for the same
   reason `takeList` refuses one: the contract's box is `ul[data-task]` only. */
function isTaskList(block) {
  return block.items.length > 0 && block.items[0].checked !== null
}

/* The copy control's confirmation window, per the contract. Deliberately its
   own number and not `kanban/copyId.js`'s `COPIED_MS`: that one is 1200ms,
   tuned for a task's id, and a different duration is a different policy, not
   a variant of the same one — borrowing it would tie this control's timing to
   a change made for that one's sake. */
const COPY_REVERT_MS = 1600

/* Which block in *this* instance's own flat list last showed "Copied", by
   its `v-for` index — `null` when none has. One ref for the whole tree rather
   than one per block: a press can only ever come from one button at a time, so
   a single "who last succeeded" is the whole of the state a list of blocks
   needs, the same shape `useCopyFeedback`'s single `target` takes for a list of
   rows. A quote or a list item recurses into its own `Markdown` instance with
   its own copy of this ref, so two code blocks in two different quotes are
   free to say "Copied" at the same moment — nothing in the contract asks for
   one confirmation across a whole document, only one per control. */
const copiedIndex = ref(null)
let copyRevertTimer = null

async function copyCode(index, text) {
  clearTimeout(copyRevertTimer)
  try {
    await navigator.clipboard.writeText(text)
    copiedIndex.value = index
    copyRevertTimer = setTimeout(() => {
      copiedIndex.value = null
    }, COPY_REVERT_MS)
  } catch (err) {
    // Left at rest: a control that claimed success it did not have would be
    // worse than one that stays silent about a clipboard it could not reach.
    console.error('[markdown] the code block did not reach the clipboard:', err)
  }
}

// The one piece of state this file owns outright — cleared so an unmounted
// panel never fires a reset into a component that is no longer there.
onBeforeUnmount(() => clearTimeout(copyRevertTimer))
</script>

<template>
  <template v-for="(block, index) in tree" :key="index">
    <component :is="`h${block.level}`" v-if="block.type === 'heading'">
      <MarkdownInline :nodes="block.children" @open="emit('open', $event)" />
    </component>

    <p v-else-if="block.type === 'paragraph'">
      <MarkdownInline :nodes="block.children" @open="emit('open', $event)" />
    </p>

    <figure v-else-if="block.type === 'code'" data-code :data-lang="block.lang || undefined">
      <figcaption v-if="block.lang">{{ block.lang }}</figcaption>
      <button
        type="button"
        data-copy
        :data-state="copiedIndex === index ? 'copied' : 'idle'"
        aria-label="Copy code"
        @click="copyCode(index, block.text)"
      >
        <Icon name="copy" data-icon="copy" />
        <Icon name="check" data-icon="check" />
        <span>{{ copiedIndex === index ? 'Copied' : 'Copy' }}</span>
      </button>
      <pre><code>{{ block.text }}</code></pre>
    </figure>

    <hr v-else-if="block.type === 'rule'" />

    <blockquote v-else-if="block.type === 'quote'">
      <Markdown :blocks="block.blocks" @open="emit('open', $event)" />
    </blockquote>

    <component
      :is="block.ordered ? 'ol' : 'ul'"
      v-else-if="block.type === 'list'"
      :start="block.ordered && block.start !== 1 ? block.start : undefined"
      :data-task="isTaskList(block) ? '' : undefined"
    >
      <li
        v-for="(entry, at) in block.items"
        :key="at"
        :data-checked="entry.checked ? '' : undefined"
      >
        <Markdown :blocks="entry.blocks" @open="emit('open', $event)" />
      </li>
    </component>

    <dl v-else-if="block.type === 'dl'">
      <template v-for="(item, at) in block.items" :key="at">
        <dt><MarkdownInline :nodes="item.term" @open="emit('open', $event)" /></dt>
        <dd v-for="(definition, d) in item.definitions" :key="d">
          <MarkdownInline :nodes="definition" @open="emit('open', $event)" />
        </dd>
      </template>
    </dl>
  </template>
</template>
