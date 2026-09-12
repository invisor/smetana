<script setup>
/* One validated node of an agent-drawn diagram, recursing on itself the way
   `Markdown.vue` and `MarkdownInline.vue` already recurse on themselves for
   their own trees. `figureSource.js`'s `readInlineSvg` is what built this
   node in the first place: every tag and every attribute here already passed
   that gate, so this file only turns the plain object into real SVG
   elements — the same trick `core/Icon.vue` already plays on a lucide
   IconNode, `<component :is>` bound from a plain object rather than a
   template literal. No `v-html` anywhere in this tree, the same rule
   `Markdown.vue`'s own header states: the DOM is built through Vue's dynamic-
   component machinery, never through markup text, so there is nothing here
   for a sanitiser to have missed. */
defineProps({
  node: { type: Object, required: true }
})
</script>

<template>
  <component :is="node.tag" v-bind="node.attrs">
    <template v-if="node.text">{{ node.text }}</template>
    <InlineFigureSvg v-for="(child, index) in node.children" :key="index" :node="child" />
  </component>
</template>
