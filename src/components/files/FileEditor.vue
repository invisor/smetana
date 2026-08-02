<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { EditorView } from '@codemirror/view'
import { EditorState, Transaction } from '@codemirror/state'
import Button from '../core/Button.vue'
import { editorExtensions } from './editor/extensions.js'
import { languageFor } from './editor/languages.js'
import { putState, takeState } from './editor/states.js'
import { languageState, readOnlyState, updateListenerState } from './editor/compartments.js'

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
  readOnly: { type: Boolean, default: false },
  /* Путь нужен ровно для одного: выбрать язык подсветки по расширению. */
  path: { type: String, default: '' }
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

/* readOnly живой: он снимается, когда первое чтение файла вернулось. Именно
   readOnly, а не editable: выделять и копировать из двоичного или ещё не
   прочитанного файла можно, менять — нет. Язык живёт в отсеке по похожей
   причине — import() асинхронный, редактор к моменту его прихода уже
   отрисован. Все три отсека — readOnlyState, languageState и
   updateListenerState — объявлены в editor/compartments.js, не здесь: см.
   комментарии там о том, почему они общие на все экземпляры. */

/* Наружу — только настоящая правка. Сравнение с modelValue гасит эхо: без
   него значение, пришедшее сверху, уезжает обратно и сбивает каретку.
   Отдельная фабрика, а не инлайн: слушатель замыкается на props и emit
   текущего экземпляра, и при усыновлении чужого состояния (onMounted) его
   пересоздают заново тем же вызовом — см. compartments.js. */
const changeListener = () =>
  EditorView.updateListener.of((update) => {
    if (!update.docChanged) return
    const text = update.state.doc.toString()
    if (text === props.modelValue) return
    emit('update:modelValue', text)
  })

const createState = (doc) =>
  EditorState.create({
    doc,
    extensions: [
      ...editorExtensions(),
      readOnlyState.of(EditorState.readOnly.of(props.readOnly)),
      languageState.of([]),
      updateListenerState.of(changeListener())
    ]
  })

onMounted(() => {
  /* Переключение на доску или в чат размонтирует поле (`v-if="fileTabActive"`
     в DesktopApp.vue), и следующее открытие файла — это уже новый экземпляр
     компонента. Без чтения takeState здесь сохранённое в onBeforeUnmount
     состояние было бы мёртвым грузом: записывается, но никогда не читается. */
  const saved = props.path ? takeState(props.path) : null
  view = new EditorView({
    state: saved ? saved.state : createState(props.modelValue),
    parent: host.value
  })
  if (saved) {
    /* Отсеки общие на все экземпляры (editor/compartments.js): readOnly с
       момента сохранения мог измениться — например, файл дочитался, — так
       что он выставляется заново, а не наследуется. updateListenerState
       переставляется по той же причине, но по более серьёзной: слушатель,
       унаследованный от прошлого (уничтоженного) экземпляра, эмитил бы в
       никуда, и правки после возврата с доски молча терялись бы. */
    view.dispatch({
      effects: [
        readOnlyState.reconfigure(EditorState.readOnly.of(props.readOnly)),
        updateListenerState.reconfigure(changeListener())
      ]
    })
    /* Пока поле было размонтировано, файл могли перечитать с диска. Тогда
       сохранённый документ устарел, и правду говорит буфер. */
    if (saved.state.doc.toString() !== props.modelValue) replaceDoc(props.modelValue)
    /* Прокрутка восстанавливается после того, как состояние отрисовано: до
       этого у scrollDOM ещё чужая высота. */
    const { scrollTop } = saved
    requestAnimationFrame(() => {
      if (view) view.scrollDOM.scrollTop = scrollTop
    })
  }
  applyLanguage(props.path)
})

onBeforeUnmount(() => {
  if (view && props.path) putState(props.path, view.state, view.scrollDOM.scrollTop)
  view?.destroy()
  view = null
})

/* Гонка здесь настоящая: пока грузится язык, вкладку могли переключить.
   Ответ применяется, только если путь всё ещё тот, что запрашивали. */
const applyLanguage = async (path) => {
  const language = await languageFor(path)
  if (!view || path !== props.path) return
  view.dispatch({ effects: languageState.reconfigure(language ?? []) })
}

const replaceDoc = (text) => {
  if (!view || text === view.state.doc.toString()) return
  /* Приход содержимого с диска — не правка человека, и в историю он не идёт:
     иначе Cmd+Z на только что открытом файле откатывал бы его к пустому
     документу, пустота уходила бы в буфер, и следующий Cmd+S записал бы её
     поверх настоящего файла. То же для Reload после stale — отменять чужую
     запись нечем, для этого есть Keep mine, который спрашивает заранее. */
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: text },
    annotations: Transaction.addToHistory.of(false)
  })
}

/* Один watcher на оба props вместо двух: при переключении вкладки path и
   modelValue меняются в один тик, и раздельные watcher'ы дерутся за порядок —
   текст нового файла успевал бы попасть в состояние старого. */
watch(
  () => [props.path, props.modelValue],
  ([path, text], [prevPath] = []) => {
    if (!view) return

    if (path !== prevPath) {
      if (prevPath) putState(prevPath, view.state, view.scrollDOM.scrollTop)
      const saved = takeState(path)
      view.setState(saved ? saved.state : createState(text))
      /* Восстановленное состояние несёт отсеки такими, какими они были при
         сохранении. readOnly с тех пор мог измениться — например, файл
         дочитался, — поэтому он выставляется заново, а не наследуется. */
      view.dispatch({ effects: readOnlyState.reconfigure(EditorState.readOnly.of(props.readOnly)) })
      if (saved) {
        /* Прокрутка восстанавливается после того, как новое состояние
           отрисовано: до этого у scrollDOM ещё чужая высота. */
        const { scrollTop } = saved
        requestAnimationFrame(() => {
          if (view) view.scrollDOM.scrollTop = scrollTop
        })
        /* Пока вкладка была скрыта, файл могли перечитать с диска. Тогда
           сохранённый документ устарел, и правду говорит буфер. */
        if (saved.state.doc.toString() !== text) replaceDoc(text)
      }
      applyLanguage(path)
      return
    }

    replaceDoc(text)
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
