import { computed, ref, toValue } from 'vue'

/* hover / active tracking shared by every control.
   Hover is a surface step up, press is one further step — never a colour
   change and never a transform, so a control in a dense list cannot jump. */
export function useInteractive(disabled) {
  const hover = ref(false)
  const active = ref(false)
  const off = () => toValue(disabled) === true

  const handlers = {
    onMouseenter: () => { if (!off()) hover.value = true },
    onMouseleave: () => { hover.value = false; active.value = false },
    onMousedown: () => { if (!off()) active.value = true },
    onMouseup: () => { active.value = false }
  }

  return {
    hover: computed(() => !off() && hover.value),
    active: computed(() => !off() && active.value),
    handlers
  }
}
