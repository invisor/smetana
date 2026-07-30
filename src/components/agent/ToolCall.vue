<script setup>
import { computed, ref, watch } from 'vue'
import Icon from '../core/Icon.vue'
import CodeBlock from './CodeBlock.vue'

const props = defineProps({
  name: { type: String, required: true },
  args: { type: String, default: '' },
  result: { type: String, default: '' },
  state: { type: String, default: 'done' },
  expanded: { type: Boolean, default: false },
  duration: { type: String, default: '' }
})

const emit = defineEmits(['toggle'])

const open = ref(props.expanded)
watch(() => props.expanded, (v) => { open.value = v })

const toggle = () => {
  open.value = !open.value
  emit('toggle', open.value)
}

const tone = computed(() =>
  props.state === 'error'
    ? 'var(--status-failed-fg)'
    : props.state === 'running'
      ? 'var(--attn-live)'
      : 'var(--text-secondary)'
)
const stateIcon = computed(() =>
  props.state === 'error' ? 'x' : props.state === 'running' ? 'loader-circle' : 'wrench'
)
const stateIconStyle = computed(() => ({
  color: tone.value,
  animation: props.state === 'running' ? 'sm-spin 1.6s linear infinite' : undefined
}))

const wrapStyle = {
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)',
  background: 'var(--surface)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  overflow: 'hidden'
}
const headStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  cursor: 'default'
}
const argsStyle = {
  flex: 1,
  minWidth: 0,
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)'
}
</script>

<template>
  <div :style="wrapStyle">
    <div role="button" tabindex="0" :aria-expanded="open" :style="headStyle" @click="toggle" @keydown.enter="toggle">
      <Icon :name="open ? 'chevron-down' : 'chevron-right'" :size="13" :style="{ color: 'var(--text-muted)' }" />
      <Icon :name="stateIcon" :size="13" :style="stateIconStyle" />
      <span :style="{ font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)', color: 'var(--text-primary)' }">
        {{ name }}
      </span>
      <span v-if="args && !open" :style="argsStyle">{{ args }}</span>
      <span v-else :style="{ flex: 1 }" />
      <span
        v-if="duration"
        :style="{ font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)', color: 'var(--text-muted)' }"
      >{{ duration }}</span>
    </div>
    <div
      v-if="open"
      :style="{ padding: '0 var(--space-5) var(--space-4)', display: 'flex', flexDirection: 'column', gap: 'var(--space-4)' }"
    >
      <CodeBlock v-if="args" :code="args" :show-line-numbers="false" filename="arguments" />
      <CodeBlock v-if="result" :code="result" :show-line-numbers="false" :filename="state === 'error' ? 'error' : 'result'" />
    </div>
  </div>
</template>
