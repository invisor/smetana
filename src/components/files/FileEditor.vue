<script setup>
import { computed } from 'vue'
import Button from '../core/Button.vue'

/* Простой текстовый редактор: моноширинный textarea на токенах редактора.
   Ни подсветки, ни номеров строк — их приносит CodeMirror 6 отдельной задачей
   (см. agent/tokenize.js: «Real file viewing goes through CodeMirror 6, which
   reads the same --syn-* tokens»).

   Полоска сверху появляется в двух случаях. `stale` — файл изменился на диске,
   и решение за человеком; поле остаётся редактируемым, потому что терять его
   правки нельзя. `blocked` — файл не открыть как текст (двоичный, слишком
   велик, не UTF-8, исчез); поле пустое и не редактируется.

   Обе полоски тихие: громкость в этом интерфейсе выделена карточке, которая
   ждёт человека, и полоска её не занимает. */
const props = defineProps({
  modelValue: { type: String, default: '' },
  notice: { type: Object, default: null },
  readOnly: { type: Boolean, default: false }
})

const emit = defineEmits(['update:modelValue', 'save', 'reload', 'keepMine'])

const rootStyle = {
  display: 'flex',
  flexDirection: 'column',
  flex: 1,
  minHeight: 0,
  background: 'var(--editor-bg)'
}

const noticeStyle = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  flex: '0 0 auto',
  padding: 'var(--space-3) var(--space-5)',
  background: 'var(--surface)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-secondary)'
}))

const areaStyle = computed(() => ({
  flex: 1,
  minHeight: 0,
  width: '100%',
  padding: 'var(--space-4) var(--space-5)',
  background: 'transparent',
  color: 'var(--syn-variable)',
  border: 'none',
  outline: 'none',
  resize: 'none',
  whiteSpace: 'pre',
  overflow: 'auto',
  font: 'var(--weight-regular) var(--text-code-size)/var(--leading-code) var(--font-mono)',
  tabSize: 2
}))

/* Tab вставляет отступ, а не уводит фокус: иначе в редакторе нельзя было бы
   набрать ни строчки кода. Выход с клавиатуры остаётся — Escape, потом Tab;
   так же ведёт себя CodeMirror, и это доступный выход, а не тупик. */
const onKeydown = (event) => {
  if (event.key === 'Tab' && !event.shiftKey) {
    event.preventDefault()
    const el = event.target
    const { selectionStart: from, selectionEnd: to } = el
    const next = `${props.modelValue.slice(0, from)}  ${props.modelValue.slice(to)}`
    emit('update:modelValue', next)
    // Курсор восстанавливаем после того, как Vue вернёт новое значение.
    requestAnimationFrame(() => {
      el.selectionStart = from + 2
      el.selectionEnd = from + 2
    })
    return
  }
  if ((event.metaKey || event.ctrlKey) && event.key === 's') {
    event.preventDefault()
    emit('save')
  }
}
</script>

<template>
  <div :style="rootStyle">
    <div v-if="notice" :style="noticeStyle">
      <span :style="{ flex: 1, minWidth: 0 }">{{ notice.text }}</span>
      <template v-if="notice.tone === 'stale'">
        <Button variant="secondary" size="sm" @click="emit('reload')">Reload</Button>
        <Button variant="secondary" size="sm" @click="emit('keepMine')">Keep mine</Button>
      </template>
    </div>
    <textarea
      :value="modelValue"
      :readonly="readOnly"
      spellcheck="false"
      autocapitalize="off"
      autocomplete="off"
      autocorrect="off"
      :style="areaStyle"
      @input="emit('update:modelValue', $event.target.value)"
      @keydown="onKeydown"
    />
  </div>
</template>
