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
   native focus and copy-link, and the navigation itself is intercepted on
   both `click` and `auxclick` — a navigation inside the webview would replace
   the app, and `click.prevent` alone only stops the primary button: a middle
   click raises `auxclick`, which nothing else in this tree catches (there is
   no navigation guard in `main.js`, `nativeMenu.js` or `tauri.conf.json`
   either), so without this second handler the one thing a middle click on a
   link means — open it without leaving where you are — was instead a hole
   this diff opened into the app replacing itself, on whichever of
   WebKitGTK, WKWebView and WebView2 turns out to act on it. The parser hands
   over an `href` only for http and https, so there is nothing here to judge;
   the local-file half of the link contract (`data-path`, `data-kind`, the
   head/tail split) is its own task, and this parser does not produce a link
   node that would need it yet.

   An `image` node reaches here only when it sat beside other words on its
   line — `markdown.js` keeps a picture that is alone on its own line a block
   of `Markdown.vue`'s own, section 7's whole figure. Section 7 has no answer
   for one sitting mid-sentence: the mat, the caption row and the expand
   control are a two-row grid, and nesting that inside the `<p>` this node is
   already inside is not a shape the contract draws anywhere. Rather than
   invent one, or drop the picture the way a link this app may not open is
   dropped, this reads it back as the markdown that made it — `![alt](src)` —
   which keeps every character on screen exactly as `markdown.js`'s own header
   promises, in the one shape the contract does have room for here: text. */
defineProps({
  nodes: { type: Array, required: true }
})

const emit = defineEmits(['open'])

/* `auxclick` is not `click` a second time: it is what a middle click raises
   instead of it, and it also fires for a browser's back/forward buttons on a
   mouse that has them — `event.button === 1` is what narrows it to the
   middle button alone, the one a person actually means by "open this
   without leaving where I am". */
function onAuxClick(event, href) {
  if (event.button === 1) emit('open', href)
}
</script>

<template>
  <template v-for="(node, index) in nodes" :key="index">
    <template v-if="node.type === 'text'">{{ node.value }}</template>
    <template v-else-if="node.type === 'image'">{{ `![${node.alt}](${node.src})` }}</template>
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
    <a
      v-else-if="node.type === 'link'"
      :href="node.href"
      @click.prevent="emit('open', node.href)"
      @auxclick.prevent="onAuxClick($event, node.href)"
    >
      <MarkdownInline :nodes="node.children" @open="emit('open', $event)" />
    </a>
  </template>
</template>
