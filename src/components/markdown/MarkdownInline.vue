<script setup>
/* The inline half of `Markdown.vue`: a run of text with its emphasis, its code
   and its links. Recursive on itself for what sits inside a strong, an
   emphasis or a strikethrough — a second file only because a template cannot
   recurse into a fragment of itself, and not a second set of rules. Every rule
   is in `markdown.js` and, for a link's own target, in `./links.js`; this file
   only turns a node into the element `sm-prose.css` already knows how to paint
   (markup-contract.md, section 2 and, for a link, section 5).

   No class and no `:style` anywhere below, the same reason `Markdown.vue`
   carries none: `code` here is `:not(pre) > code` in the stylesheet, told apart
   from a code *block*'s `pre > code` by nothing but the ancestor, and a plain
   text run is emitted as a text node with no wrapping element at all — the
   contract's own list of prose elements has no `span` for one.

   A link is emitted rather than opened, the way `AboutSettings` does it: no
   component in `src/components/` knows Tauri exists, and the view binds the
   app's own verbs to the two events below. That split is also why a local
   link's `href` is built from a `root` prop rather than read off a store —
   `filesState.root` — passed down by whoever draws this tree and has one:
   `ConversationView.vue`, which already imports stores of its own. `root`
   defaults to `''`, which is the task inspector's case (no session, no
   working tree — see the comment over the plain-text branch in the template
   below for what that does to a local link), and never breaks the "no store
   here" rule: a plain string prop is not an import of anything.

   **Two kinds of link and two events, and `href` means something different on
   each.** An **external** link (`markdown.js`'s `kind: 'external'` node,
   drawn below as a plain `<a>`) keeps this file's original behaviour word for
   word: `href` stays on the anchor for native focus and copy-link, and the
   navigation itself is intercepted on both `click` and `auxclick` — a
   navigation inside the webview would replace the app, and `click.prevent`
   alone only stops the primary button: a middle click raises `auxclick`,
   which nothing else in this tree catches (there is no navigation guard in
   `main.js`, `nativeMenu.js` or `tauri.conf.json` either), so without this
   second handler the one thing a middle click on a link means — open it
   without leaving where you are — was instead a hole this diff opened into
   the app replacing itself, on whichever of WebKitGTK, WKWebView and WebView2
   turns out to act on it. `emit('open', href)` is the same event this file
   has always raised, and `openExternal` is still its one legitimate handler
   — see the hazard recorded on `stores/app.js`'s own scope, which admits
   `https://*` and `http://*` and nothing else.

   A **local** link (`node.local`) must never reach that event: its `path`
   names something on this machine, not a URL the opener plugin's allow-list
   has ever heard of, and handing it to `openExternal` would ask the scope to
   pass a string it was never meant to see. It gets its own event,
   `open-local`, carrying `{ path, kind }` — `kind` being `file` or `dir`,
   markdown.js's own `targetKind` — and only the primary click raises it, with
   `click.prevent` alone: unlike the external case there is no `@auxclick`
   handler here, on purpose, because the contract's own acceptance criteria
   ask for the *native* middle-click and "copy link" to keep working, which
   is exactly what leaving `href` alone and `auxclick` unbound buys. `href` on
   a local anchor is a `file://` URI built by `./links.js`'s `localHref`
   rather than anything this file constructs by hand, so there is one place
   that decides what such a URI looks like. */
import { localHref } from './links.js'

defineProps({
  nodes: { type: Array, required: true },
  /* The active project's absolute path, or `''` where there is none — see the
     header above and the template's own comments for what an empty root does
     to a local link. */
  root: { type: String, default: '' }
})

const emit = defineEmits(['open', 'open-local'])

/* `auxclick` is not `click` a second time: it is what a middle click raises
   instead of it, and it also fires for a browser's back/forward buttons on a
   mouse that has them — `event.button === 1` is what narrows it to the
   middle button alone, the one a person actually means by "open this
   without leaving where I am". Bound only on the external anchor — see the
   header above for why a local one carries no handler here at all. */
function onAuxClick(event, href) {
  if (event.button === 1) emit('open', href)
}
</script>

<template>
  <template v-for="(node, index) in nodes" :key="index">
    <template v-if="node.type === 'text'">{{ node.value }}</template>
    <code v-else-if="node.type === 'code'">{{ node.value }}</code>
    <strong v-else-if="node.type === 'strong'">
      <MarkdownInline
        :nodes="node.children"
        :root="root"
        @open="emit('open', $event)"
        @open-local="emit('open-local', $event)"
      />
    </strong>
    <em v-else-if="node.type === 'em'">
      <MarkdownInline
        :nodes="node.children"
        :root="root"
        @open="emit('open', $event)"
        @open-local="emit('open-local', $event)"
      />
    </em>
    <del v-else-if="node.type === 'del'">
      <MarkdownInline
        :nodes="node.children"
        :root="root"
        @open="emit('open', $event)"
        @open-local="emit('open-local', $event)"
      />
    </del>
    <!-- A local link with somewhere to act on it: the anchor the contract
         draws, `data-path` and `data-kind` carrying the distinction the
         stylesheet keys on and never the href's own shape. The two spans are
         `markdown.js`'s own split — this file draws them and computes
         neither. -->
    <a
      v-else-if="node.type === 'link' && node.local && root"
      :data-path="node.path"
      :data-kind="node.targetKind"
      :href="localHref(root, node.path)"
      @click.prevent="emit('open-local', { path: node.path, kind: node.targetKind })"
    ><span data-head>{{ node.head }}</span><span data-tail>{{ node.tail }}</span></a>
    <!-- The same node with nowhere to act on it — the task inspector's case,
         which has no session and no working tree (see this file's header).
         Drawn as the plain text the path already is rather than as an anchor
         nothing can answer: `sm-prose.css` only styles `a[data-path]`, so an
         inert control here would still look pressable while doing nothing,
         which is the one thing worse than not styling it at all. -->
    <template v-else-if="node.type === 'link' && node.local">{{ node.head }}{{ node.tail }}</template>
    <a
      v-else-if="node.type === 'link'"
      :href="node.href"
      @click.prevent="emit('open', node.href)"
      @auxclick.prevent="onAuxClick($event, node.href)"
    >
      <MarkdownInline
        :nodes="node.children"
        :root="root"
        @open="emit('open', $event)"
        @open-local="emit('open-local', $event)"
      />
    </a>
  </template>
</template>
