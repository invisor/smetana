<script setup>
import { computed, nextTick, ref, watch } from 'vue'
import LogLine from './LogLine.vue'
import LogToolbar from './LogToolbar.vue'

const props = defineProps({
  lines: { type: Array, default: () => [] },
  query: { type: String, default: '' },
  streamState: { type: String, default: 'streaming' },
  follow: { type: Boolean, default: true },
  height: { type: [Number, String], default: 260 }
})

const emit = defineEmits(['toggle-follow', 'toggle-stream', 'update:query'])

const q = computed(() => String(props.query || '').toLowerCase())
const isMatch = (l) => (q.value ? String(l.text || '').toLowerCase().indexOf(q.value) >= 0 : false)
const matches = computed(() => (q.value ? props.lines.filter(isMatch).length : null))

const scroller = ref(null)
watch(
  () => [props.lines, props.follow],
  async () => {
    if (!props.follow) return
    await nextTick()
    if (scroller.value) scroller.value.scrollTop = scroller.value.scrollHeight
  },
  { immediate: true, deep: true }
)

const style = computed(() => ({
  display: 'flex',
  flexDirection: 'column',
  minHeight: 0,
  height: typeof props.height === 'number' ? `${props.height}px` : props.height,
  background: 'var(--editor-bg)',
  color: 'var(--text-primary)',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-3)',
  overflow: 'hidden'
}))
</script>

<template>
  <div :style="style">
    <LogToolbar
      :stream-state="streamState"
      :follow="follow"
      :query="query"
      :matches="matches"
      @toggle-follow="emit('toggle-follow')"
      @toggle-stream="emit('toggle-stream')"
      @update:query="emit('update:query', $event)"
    />
    <div ref="scroller" :style="{ flex: 1, minHeight: 0, overflow: 'auto', padding: 'var(--space-3) 0' }">
      <LogLine v-for="(l, i) in lines" :key="i" v-bind="l" :match="isMatch(l)" />
    </div>
  </div>
</template>
