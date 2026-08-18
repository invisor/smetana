<script setup>
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import IconButton from '../core/IconButton.vue'
import Tab from './Tab.vue'

const props = defineProps({
  tabs: { type: Array, default: () => [] },
  activeId: { type: [String, Number], default: undefined },
  overflowCount: { type: Number, default: 0 }
})

defineEmits(['select', 'close', 'promote'])

/* The row has two slots and they are in different places, which is the whole
   reason there are two. `actions` sits past the `flex: 1` strut at the far
   right of the bar, for anything about the row as a whole. `afterPinned` sits
   *inside* the scrolling strip, between the tabs that are always there and the
   ones a project brought — so a control about those first tabs stays beside
   them however many files are open, instead of drifting to the far end where
   the scrollbar is hidden and it could be pushed off the edge.

   Which tabs those are is read off the list rather than passed in as a count:
   the leading run of pinned ones is what "the tabs that are always there"
   means, and `tabs.js` is already the one place that decides it. */
const pinned = computed(() => {
  const at = props.tabs.findIndex((tab) => tab.kind !== 'pinned')
  return at === -1 ? props.tabs : props.tabs.slice(0, at)
})

const rest = computed(() => props.tabs.slice(pinned.value.length))

/* The tab strip scrolls, but its own scrollbar is hidden (sm-scroll-hidden)
   and the overflow menu (overflowCount) is not wired up — without this the
   active tab can slide entirely past the edge and become unreachable, behind a
   scrollbar there is no way even to feel for. So the container brings the
   active tab into view itself — and not only when the active tab changes, but
   whenever the container's own width changes (a neighbouring panel was
   collapsed or expanded, the window was resized): the width may have changed
   without a single click on a tab, while the old scroll position stays the
   same. block: 'nearest' matters no less than inline: without it the browser
   would drag the page's vertical scroll along too. Smooth scrolling is not
   enabled — in this system movement carries no meaning. */
const scrollerRef = ref(null)

const revealActiveTab = () => {
  const el = scrollerRef.value?.querySelector('[aria-selected="true"]')
  el?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
}

watch(
  () => props.activeId,
  async () => {
    // nextTick is mandatory: at the moment the watch fires the new tab may not
    // be rendered in the DOM yet.
    await nextTick()
    revealActiveTab()
  }
)

let resizeObserver = null

onMounted(() => {
  // An active tab restored at startup (from settings.json) may have ended up
  // past the edge before the first click on any tab — the watch on activeId has
  // not fired at that point. immediate: true on the watch would not do: it would
  // run before the render, when there is nothing to look for.
  revealActiveTab()

  resizeObserver = new ResizeObserver(revealActiveTab)
  if (scrollerRef.value) {
    resizeObserver.observe(scrollerRef.value)
  }
})

onUnmounted(() => {
  // A ResizeObserver left connected survives the component's unmount and keeps
  // a reference to the node — we disconnect explicitly rather than relying on
  // garbage collection.
  resizeObserver?.disconnect()
  resizeObserver = null
})

const barStyle = {
  display: 'flex',
  alignItems: 'stretch',
  height: 'var(--tab-h)',
  flex: '0 0 auto',
  background: 'var(--surface)',
  borderBottom: 'var(--border-w) solid var(--border)',
  minWidth: 0
}
/* Its own border on the right, so the control reads as part of the pinned block
   rather than as the first of the file tabs — the same separator every tab
   draws. `flex: '0 0 auto'` because it lives inside a scrolling strip and must
   not be squeezed by it. */
const afterPinnedStyle = {
  display: 'flex',
  alignItems: 'center',
  flex: '0 0 auto',
  padding: '0 var(--space-2)',
  borderRight: 'var(--border-w) solid var(--border-subtle)'
}
const overflowStyle = {
  display: 'flex',
  alignItems: 'center',
  padding: '0 var(--space-3)',
  borderRight: 'var(--border-w) solid var(--border-subtle)'
}
</script>

<template>
  <div role="tablist" :style="barStyle">
    <div
      ref="scrollerRef"
      class="sm-scroll-hidden"
      :style="{ display: 'flex', minWidth: 0, overflowX: 'auto', overflowY: 'hidden' }"
    >
      <Tab
        v-for="t in pinned"
        :key="t.id"
        v-bind="t"
        :active="t.id === activeId"
        @select="$emit('select', t.id)"
        @close="$emit('close', t.id)"
        @promote="$emit('promote', t.id)"
      />
      <div v-if="$slots.afterPinned" :style="afterPinnedStyle">
        <slot name="afterPinned" />
      </div>
      <Tab
        v-for="t in rest"
        :key="t.id"
        v-bind="t"
        :active="t.id === activeId"
        @select="$emit('select', t.id)"
        @close="$emit('close', t.id)"
        @promote="$emit('promote', t.id)"
      />
    </div>
    <div v-if="overflowCount > 0" :style="overflowStyle">
      <IconButton icon="chevrons-right" :label="`${overflowCount} more tabs`" size="sm" />
      <span :style="{ fontSize: 'var(--text-2xs)', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)' }">
        +{{ overflowCount }}
      </span>
    </div>
    <div :style="{ flex: 1 }" />
    <div
      v-if="$slots.actions"
      :style="{ display: 'flex', alignItems: 'center', gap: 'var(--space-2)', padding: '0 var(--space-3)' }"
    >
      <slot name="actions" />
    </div>
  </div>
</template>
