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

   An `image` block is `MarkdownFigure.vue`'s, section 7's `figure[data-figure]`
   whole — the mat, the preferred inline `<svg>` form, the loading and the
   failure placeholders. `groups` above is the one thing this file still owns
   about it: folding a run of consecutive `image` blocks into the wrapping
   `div[data-figures]` row section 7 asks for, since that grouping is about the
   turn's layout and not about any one figure. Which sources an illustration
   may even draw, and what a source or an inline `<svg>` that fails the render
   check draws *instead* of the figure it asked for, is `figureSource.js`'s —
   see `MarkdownFigure.vue`'s own header for why that is not simply `link()`'s
   `OPENABLE` reused. */
import { computed } from 'vue'
import MarkdownInline from './MarkdownInline.vue'
import MarkdownFigure from './MarkdownFigure.vue'
import { parseMarkdown } from './markdown.js'

const props = defineProps({
  /* The source. Ignored when `blocks` is given, which is what the recursive
     calls below pass — a quote and a list item are already parsed, and parsing
     them again would be the same work done twice per level of nesting. */
  text: { type: String, default: '' },
  blocks: { type: Array, default: null }
})

const emit = defineEmits(['open', 'open-image'])

const tree = computed(() => props.blocks ?? parseMarkdown(props.text))

/* `markdown.js` hands over a flat run of blocks, with two adjacent `image`
   blocks simply sitting next to each other — nothing in the parser groups
   them, because grouping is a fact about how the *renderer* lays a turn out,
   not about the source. Section 7 of the contract draws two figures in one
   turn inside `div[data-figures]`, a wrapping flex row, and a lone figure
   bare — so this is the one place the flat list becomes something the
   template can dispatch on directly, folding every run of consecutive
   `image` blocks into one `figures` entry and leaving everything else as an
   ordinary `single` one. */
const groups = computed(() => {
  const entries = []
  for (const block of tree.value) {
    if (block.type === 'image') {
      const last = entries[entries.length - 1]
      if (last && last.type === 'figures') last.blocks.push(block)
      else entries.push({ type: 'figures', blocks: [block] })
    } else {
      entries.push({ type: 'single', block })
    }
  }
  return entries
})

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
  <template v-for="(group, index) in groups" :key="index">
    <!-- A run of one or more illustrations. A lone one stays bare, the way
         every other block does; two or more share the wrapping row section 7
         asks for, `div[data-figures]`, which is a fact about the *run* and
         not about any one figure in it. -->
    <template v-if="group.type === 'figures'">
      <div v-if="group.blocks.length > 1" data-figures>
        <MarkdownFigure
          v-for="(figure, at) in group.blocks"
          :key="at"
          :block="figure"
          @open-image="emit('open-image', $event)"
        />
      </div>
      <MarkdownFigure
        v-else
        :block="group.blocks[0]"
        @open-image="emit('open-image', $event)"
      />
    </template>

    <template v-else>
      <component :is="`h${group.block.level}`" v-if="group.block.type === 'heading'">
        <MarkdownInline :nodes="group.block.children" @open="emit('open', $event)" />
      </component>

      <p v-else-if="group.block.type === 'paragraph'">
        <MarkdownInline :nodes="group.block.children" @open="emit('open', $event)" />
      </p>

      <pre v-else-if="group.block.type === 'code'"><code>{{ group.block.text }}</code></pre>

      <hr v-else-if="group.block.type === 'rule'" />

      <blockquote v-else-if="group.block.type === 'quote'">
        <Markdown
          :blocks="group.block.blocks"
          @open="emit('open', $event)"
          @open-image="emit('open-image', $event)"
        />
      </blockquote>

      <component
        :is="group.block.ordered ? 'ol' : 'ul'"
        v-else-if="group.block.type === 'list'"
        :start="group.block.ordered && group.block.start !== 1 ? group.block.start : undefined"
        :data-task="isTaskList(group.block) ? '' : undefined"
      >
        <li
          v-for="(entry, at) in group.block.items"
          :key="at"
          :data-checked="entry.checked ? '' : undefined"
        >
          <Markdown
            :blocks="entry.blocks"
            @open="emit('open', $event)"
            @open-image="emit('open-image', $event)"
          />
        </li>
      </component>

      <dl v-else-if="group.block.type === 'dl'">
        <template v-for="(item, at) in group.block.items" :key="at">
          <dt><MarkdownInline :nodes="item.term" @open="emit('open', $event)" /></dt>
          <dd v-for="(definition, d) in item.definitions" :key="d">
            <MarkdownInline :nodes="definition" @open="emit('open', $event)" />
          </dd>
        </template>
      </dl>
    </template>
  </template>
</template>
