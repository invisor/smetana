<script setup>
import { computed, onBeforeUnmount, ref } from 'vue'

const props = defineProps({
  orientation: { type: String, default: 'vertical' },
  /* What one arrow key moves. The separator is focusable and announces itself
     as a separator, so it owes a keyboard a way to do what the pointer does. */
  step: { type: Number, default: 16 },
  label: { type: String, default: undefined }
})

/* `drag` carries the displacement since the last `dragstart`, not since the
   last move: the consumer clamps against a width it snapshotted at the start,
   and per-move increments would drift away from the pointer every time a clamp
   swallowed one. A keyboard step is a whole drag in miniature — start, one
   delta, end — so both paths reach the consumer as the same three events. */
const emit = defineEmits(['dragstart', 'drag', 'dragend', 'reset'])

const state = ref('idle')
const vertical = computed(() => props.orientation === 'vertical')

/* The page-wide guards a drag needs. Without them the pointer sweeping across
   the app selects every label it crosses, and the cursor flickers between
   col-resize and whatever it happens to be over. */
let guarded = null
const guard = () => {
  if (guarded) return
  guarded = {
    userSelect: document.body.style.userSelect,
    cursor: document.body.style.cursor
  }
  document.body.style.userSelect = 'none'
  document.body.style.cursor = vertical.value ? 'col-resize' : 'row-resize'
}
const unguard = () => {
  if (!guarded) return
  document.body.style.userSelect = guarded.userSelect
  document.body.style.cursor = guarded.cursor
  guarded = null
}

let origin = 0
let captured = null

const axis = (e) => (vertical.value ? e.clientX : e.clientY)

const end = () => {
  if (state.value !== 'dragging') return
  state.value = 'idle'
  unguard()
  if (captured) {
    /* Releasing a capture the element no longer holds throws in some engines,
       and by here the pointer may already be gone. */
    try {
      captured.el.releasePointerCapture(captured.id)
    } catch {
      /* already released — nothing to undo */
    }
    captured = null
  }
  emit('dragend')
}

const onPointerdown = (e) => {
  if (e.button !== 0) return
  origin = axis(e)
  state.value = 'dragging'
  guard()
  /* Capture is what makes a release outside the window still end the drag.
     Window listeners cannot see that one, and the separator would stay stuck
     to the pointer with no button held.

     It is also the one call here that can throw — a pointer the engine no
     longer knows about. Letting that escape would abort the handler with the
     page already unselectable and no drag to release it, so a refused capture
     costs the drag once the pointer leaves the strip, and nothing more. */
  try {
    e.currentTarget.setPointerCapture(e.pointerId)
    captured = { el: e.currentTarget, id: e.pointerId }
  } catch {
    captured = null
  }
  emit('dragstart')
}

const onPointermove = (e) => {
  if (state.value !== 'dragging') return
  emit('drag', axis(e) - origin, e)
}

const onKeydown = (e) => {
  const back = vertical.value ? 'ArrowLeft' : 'ArrowUp'
  const forward = vertical.value ? 'ArrowRight' : 'ArrowDown'
  if (e.key !== back && e.key !== forward) return
  e.preventDefault()
  emit('dragstart')
  emit('drag', e.key === forward ? props.step : -props.step, e)
  emit('dragend')
}

// A drag that outlives the component would leave the page unselectable.
onBeforeUnmount(unguard)

const style = computed(() => ({
  flex: '0 0 auto',
  position: 'relative',
  width: vertical.value ? 'var(--resizer-w)' : 'auto',
  height: vertical.value ? 'auto' : 'var(--resizer-w)',
  cursor: vertical.value ? 'col-resize' : 'row-resize',
  /* Hover paints too, not just the drag. The `0.35` below has always been here
     and did nothing while only a drag painted a background: a fraction of
     nothing is nothing, and the strip's whole affordance was the cursor. */
  background: state.value === 'idle' ? 'transparent' : 'var(--focus-ring)',
  opacity: state.value === 'hover' ? 0.35 : 1,
  touchAction: 'none',
  transition: 'background-color var(--dur-fast) var(--ease-out)'
}))
</script>

<template>
  <div
    role="separator"
    :aria-orientation="orientation"
    :aria-label="label"
    tabindex="0"
    :style="style"
    @pointerdown="onPointerdown"
    @pointermove="onPointermove"
    @pointerup="end"
    @pointercancel="end"
    @lostpointercapture="end"
    @dblclick="emit('reset')"
    @keydown="onKeydown"
    @pointerenter="state = state === 'dragging' ? state : 'hover'"
    @pointerleave="state = state === 'dragging' ? state : 'idle'"
  />
</template>
