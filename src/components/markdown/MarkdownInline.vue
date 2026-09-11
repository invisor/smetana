<script setup>
/* The inline half of `Markdown.vue`: a run of text with its emphasis, its code
   and its links. Recursive on itself for what sits inside a strong or an
   emphasis — a second file only because a template cannot recurse into a
   fragment of itself, and not a second set of rules. Every rule is in
   `markdown.js`; this file is only how a node looks.

   A link is emitted rather than opened, the way `AboutSettings` does it: no
   component in `src/components/` knows Tauri exists, and the view binds the
   app's one link-opening path to the event. The parser hands over an `href`
   only for http and https, so there is nothing here to judge. */
defineProps({
  nodes: { type: Array, required: true }
})

const emit = defineEmits(['open'])

const strong = { fontWeight: 'var(--weight-semibold)', color: 'var(--text-primary)' }
const em = { fontStyle: 'italic' }

/* Inline code wraps at any character: the panel is one narrow column and the
   thing most often in backticks here is a long path. A code *block* does the
   opposite — see `Markdown.vue`. */
const code = {
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-snug) var(--font-mono)',
  color: 'var(--text-primary)',
  background: 'var(--surface-sunken)',
  borderRadius: 'var(--radius-2)',
  padding: '0 var(--space-1)',
  overflowWrap: 'anywhere'
}

/* The underline is the whole affordance, and it is permanent rather than shown
   on hover: a link here sits inside a sentence, and one that announced itself
   only under the pointer would be invisible to somebody reading. The cursor
   stays the arrow, as it does on every other control in this app — this is a
   desktop window, not a page. */
const linkStyle = {
  color: 'var(--text-primary)',
  textDecoration: 'underline',
  textUnderlineOffset: 'var(--space-1)',
  cursor: 'default',
  overflowWrap: 'anywhere'
}

/* The author's line breaks are kept. Markdown would fold them into one line,
   and this panel deliberately does not: bd's prose is written hard-wrapped, and
   `notes` is a log where every `bd note` appends a line — reflowing it would
   turn a list of entries into a paragraph. */
const textStyle = { whiteSpace: 'pre-wrap' }
</script>

<template>
  <template v-for="(node, index) in nodes" :key="index">
    <span v-if="node.type === 'text'" :style="textStyle">{{ node.value }}</span>
    <code v-else-if="node.type === 'code'" :style="code">{{ node.value }}</code>
    <strong v-else-if="node.type === 'strong'" :style="strong">
      <MarkdownInline :nodes="node.children" @open="emit('open', $event)" />
    </strong>
    <em v-else-if="node.type === 'em'" :style="em">
      <MarkdownInline :nodes="node.children" @open="emit('open', $event)" />
    </em>
    <span
      v-else-if="node.type === 'link'"
      :style="linkStyle"
      role="link"
      tabindex="0"
      @click="emit('open', node.href)"
      @keydown.enter="emit('open', node.href)"
    >
      <MarkdownInline :nodes="node.children" @open="emit('open', $event)" />
    </span>
  </template>
</template>
