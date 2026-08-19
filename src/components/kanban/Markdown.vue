<script setup>
/* One of bd's prose fields, drawn as the markdown it is.

   The rules are all in `markdown.js`; this file is only how a block looks. It
   references itself for the blocks of a list item and of a quote — one
   recursive component rather than a second file that would drift from this one.

   No `v-html`, deliberately and permanently: the tree is drawn as Vue nodes, so
   an issue's text can never become markup and no sanitiser is needed. A link is
   emitted rather than opened, the way `AboutSettings` does it — the library
   knows nothing about Tauri, and the view binds the app's one link path.

   Read-only, all of it. A task item is a `square` / `square-check` glyph and not
   `core/Checkbox.vue`: that component carries a real focusable `<input>`, and in
   a panel that cannot write to the tracker it would offer an edit nothing here
   can make. */
import { computed } from 'vue'
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

const root = { display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }

const prose = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-secondary)',
  textWrap: 'pretty',
  /* A long unbroken string — a path, a URL — breaks rather than widening the
     column it sits in, which is 320px and holds everything else the panel has
     to say. */
  overflowWrap: 'anywhere'
}

/* A heading inside the prose is never larger than the issue's own title
   (`--text-md`) two elements above it: `#` and `##` sit at the title's size and
   are told apart by weight and colour, `###` and below step down. A description
   opening with `## Acceptance Criteria` must not shout over the title. */
const heading = (level) => ({
  font: `var(--weight-${level <= 2 ? 'semibold' : 'medium'}) ${
    level <= 2 ? 'var(--text-md)' : 'var(--text-sm)'
  }/var(--leading-snug) var(--font-sans)`,
  color: 'var(--text-primary)',
  marginTop: 'var(--space-2)',
  textWrap: 'pretty'
})

/* A block keeps its lines and scrolls inside its own box: breaking a line of
   code where it does not break is a lie about the code. The inline case does
   the opposite — see `MarkdownInline.vue`. */
const codeBlock = {
  font: 'var(--weight-regular) var(--text-code-size)/var(--leading-code) var(--font-mono)',
  color: 'var(--text-primary)',
  background: 'var(--surface-sunken)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)',
  padding: 'var(--space-4)',
  margin: 0,
  overflowX: 'auto',
  whiteSpace: 'pre'
}

const quote = {
  borderLeft: 'var(--border-w-strong) solid var(--border-strong)',
  paddingLeft: 'var(--space-4)',
  color: 'var(--text-muted)'
}

const rule = { height: 'var(--border-w)', background: 'var(--border-subtle)' }

const list = { display: 'flex', flexDirection: 'column', gap: 'var(--space-2)' }

/* The marker column is as wide as its widest marker and no wider, and the text
   beside it is `minmax(0, 1fr)` so a path in it wraps instead of pushing the
   panel out. Baselines rather than tops: a checkbox and the first line of its
   item sit on the same line. */
const item = {
  display: 'grid',
  gridTemplateColumns: 'max-content minmax(0, 1fr)',
  columnGap: 'var(--space-3)',
  alignItems: 'baseline'
}

const marker = {
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-mono)',
  color: 'var(--text-muted)'
}

const check = { color: 'var(--text-muted)' }

/* Numbered from the list's own start, so a list beginning at 3 says 3. */
const bullet = (block, index) => (block.ordered ? `${block.start + index}.` : '•')
</script>

<template>
  <div :style="root">
    <template v-for="(block, index) in tree" :key="index">
      <div v-if="block.type === 'heading'" :style="heading(block.level)">
        <MarkdownInline :nodes="block.children" @open="emit('open', $event)" />
      </div>

      <div v-else-if="block.type === 'paragraph'" :style="prose">
        <MarkdownInline :nodes="block.children" @open="emit('open', $event)" />
      </div>

      <pre v-else-if="block.type === 'code'" :style="codeBlock">{{ block.text }}</pre>

      <div v-else-if="block.type === 'rule'" :style="rule" />

      <div v-else-if="block.type === 'quote'" :style="quote">
        <Markdown :blocks="block.blocks" @open="emit('open', $event)" />
      </div>

      <div v-else-if="block.type === 'list'" :style="list">
        <div v-for="(entry, at) in block.items" :key="at" :style="item">
          <!-- A glyph and not a control: nothing here is focusable, and
               clicking it does nothing, because the panel writes nothing to
               the tracker. -->
          <Icon
            v-if="entry.checked !== null"
            :name="entry.checked ? 'square-check' : 'square'"
            :size="12"
            :style="check"
          />
          <span v-else :style="marker">{{ bullet(block, at) }}</span>
          <Markdown :blocks="entry.blocks" @open="emit('open', $event)" />
        </div>
      </div>
    </template>
  </div>
</template>
