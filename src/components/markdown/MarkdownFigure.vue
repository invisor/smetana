<script setup>
/* One illustration in the agent's prose, matched to the contract exactly
   (`docs/design_handoff_conversation_panel/markup-contract.md`, section 7):
   `figure[data-figure] > img|svg|div[data-placeholder]` then `figcaption`
   then, for a figure with a real file behind it, `button[data-expand]`.
   `sm-prose.css` paints all of it off those element names and `data-`
   attributes alone — nothing here is a `:style`.

   Three rules decide what may even reach this markup and how, all in
   `figureSource.js` because a `.vue` file is the one thing no test in this
   repository can reach:

   `isAllowedFigureSrc` is why this does not simply reuse `markdown.js`'s own
   `link()`. A link is only ever opened by an explicit click, through the OS's
   own browser (`opener:allow-open-url`) — nothing this app fetches on its own
   account. An illustration is the opposite: the moment the markup exists the
   webview paints an `<img>`, unattended, so a scheme this app cannot make
   sense of as a picture (`javascript:` chief among them) is refused outright,
   and so is a remote `http(s)://` address — this is an offline-first desktop
   app that fetches nothing of its own accord, and an inline illustration that
   silently asked a stranger's server for bytes on every render of a task
   nobody opened a connection for is a tracking pixel with no click behind it.
   `//host/path` and its UNC twin `\\host\path` are refused the same way and
   before the scheme test even runs — see that module's own header for why the
   two are one hole and not two.

   `isInlineSvgSrc` and `readInlineSvg` are the second, and they are what let
   the preferred form exist at all. The only way literal `<svg>` markup reaches
   this panel without the `v-html` `Markdown.vue`'s header refuses is a
   `data:image/svg+xml` source, decoded and walked element by element against a
   closed diagram vocabulary, with every `fill`/`stroke` checked against
   `currentColor`, `none`, `transparent` or a `var(--token)` and nothing else.
   Anything the walk refuses fails the *whole* figure — an SVG that does not
   pass is never drawn partially — and the render falls back to the same
   placeholder a broken raster load already draws, captioned with the reason.

   `isPathFigureSrc` is the third, and it is what decides whether a source is
   read off disk at all. Everything `isAllowedFigureSrc` admits that is not a
   `data:` URI is a filesystem path — relative to `base`, or absolute as it is
   — and the bytes behind it are never this component's to fetch: `readImage`
   is a function prop (`Markdown.vue`'s own `inject('smReadImage', …)`,
   threaded down rather than imported here, since a library component may not
   know `stores/attachments.js` exists any more than it may know Tauri does),
   answering with `{ path, name, bytes, url }` off the new `image_read`
   command — the same shape `attachment_reopen` already answers with, read
   `.claude/rules/attachments.md` for why that matters. A `data:` source, by
   contrast, is exactly the bytes already, and this component still reads it
   the ordinary way a browser reads an image — an off-screen `Image` probe,
   not a second `<img>` in the panel's own DOM — so the frame and the caption
   are already in place before the visible `<img>` ever appears.

   **The expand control exists only for a figure with a real file behind
   it.** A `data:` source and a validated inline `<svg>` are never read off
   disk, so there is no absolute path `ImageWindow.vue` could be aimed at —
   the caption row stays a two-row grid regardless, since `sm-prose.css`'s own
   grid does not need a populated second column to lay the first one out. */
import { computed, ref, watch } from 'vue'
import InlineFigureSvg from './InlineFigureSvg.vue'
import { iconNodes } from '../core/icons.js'
import {
  describeFigureSrc,
  isAllowedFigureSrc,
  isInlineSvgSrc,
  isPathFigureSrc,
  readInlineSvg
} from './figureSource.js'

const props = defineProps({
  /* The parsed node — `{ src, alt }`, `markdown.js`'s `image` block or inline
     node unchanged. */
  block: { type: Object, required: true },
  /* Where a relative `block.src` resolves from — `Markdown.vue`'s own
     `effectiveBase`, already folded with `root`. */
  base: { type: String, default: '' },
  /* `(base, src) => Promise<{ path, name, bytes, url }>` — see this file's
     own header for why it arrives as a prop rather than an import. */
  readImage: { type: Function, required: true }
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
/* A source `image_read` has to be asked about: allowed, not the preferred
   vector form, and not a `data:` URI already carrying its own bytes. */
const isPath = computed(() => allowed.value && !inlineSvg.value && isPathFigureSrc(props.block.src))

/* The raster branch's own state, read one of two ways depending on where the
   bytes come from — `pathRecord`/`pathError` off `readImage` for a path,
   `loading`/`failed` off an `Image` probe for a self-contained `data:` source.
   Both start fresh on every *new* source (and, for a path, on a new `base`
   too): reset by the watcher below rather than only once at mount, because
   `v-for` reuses this component by position, not by picture — editing a field
   and re-parsing it hands the same instance a different `block.src`. `seq` is
   the guard against the late answer of a probe or a read a newer source has
   already replaced — the same shape `ImageWindow.vue`'s `showSeq` and
   `git.js`'s loads use it for. */
const loading = ref(true)
const failed = ref(false)
const pathRecord = ref(null)
const pathError = ref(null)
let seq = 0

watch(
  /* One source per getter, rather than one getter answering an array of both,
     and that is not a style choice. `parseMarkdown` re-runs over the whole
     reply on every streamed delta and hands this component a fresh `block`
     object each time, so a getter *returning* `[src, base]` builds a new array
     `Object.is` can never match, and the reset below fired on every delta of a
     reply whose picture had not changed at all — tearing the `<img>` out for
     the placeholder and re-reading the same file over IPC several times a
     second, which on screen is a figure flickering under the text still being
     typed beneath it. Watched one at a time, Vue compares each string by value
     and an unchanged source never reaches the callback. */
  [() => props.block.src, () => props.base],
  ([src]) => {
    const at = ++seq
    failed.value = false
    pathRecord.value = null
    pathError.value = null
    if (!allowed.value || inlineSvg.value) {
      /* Nothing to load: a blocked source is never handed to the DOM at all,
         and the vector branch resolves the instant it is validated. */
      loading.value = false
      return
    }
    loading.value = true
    if (isPath.value) {
      props
        .readImage(props.base, src)
        .then((record) => {
          if (at !== seq) return
          pathRecord.value = record
          loading.value = false
        })
        .catch((err) => {
          if (at !== seq) return
          pathError.value = err instanceof Error ? err.message : String(err)
          failed.value = true
          loading.value = false
        })
      return
    }
    const probe = new Image()
    probe.onload = () => {
      if (at === seq) loading.value = false
    }
    probe.onerror = () => {
      if (at === seq) {
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
  if (isPath.value) return pathError.value ?? 'could not be read'
  return 'could not be loaded'
})

/* The picture's own URL: a path's `data:` URL off `readImage`, or the source
   itself for a self-contained `data:` figure. Never `block.src` for a path —
   that is a filesystem path or the webview would resolve it against its own
   origin exactly the way this whole feature exists to stop. */
const imgSrc = computed(() => (isPath.value ? pathRecord.value?.url : props.block.src))

const showImg = computed(
  () => !inlineSvg.value && allowed.value && state.value === undefined && (!isPath.value || pathRecord.value)
)
const showSvg = computed(() => inlineSvg.value && svgResult.value?.ok === true)
/* Only a resolved path names a real file this window can be aimed at. */
const showExpand = computed(() => isPath.value && state.value === undefined && pathRecord.value)

/* The short, readable name for the source, printed *after* the reason in the
   placeholder below rather than before it. The inline `<svg>` form only ever
   exists as a `data:` URI, so a render-check failure's own source is exactly
   the case a raw `block.src` runs to hundreds of characters and clips
   silently inside the frame — `describeFigureSrc` is what shortens a `data:`
   source to its media type alone, and the reason leading is what stays
   readable regardless of how long the source still is. */
const sourceLabel = computed(() => describeFigureSrc(props.block.src))

function onExpand() {
  if (!pathRecord.value) return
  emit('open-image', { path: pathRecord.value.path, name: props.block.alt || pathRecord.value.name })
}
</script>

<template>
  <figure data-figure :data-state="state">
    <img v-if="showImg" :src="imgSrc" :alt="block.alt" />
    <InlineFigureSvg v-else-if="showSvg" :node="svgResult.root" />
    <div v-else data-placeholder>
      <template v-if="state === 'error'">
        <strong>Failed to render</strong>
        <span>{{ errorReason }} · {{ sourceLabel }}</span>
      </template>
    </div>
    <!-- Always rendered, whatever `block.alt` says: the caption *row* is
         permanent (`sm-prose.css`'s own comment on this element), and with
         the expand button now conditional too — absent for a `data:` figure
         and an inline `<svg>`, see the header above — the empty element is
         what still holds that row's height when neither a caption nor a
         button has anything to draw into it. An unconditional `figcaption`
         costs nothing where one has always rendered: a `<figcaption></figcaption>`
         with no text still takes its own padding and line box. -->
    <figcaption>{{ block.alt }}</figcaption>
    <button v-if="showExpand" type="button" data-expand aria-label="Open full size" @click="onExpand">
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
