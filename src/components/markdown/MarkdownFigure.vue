<script setup>
/* One illustration in the agent's prose, matched to the contract exactly
   (`docs/design_handoff_conversation_panel/markup-contract.md`, section 7):
   `figure[data-figure] > img|svg|div[data-placeholder]` then `figcaption`
   then `button[data-expand]`. `sm-prose.css` paints all of it off those
   element names and `data-` attributes alone — nothing here is a `:style`.

   Two rules decide what may even reach this markup, both in
   `figureSource.js` because a `.vue` file is the one thing no test in this
   repository can reach:

   `isAllowedFigureSrc` is why this does not simply reuse `markdown.js`'s own
   `link()`. A link is only ever opened by an explicit click, through the OS's
   own browser — nothing this app fetches on its own account. An illustration
   is the opposite: the browser paints an `<img>` the instant the markup
   exists, unattended, so a scheme this app cannot make sense of as a picture
   (`javascript:` chief among them) is refused before it ever reaches the DOM,
   and so is a remote `http(s)://` address — this is an offline-first app that
   fetches nothing of its own accord, and an inline illustration that quietly
   asked a stranger's server for bytes on every render is a tracking pixel
   with no click behind it. `//host/path` is refused the same way and before
   the scheme test even runs: it has no scheme by the grammar, but every
   renderer resolves it against the current origin's own scheme, so it is
   exactly as live a request as a written-out `http://` one. What is
   accepted: a path with no scheme and no leading `//` (`./a.png`, the
   parser's own acceptance case, included) and a self-contained `data:` URI
   whose media type is `image/…`.

   `readInlineSvg` is the second, and it is what lets the preferred form exist
   at all. The only way literal `<svg>` markup reaches this panel without the
   `v-html` `Markdown.vue`'s header refuses is a `data:image/svg+xml` source,
   decoded and walked element by element against a closed diagram vocabulary,
   with every `fill`/`stroke` checked against `currentColor`, `none`,
   `transparent` or a `var(--token)` and nothing else. Anything the walk
   refuses fails the *whole* figure — an SVG that does not pass is never drawn
   partially — and the render falls back to the same placeholder a broken
   raster load already draws, captioned with the reason, which is the answer
   to "what does it do instead": the loading and the failure states below are
   one mechanism, not two.

   A raster source is the mat's own case, and its `loading`/`error` states are
   read off the ordinary way a browser reads an image — an off-screen `Image`
   probe, not a second `<img>` in the panel's own DOM, so the frame and the
   caption are already in place before the visible `<img>` ever appears and
   nothing about the figure moves when it does. No bytes are read by this app
   to do it; the browser resolves the same URL an `<img src>` would. */
import { computed, ref, watch } from 'vue'
import InlineFigureSvg from './InlineFigureSvg.vue'
import { iconNodes } from '../core/icons.js'
import {
  describeFigureSrc,
  isAllowedFigureSrc,
  isInlineSvgSrc,
  readInlineSvg
} from './figureSource.js'

const props = defineProps({
  /* The parsed node — `{ src, alt }`, `markdown.js`'s `image` block or inline
     node unchanged. */
  block: { type: Object, required: true }
})

const emit = defineEmits(['open-image'])

/* The one glyph this figure draws by hand rather than through `Icon.vue`:
   that component's own root carries a `style` attribute for its flex sizing,
   and section 9 of the contract refuses one anywhere in this tree. Its
   children are still `core/icons.js`'s own — the only file allowed to name
   Lucide — read straight off the registered node and drawn the way
   `Icon.vue` draws them, just without the wrapper that would add the one
   thing not allowed here. */
const expandIconChildren = iconNodes['maximize-2']?.[2] || []

const allowed = computed(() => isAllowedFigureSrc(props.block.src))
const inlineSvg = computed(() => allowed.value && isInlineSvgSrc(props.block.src))
const svgResult = computed(() => (inlineSvg.value ? readInlineSvg(props.block.src) : null))

/* The raster branch's own state. `loading` starts true and `failed` false on
   every *new* source — reset by the watcher below rather than only once at
   mount, because `v-for` reuses this component by position, not by picture:
   editing a field and re-parsing it hands the same instance a different
   `block.src`. `probeSeq` is the guard against the late answer of a probe a
   newer source has already replaced — the same shape `ImageWindow.vue`'s
   `showSeq` and `git.js`'s loads use it for. */
const loading = ref(true)
const failed = ref(false)
let probeSeq = 0

watch(
  () => props.block.src,
  (src) => {
    const seq = ++probeSeq
    failed.value = false
    if (inlineSvg.value || !allowed.value) {
      /* Nothing to load: the vector branch resolves the instant it is
         validated, and a blocked source is never handed to the DOM at all. */
      loading.value = false
      return
    }
    loading.value = true
    const probe = new Image()
    probe.onload = () => {
      if (seq === probeSeq) loading.value = false
    }
    probe.onerror = () => {
      if (seq === probeSeq) {
        loading.value = false
        failed.value = true
      }
    }
    probe.src = src
  },
  { immediate: true }
)

const state = computed(() => {
  if (!allowed.value) return 'error'
  if (inlineSvg.value) return svgResult.value?.ok ? undefined : 'error'
  if (failed.value) return 'error'
  return loading.value ? 'loading' : undefined
})

const errorReason = computed(() => {
  if (!allowed.value) return 'source not accepted'
  if (inlineSvg.value) return svgResult.value?.reason ?? 'could not be rendered'
  return 'could not be loaded'
})

const showImg = computed(() => !inlineSvg.value && allowed.value && state.value === undefined)
const showSvg = computed(() => inlineSvg.value && svgResult.value?.ok === true)

/* The short, readable name for the source, printed *after* the reason in the
   placeholder below rather than before it. The inline `<svg>` form only ever
   exists as a `data:` URI, so a render-check failure's own source is exactly
   the case a raw `block.src` runs to hundreds of characters and clips
   silently inside the frame — `describeFigureSrc` is what shortens a `data:`
   source to its media type alone, and the reason leading is what stays
   readable regardless of how long the source still is. */
const sourceLabel = computed(() => describeFigureSrc(props.block.src))

function onExpand() {
  emit('open-image', { src: props.block.src, name: props.block.alt || undefined })
}
</script>

<template>
  <figure data-figure :data-state="state">
    <img v-if="showImg" :src="block.src" :alt="block.alt" />
    <InlineFigureSvg v-else-if="showSvg" :node="svgResult.root" />
    <div v-else data-placeholder>
      <template v-if="state === 'error'">
        <strong>Failed to render</strong>
        <span>{{ errorReason }} · {{ sourceLabel }}</span>
      </template>
    </div>
    <figcaption v-if="block.alt">{{ block.alt }}</figcaption>
    <button type="button" data-expand aria-label="Open full size" @click="onExpand">
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.75"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <component :is="child[0]" v-for="(child, index) in expandIconChildren" :key="index" v-bind="child[1]" />
      </svg>
    </button>
  </figure>
</template>
