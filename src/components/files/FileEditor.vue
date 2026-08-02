<script setup>
import { computed, ref } from 'vue'
import Button from '../core/Button.vue'

/* Простой текстовый редактор: моноширинный textarea на токенах редактора.
   Ни подсветки, ни номеров строк — их приносит CodeMirror 6 отдельной задачей
   (см. agent/tokenize.js: «Real file viewing goes through CodeMirror 6, which
   reads the same --syn-* tokens»).

   Полоска сверху появляется в двух случаях. `stale` — файл изменился на диске,
   и решение за человеком; поле остаётся редактируемым, потому что терять его
   правки нельзя. `blocked` — файл не открыть как текст (двоичный, слишком
   велик, не UTF-8, исчез); поле не редактируется. Показывать ему обычно
   нечего — кроме файла, исчезнувшего под открытой вкладкой: там прочитанное
   однажды содержимое остаётся на экране, а полоска говорит, что на диске его
   больше нет.

   Обе полоски тихие: громкость в этом интерфейсе выделена карточке, которая
   ждёт человека, и полоска её не занимает. */
const props = defineProps({
  modelValue: { type: String, default: '' },
  notice: { type: Object, default: null },
  readOnly: { type: Boolean, default: false }
})

/* `save` тут нет намеренно: Cmd+S слушает окно (DesktopApp.vue), потому что
   фокус к моменту нажатия давно мог уйти из поля — на вкладку, на строку
   дерева, на кнопку. Объявленный, но никогда не испускаемый эмит обещал бы
   вызывающему то, чего не будет. Tab и Escape остались здесь: они про поле и
   без него ничего не значат. */
const emit = defineEmits(['update:modelValue', 'reload', 'keepMine'])

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
  // В проекте нет глобального box-sizing: border-box, а width: 100% в паре
  // с padding по умолчанию считается content-box — поле оказывается шире
  // своей колонки на сумму горизонтальных отступов. border-box держит его
  // в границах панели вместе с отступами.
  boxSizing: 'border-box',
  background: 'transparent',
  color: 'var(--syn-variable)',
  border: 'none',
  resize: 'none',
  whiteSpace: 'pre',
  overflow: 'auto',
  font: 'var(--weight-regular) var(--text-code-size)/var(--leading-code) var(--font-mono)',
  tabSize: 2
}))

/* Tab вставляет отступ, а не уводит фокус: иначе в редакторе нельзя было бы
   набрать ни строчки кода. Выход с клавиатуры остаётся — Escape, потом Tab;
   так же ведёт себя CodeMirror, и это доступный выход, а не тупик.

   Режим взведён отдельным состоянием (tab-focus mode), а не проверкой «была
   ли предыдущая клавиша Escape»: Escape ничего не делает с текстом и не
   мешает набору, поэтому взвод должен пережить лишь одно точное нажатие
   Tab следом. Любая другая клавиша снимает взвод — иначе человек, нажавший
   Escape и передумавший продолжить печатать, потерял бы вставку отступа
   на первом же Tab после случайной буквы. */
const tabFocusArmed = ref(false)

const onKeydown = (event) => {
  if (event.key === 'Escape') {
    tabFocusArmed.value = true
    return
  }
  if (event.key === 'Tab' && tabFocusArmed.value) {
    tabFocusArmed.value = false
    // preventDefault здесь не нужен — фокус должен уйти по обычным правилам браузера.
    return
  }
  if (event.key !== 'Tab') {
    tabFocusArmed.value = false
  }
  if (event.key === 'Tab' && !event.shiftKey && !props.readOnly) {
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
