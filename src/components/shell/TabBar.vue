<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, onUnmounted, ref, useSlots, watch } from 'vue'
import IconButton from '../core/IconButton.vue'
import Tab from './Tab.vue'
import { moveTab, orderTabs } from './tabOrder.js'

const props = defineProps({
  tabs: { type: Array, default: () => [] },
  activeId: { type: [String, Number], default: undefined },
  overflowCount: { type: Number, default: 0 },
  /* Whether a tab may be dragged to another place in the row. The board's own
     prop by the same name and for the same reason: the row is not the owner of
     the order, and a caller that keeps none says so here. */
  reorderable: { type: Boolean, default: true }
})

/* `reorder` carries the full list of ids in their new order, not a from/to
   pair, and only the ids of the tabs that can move — the pinned run is not in
   it. `KanbanBoard.vue`'s decision, for its reason: the row does not own the
   order and has no business describing a change to a list it does not keep.
   Whoever stores it applies the answer wholesale and hands it back through
   `tabs`. */
const emit = defineEmits(['select', 'close', 'promote', 'reorder'])

const slots = useSlots()

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
/* The order under the pointer, and only while the pointer holds it. Idle, this
   is null and the row draws exactly what it was given — the drawn order is the
   parent's, the same way the board's is. The draft is applied through
   `orderTabs`, so a tab opened mid-drag appears at the end instead of vanishing.
   `held` is the id of the tab being dragged. */
const draft = ref(null)
const held = ref(null)

const view = computed(() => (draft.value ? orderTabs(props.tabs, draft.value) : props.tabs))

const pinned = computed(() => {
  const at = view.value.findIndex((tab) => tab.kind !== 'pinned')
  return at === -1 ? view.value : view.value.slice(0, at)
})

const rest = computed(() => view.value.slice(pinned.value.length))

const movable = computed(() => props.reorderable && rest.value.length > 1)

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

/* The page-wide guards a drag needs, the same pair `KanbanBoard.vue` and
   `Resizer` take: without them the pointer sweeping the row selects every tab
   label it crosses, and the cursor flickers between grabbing and whatever it
   passes over. */
let guarded = null
const guard = () => {
  if (guarded) return
  guarded = { userSelect: document.body.style.userSelect, cursor: document.body.style.cursor }
  document.body.style.userSelect = 'none'
  document.body.style.cursor = 'grabbing'
}
const unguard = () => {
  if (!guarded) return
  document.body.style.userSelect = guarded.userSelect
  document.body.style.cursor = guarded.cursor
  guarded = null
}

let capture = null
let moved = false
/* The box of the held tab, kept from the moment the row last settled. Nothing
   moves again until the pointer leaves it, and this is the one thing here the
   board does not need.
   Columns are all one width, so after a swap the pointer is over the held
   column again by construction and the next move finds it already where it
   belongs. A tab is as wide as its label up to 200px, and that guarantee is
   simply false: a narrow tab dragged through a wide one lands in a position
   where the neighbour is under the pointer once more, and the row would then
   trade the two back and forth at the frame rate with the pointer standing
   still. `settling` covers the frame between the draft changing and the row
   being redrawn, during which no box can be measured. */
let lock = null
let settling = false

/* The tabs that can be dragged, as elements, in drawn order. Read off the
   strip's own children rather than looked up by id — `KanbanBoard.vue`'s
   `columnAt` does the same and for a stronger reason here: a file tab's id is a
   path and a terminal's begins with a zero byte, neither of which is a thing to
   put in a selector. The offset is the pinned run plus the `afterPinned` slot's
   own box, which is exactly what the template puts in front of them. */
const cells = () => {
  const kids = scrollerRef.value ? [...scrollerRef.value.children] : []
  return kids.slice(pinned.value.length + (slots.afterPinned ? 1 : 0))
}

/* Which movable tab the pointer is over, in `rest`'s indices. Everything to the
   left of the first one answers 0, which is what stops a tab being dropped in
   front of the pinned run: the index space is the movable part of the row and
   has no position before it. */
const tabAt = (x) => {
  const boxes = cells()
  for (let i = 0; i < boxes.length; i += 1) {
    if (x < boxes[i].getBoundingClientRect().right) return i
  }
  return boxes.length - 1
}

const heldBox = () => {
  const box = cells()[rest.value.findIndex((tab) => tab.id === held.value)]
  if (!box) return null
  const { left, right } = box.getBoundingClientRect()
  return { left, right }
}

/* Escape abandons the drag. It has to be on the window: the pointer is captured
   and focus is wherever it was, so a handler on the row would never see it. */
const onKeydown = (event) => {
  if (event.key !== 'Escape') return
  event.preventDefault()
  end(false)
}

/* Capture goes on the scrolling strip, not on the tab that was pressed. It is
   what makes a release outside the window still end the drag — and taking it
   here means every move and release arrives at one element regardless of which
   tab the pointer has since crossed into.

   It is also the one call that can throw, on a pointer the engine no longer
   knows. Letting that escape would leave the page unselectable with no drag left
   to release it, so a refused capture costs the drag once the pointer leaves the
   strip, and nothing more. */
const onGrab = (id, event) => {
  if (!movable.value || draft.value) return
  held.value = id
  draft.value = rest.value.map((tab) => tab.id)
  moved = false
  /* Latched from the start rather than from the first swap: the tab under the
     pointer is the held one, so until the pointer leaves its box there is
     nothing to decide — and that doubles as the threshold that keeps a plain
     click from being a drag of nothing. */
  lock = heldBox()
  guard()
  window.addEventListener('keydown', onKeydown)
  try {
    scrollerRef.value.setPointerCapture(event.pointerId)
    capture = event.pointerId
  } catch {
    capture = null
  }
}

/* The order is rebuilt from what is drawn on every move rather than mutated in
   place: `tabAt` answers in the drawn row's indices, and the two would disagree
   the moment a tab opened or closed mid-drag. */
const onPointermove = (event) => {
  if (!draft.value || settling) return
  const x = event.clientX
  if (lock) {
    if (x >= lock.left && x <= lock.right) return
    lock = null
  }
  const order = rest.value.map((tab) => tab.id)
  const next = moveTab(order, order.indexOf(held.value), tabAt(x))
  if (next === order) return
  draft.value = next
  moved = true
  /* The row has not been redrawn yet, so the held tab's new box cannot be
     measured until the next tick. Until then nothing else may move: measuring
     the old box would latch onto a position the tab has already left. */
  settling = true
  nextTick(() => {
    settling = false
    lock = heldBox()
  })
}

const end = (commit = true) => {
  if (!draft.value) return
  const order = draft.value
  const changed = moved
  draft.value = null
  held.value = null
  moved = false
  lock = null
  settling = false
  unguard()
  window.removeEventListener('keydown', onKeydown)
  if (capture != null) {
    /* Releasing a capture the element no longer holds throws in some engines,
       and by here the pointer may already be gone. */
    try {
      scrollerRef.value?.releasePointerCapture(capture)
    } catch {
      /* already released — nothing to undo */
    }
    capture = null
  }
  if (commit && changed) emit('reorder', order)
}

// A drag that outlives the component would leave the page unselectable.
onBeforeUnmount(() => {
  unguard()
  window.removeEventListener('keydown', onKeydown)
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
    <!-- The strip is where the capture is taken, so every move and release
         during a drag arrives here whichever tab the pointer has crossed into.
         The pinned tabs and the `afterPinned` slot are inside it and stay in
         front of the movable ones throughout: `pinned` and `rest` are read off
         the drawn order, draft and all. -->
    <div
      ref="scrollerRef"
      class="sm-scroll-hidden"
      :style="{ display: 'flex', minWidth: 0, overflowX: 'auto', overflowY: 'hidden' }"
      @pointermove="onPointermove"
      @pointerup="end()"
      @pointercancel="end(false)"
      @lostpointercapture="end()"
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
        :movable="movable"
        :moving="t.id === held"
        @select="$emit('select', t.id)"
        @close="$emit('close', t.id)"
        @promote="$emit('promote', t.id)"
        @grab="onGrab(t.id, $event)"
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
