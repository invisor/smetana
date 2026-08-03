<script setup>
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import IconButton from '../core/IconButton.vue'
import Tab from './Tab.vue'

const props = defineProps({
  tabs: { type: Array, default: () => [] },
  activeId: { type: [String, Number], default: undefined },
  overflowCount: { type: Number, default: 0 }
})

defineEmits(['select', 'close', 'promote'])

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
        v-for="t in props.tabs"
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
