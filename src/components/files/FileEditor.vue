<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { EditorView } from '@codemirror/view'
import { Compartment, EditorState } from '@codemirror/state'
import Button from '../core/Button.vue'
import { editorExtensions } from './editor/extensions.js'

/* Редактор кода на CodeMirror 6. Вся видимая механика — подсветка, номера
   строк, поиск, история, множественная каретка — живёт в editor/extensions.js;
   здесь только жизненный цикл EditorView и связь с v-model.

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
   вызывающему то, чего не будет. CodeMirror Mod-s не занимает, и событие
   спокойно всплывает до окна. */
const emit = defineEmits(['update:modelValue', 'reload', 'keepMine'])

const rootStyle = {
  display: 'flex',
  flexDirection: 'column',
  flex: 1,
  minHeight: 0,
  background: 'var(--editor-bg)'
}

const noticeStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  flex: '0 0 auto',
  padding: 'var(--space-3) var(--space-5)',
  background: 'var(--surface)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-secondary)'
}

const hostStyle = { flex: 1, minHeight: 0, overflow: 'hidden' }

const host = ref(null)
let view = null

/* readOnly живой: он снимается, когда первое чтение файла вернулось. Живое
   значение в конфигурации CodeMirror — это Compartment, другого способа нет.
   Именно readOnly, а не editable: выделять и копировать из двоичного или ещё
   не прочитанного файла можно, менять — нет. */
const readOnlyState = new Compartment()

const createState = (doc) =>
  EditorState.create({
    doc,
    extensions: [
      ...editorExtensions(),
      readOnlyState.of(EditorState.readOnly.of(props.readOnly)),
      /* Наружу — только настоящая правка. Сравнение с modelValue гасит эхо:
         без него значение, пришедшее сверху, уезжает обратно и сбивает
         каретку. */
      EditorView.updateListener.of((update) => {
        if (!update.docChanged) return
        const text = update.state.doc.toString()
        if (text === props.modelValue) return
        emit('update:modelValue', text)
      })
    ]
  })

onMounted(() => {
  view = new EditorView({ state: createState(props.modelValue), parent: host.value })
})

onBeforeUnmount(() => {
  view?.destroy()
  view = null
})

/* Внутрь — только когда пришедшее действительно отличается от документа.
   Правка снаружи (Reload после stale, Keep mine) идёт обычной транзакцией,
   а не пересозданием состояния: история правок переживает подмену. */
watch(
  () => props.modelValue,
  (next) => {
    if (!view || next === view.state.doc.toString()) return
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } })
  }
)

watch(
  () => props.readOnly,
  (next) => {
    view?.dispatch({ effects: readOnlyState.reconfigure(EditorState.readOnly.of(next)) })
  }
)
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
    <div ref="host" :style="hostStyle" />
  </div>
</template>
