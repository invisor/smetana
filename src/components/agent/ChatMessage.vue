<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'

/* The agent is the actor in copy: "claude-1 needs you", never "you have a
   pending question". The app itself never says "I". */
const props = defineProps({
  role: { type: String, default: 'assistant' },
  author: { type: String, default: '' },
  time: { type: String, default: '' },
  streaming: { type: Boolean, default: false },
  error: { type: Boolean, default: false }
})

const user = computed(() => props.role === 'user')
const icon = computed(() => (user.value ? 'user' : props.role === 'system' ? 'terminal' : 'bot'))
const name = computed(
  () => props.author || (user.value ? 'you' : props.role === 'system' ? 'system' : 'agent')
)

const style = computed(() => ({
  display: 'flex',
  gap: 'var(--space-5)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  padding: 'var(--space-5) var(--panel-pad)',
  borderLeft: `var(--accent-bar-w) solid ${
    user.value ? 'var(--border-strong)' : props.error ? 'var(--status-failed-fg)' : 'transparent'
  }`,
  background: user.value ? 'var(--surface)' : 'transparent',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}))

const metaStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}

const bodyStyle = computed(() => ({
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-normal)',
  color: props.error ? 'var(--status-failed-fg)' : 'var(--text-primary)',
  textWrap: 'pretty',
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-5)'
}))

const pulseDot = {
  width: '5px', height: '5px', borderRadius: '50%', background: 'currentColor',
  animation: 'sm-pulse var(--dur-pulse) var(--ease-in-out) infinite'
}
const caret = {
  display: 'inline-block', width: '7px', height: '13px', background: 'var(--editor-cursor)',
  verticalAlign: 'text-bottom', animation: 'sm-pulse 1s steps(2,end) infinite'
}
</script>

<template>
  <div :style="style">
    <div :style="{ flex: '0 0 auto', width: '16px', paddingTop: '1px', color: 'var(--text-muted)' }">
      <Icon :name="icon" :size="14" />
    </div>
    <div :style="{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 'var(--space-4)' }">
      <div :style="metaStyle">
        <span :style="{ color: error ? 'var(--status-failed-fg)' : 'var(--text-secondary)' }">{{ name }}</span>
        <span v-if="time">{{ time }}</span>
        <span v-if="streaming" :style="{ color: 'var(--attn-live)', display: 'inline-flex', alignItems: 'center', gap: '3px' }">
          <span :style="pulseDot" />writing
        </span>
      </div>
      <div :style="bodyStyle">
        <slot />
        <span v-if="streaming" :style="caret" />
      </div>
    </div>
  </div>
</template>
