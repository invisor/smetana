<script setup>
/* The inline half of `Markdown.vue`: a run of text with its emphasis, its code
   and its links. Recursive on itself for what sits inside a strong, an
   emphasis or a strikethrough — a second file only because a template cannot
   recurse into a fragment of itself, and not a second set of rules. Every rule
   is in `markdown.js`; this file only turns a node into the element
   `sm-prose.css` already knows how to paint (markup-contract.md, section 2).

   No class and no `:style` anywhere below, the same reason `Markdown.vue`
   carries none: `code` here is `:not(pre) > code` in the stylesheet, told apart
   from a code *block*'s `pre > code` by nothing but the ancestor, and a plain
   text run is emitted as a text node with no wrapping element at all — the
   contract's own list of prose elements has no `span` for one.

   A link is emitted rather than opened, the way `AboutSettings` does it: no
   component in `src/components/` knows Tauri exists, and the view binds the
   app's one link-opening path to the event. `href` stays on the anchor for
   native focus, middle-click and copy-link, and the click is intercepted
   rather than left to navigate — a navigation inside the webview would replace
   the app. The parser hands over an `href` only for http and https, so there
   is nothing here to judge; the local-file half of the link contract
   (`data-path`, `data-kind`, the head/tail split) is its own task, and this
   parser does not produce a link node that would need it yet. */
defineProps({
  nodes: { type: Array, required: true }
})

const emit = defineEmits(['open'])
</script>

<template>
  <template v-for="(node, index) in nodes" :key="index">
    <template v-if="node.type === 'text'">{{ node.value }}</template>
    <code v-else-if="node.type === 'code'">{{ node.value }}</code>
    <strong v-else-if="node.type === 'strong'">
      <MarkdownInline :nodes="node.children" @open="emit('open', $event)" />
    </strong>
    <em v-else-if="node.type === 'em'">
      <MarkdownInline :nodes="node.children" @open="emit('open', $event)" />
    </em>
    <del v-else-if="node.type === 'del'">
      <MarkdownInline :nodes="node.children" @open="emit('open', $event)" />
    </del>
    <a v-else-if="node.type === 'link'" :href="node.href" @click.prevent="emit('open', node.href)">
      <MarkdownInline :nodes="node.children" @open="emit('open', $event)" />
    </a>
  </template>
</template>
