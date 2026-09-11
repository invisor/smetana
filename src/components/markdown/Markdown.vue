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
   knows nothing about Tauri, and the view binds the app's own verbs to the
   two events `MarkdownInline.vue` raises: `open` for an external link, and
   `open-local` for one that names a path on this machine. `root`, threaded
   down to every `MarkdownInline` below, is that same split's other half — see
   that file's own header for what it is and what an empty one does.

   Read-only, all of it. A task item's box is drawn by `sm-prose.css` off
   `li[data-checked]`, never a Lucide glyph and never `<input type="checkbox"
   disabled>` — see the contract for why the native control is refused.

   The one control on the tree — the code block's copy button (contract
   section 4) — stays read-only in the same sense: it acts on the clipboard,
   never on the prose, and answers with nothing this component keeps for
   itself, past the confirmation on the one button somebody pressed.

   Its state and its timer are `core/copyFeedback.js`'s `useCopyFeedback`, not
   a copy of that policy written out again — that file's own header narrates
   what a fifth copy of "clear the timer, claim the target, await the write,
   bail if a later press has taken it over, clear the timer a second time" has
   already cost this tree twice, and there is no reason to find out a fifth
   time what a fourth already proved. The one place this call site differs
   from every other is the duration: the contract's own 1600ms rather than
   `COPIED_MS`'s 1200, passed as that composable's second argument — see
   `kanban/copyId.js`'s header for why the two numbers are allowed to differ.

   The clipboard writer is `inject('smCopyText', …)`, the same shape
   `overlays/Modal.vue` reaches `views/DialogWindow.vue` through
   (`smDialogWindow`, `smDialogFill`, `smDialogClosable`): a library component
   reaching an ancestor that owns a store without importing one itself. This
   file draws markup for whichever view ends up rendering it — today
   `views/DesktopApp.vue` and `views/Gallery.vue`, the only two that draw
   anything with a `Markdown` under it — and must never import
   `@tauri-apps/api` on its own account, so the default below, wired to
   nothing, is the browser's own `navigator.clipboard`. Both of those views
   already import `stores/app.js`'s `copyText`, which prefers the Tauri
   clipboard plugin for the reason that store's own header gives —
   `navigator.clipboard` wants a secure context and a gesture the webview does
   not always agree it had, a failure with no visible cause in the packaged
   app alone — so both `provide('smCopyText', copyText)`, and the default
   below is what a future owner falls back to until it does the same. */
import { computed, inject } from 'vue'
import Icon from '../core/Icon.vue'
import MarkdownInline from './MarkdownInline.vue'
import { parseMarkdown } from './markdown.js'
import { useCopyFeedback } from '../core/copyFeedback.js'

const props = defineProps({
  /* The source. Ignored when `blocks` is given, which is what the recursive
     calls below pass — a quote and a list item are already parsed, and parsing
     them again would be the same work done twice per level of nesting. */
  text: { type: String, default: '' },
  blocks: { type: Array, default: null },
  /* The active project's absolute path, or `''` where there is none — see
     `MarkdownInline.vue`'s own header. Passed straight through to every node
     this file draws and to the recursive calls below, since a quote or a list
     item is the same prose at one remove and owes its own links the same
     answer to "is there anything here that can open one". */
  root: { type: String, default: '' }
})

const emit = defineEmits(['open', 'open-local'])

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

/* Wrapped rather than passed bare so it always answers `Promise<boolean>`,
   `copyText`'s own shape and what `useCopyFeedback` requires of `write` —
   `navigator.clipboard.writeText` alone resolves `undefined` on success and
   rejects on failure, neither of which is a boolean. */
async function browserCopyText(text) {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch (err) {
    // Left at rest: a control that claimed success it did not have would be
    // worse than one that stays silent about a clipboard it could not reach.
    console.error('[markdown] the code block did not reach the clipboard:', err)
    return false
  }
}

const writeCode = inject('smCopyText', browserCopyText)

/* The contract's own confirmation window — see `kanban/copyId.js`'s header
   for why this is not `COPIED_MS`. */
const COPY_CODE_MS = 1600

/* One `useCopyFeedback` per `Markdown` instance, keyed on the `v-for` index
   below: a quote or a list item recurses into its own instance with its own
   copy of this state, so two code blocks in two different quotes are free to
   say "Copied" at the same moment — nothing in the contract asks for one
   confirmation across a whole document, only one per control, which is this
   composable's own "one target at a time" read one level narrower. The
   parser only ever appends blocks, so the index is a stable enough key for
   which one last said "Copied"; see `useCopyFeedback` for the rest of the
   policy this file no longer writes out by hand. */
const { stateFor: copyStateFor, copy: copyCode } = useCopyFeedback(writeCode, COPY_CODE_MS)

function isCodeCopied(index) {
  return copyStateFor(index) === 'copied'
}
</script>

<template>
  <template v-for="(block, index) in tree" :key="index">
    <component :is="`h${block.level}`" v-if="block.type === 'heading'">
      <MarkdownInline
        :nodes="block.children"
        :root="root"
        @open="emit('open', $event)"
        @open-local="emit('open-local', $event)"
      />
    </component>

    <p v-else-if="block.type === 'paragraph'">
      <MarkdownInline
        :nodes="block.children"
        :root="root"
        @open="emit('open', $event)"
        @open-local="emit('open-local', $event)"
      />
    </p>

    <figure v-else-if="block.type === 'code'" data-code :data-lang="block.lang || undefined">
      <figcaption v-if="block.lang">{{ block.lang }}</figcaption>
      <button
        type="button"
        data-copy
        :data-state="isCodeCopied(index) ? 'copied' : 'idle'"
        :aria-label="isCodeCopied(index) ? 'Copied' : 'Copy code'"
        @click="copyCode(index, block.text)"
      >
        <Icon name="copy" data-icon="copy" />
        <Icon name="check" data-icon="check" />
        <span>{{ isCodeCopied(index) ? 'Copied' : 'Copy' }}</span>
      </button>
      <pre><code>{{ block.text }}</code></pre>
    </figure>

    <hr v-else-if="block.type === 'rule'" />

    <blockquote v-else-if="block.type === 'quote'">
      <Markdown
        :blocks="block.blocks"
        :root="root"
        @open="emit('open', $event)"
        @open-local="emit('open-local', $event)"
      />
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
        <Markdown
          :blocks="entry.blocks"
          :root="root"
          @open="emit('open', $event)"
          @open-local="emit('open-local', $event)"
        />
      </li>
    </component>

    <dl v-else-if="block.type === 'dl'">
      <template v-for="(item, at) in block.items" :key="at">
        <dt>
          <MarkdownInline
            :nodes="item.term"
            :root="root"
            @open="emit('open', $event)"
            @open-local="emit('open-local', $event)"
          />
        </dt>
        <dd v-for="(definition, d) in item.definitions" :key="d">
          <MarkdownInline
            :nodes="definition"
            :root="root"
            @open="emit('open', $event)"
            @open-local="emit('open-local', $event)"
          />
        </dd>
      </template>
    </dl>
  </template>
</template>
