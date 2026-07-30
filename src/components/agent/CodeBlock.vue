<script setup>
import { computed } from 'vue'
import IconButton from '../core/IconButton.vue'
import { tokenize } from './tokenize.js'

const props = defineProps({
  code: { type: String, default: '' },
  language: { type: String, default: '' },
  filename: { type: String, default: '' },
  startLine: { type: Number, default: 1 },
  showLineNumbers: { type: Boolean, default: true },
  diff: { type: Boolean, default: false }
})

defineEmits(['copy'])

const DIFF_BG = { '+': 'var(--diff-added-bg)', '-': 'var(--diff-removed-bg)', '~': 'var(--diff-changed-bg)' }
const DIFF_GUTTER = {
  '+': 'var(--diff-added-gutter)',
  '-': 'var(--diff-removed-gutter)',
  '~': 'var(--diff-changed-gutter)'
}

const rows = computed(() =>
  String(props.code)
    .replace(/\n$/, '')
    .split('\n')
    .map((raw, i) => {
      let mark = null
      let body = raw
      if (props.diff && /^[+\-~]/.test(raw)) {
        mark = raw[0]
        body = raw.slice(1)
      }
      return {
        key: i,
        mark,
        number: props.startLine + i,
        bg: mark ? DIFF_BG[mark] : 'transparent',
        gutter: mark ? DIFF_GUTTER[mark] : 'transparent',
        tokens: tokenize(body)
      }
    })
)

const wrapStyle = {
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-3)',
  background: 'var(--editor-bg)',
  color: 'var(--syn-variable)',
  overflow: 'hidden'
}
const headStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  height: '24px',
  padding: '0 var(--space-3) 0 var(--space-5)',
  background: 'var(--surface)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)',
  color: 'var(--text-muted)'
}
const bodyStyle = {
  overflow: 'auto',
  padding: 'var(--space-3) 0',
  font: 'var(--weight-regular) var(--text-code-size)/var(--leading-code) var(--font-mono)'
}
const numStyle = {
  flex: '0 0 auto',
  width: '34px',
  textAlign: 'right',
  paddingRight: 'var(--space-5)',
  color: 'var(--editor-line-number)',
  userSelect: 'none',
  fontVariantNumeric: 'tabular-nums'
}
const codeStyle = computed(() => ({
  flex: 1,
  minWidth: 0,
  whiteSpace: 'pre',
  paddingRight: 'var(--space-5)',
  paddingLeft: props.diff ? 'var(--space-3)' : 0,
  color: 'var(--syn-variable)'
}))
</script>

<template>
  <div :style="wrapStyle">
    <div v-if="filename || language" :style="headStyle">
      <span :style="{ flex: 1, minWidth: 0, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }">
        {{ filename || language }}
      </span>
      <span v-if="language && filename">{{ language }}</span>
      <IconButton icon="copy" label="Copy" size="sm" :style="{ width: '18px', height: '18px' }" @click="$emit('copy')" />
    </div>
    <div :style="bodyStyle">
      <div v-for="r in rows" :key="r.key" :style="{ display: 'flex', background: r.bg, minHeight: '1.55em' }">
        <span v-if="diff" :style="{ flex: '0 0 auto', width: '3px', background: r.gutter }" />
        <span v-if="showLineNumbers" :style="numStyle">{{ r.number }}</span>
        <span :style="codeStyle">
          <span v-if="r.mark" :style="{ color: r.gutter, marginRight: '4px' }">{{ r.mark }}</span>
          <span v-for="(t, j) in r.tokens" :key="j" :style="{ color: t.v ? `var(${t.v})` : 'inherit' }">{{ t.txt }}</span>
        </span>
      </div>
    </div>
  </div>
</template>
