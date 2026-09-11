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
   disabled>` — see the contract for why the native control is refused. */
import { computed } from 'vue'
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
</script>

<template>
  <template v-for="(block, index) in tree" :key="index">
    <component :is="`h${block.level}`" v-if="block.type === 'heading'">
      <MarkdownInline :nodes="block.children" @open="emit('open', $event)" />
    </component>

    <p v-else-if="block.type === 'paragraph'">
      <MarkdownInline :nodes="block.children" @open="emit('open', $event)" />
    </p>

    <pre v-else-if="block.type === 'code'"><code>{{ block.text }}</code></pre>

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

    <!-- The wrapper owns the border, the radius and the horizontal scroll
         (`markup-contract.md`, section 3), so the table itself never has to
         clip anything and the panel never has to grow. `data-wide` is set off
         the column count alone — more than four — which is the one thing this
         file knows ahead of layout; `sm-prose.css` is what turns that into
         `width:max-content` against `--prose-table-wide-min`. `block.align`
         is `null` for a column with no opinion, and `?? undefined` is what
         keeps that a missing attribute rather than `data-align="null"`. -->
    <div data-table-scroll v-else-if="block.type === 'table'">
      <table :data-wide="block.head.length > 4 ? '' : undefined">
        <thead>
          <tr>
            <th
              v-for="(cell, column) in block.head"
              :key="column"
              :data-align="block.align[column] ?? undefined"
            >
              <MarkdownInline :nodes="cell" @open="emit('open', $event)" />
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, r) in block.rows" :key="r">
            <td
              v-for="(cell, column) in row"
              :key="column"
              :data-align="block.align[column] ?? undefined"
            >
              <MarkdownInline :nodes="cell" @open="emit('open', $event)" />
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </template>
</template>
