<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { EditorView } from '@codemirror/view'
import { EditorState, Transaction } from '@codemirror/state'
import Button from '../core/Button.vue'
import { editorExtensions } from './editor/extensions.js'
import { languageFor } from './editor/languages.js'
import { peekState, putState } from './editor/states.js'
import { languageState, readOnlyState, updateListenerState } from './editor/compartments.js'

/* A code editor on CodeMirror 6. All the visible mechanics — highlighting,
   line numbers, search, history, multiple carets — live in
   editor/extensions.js; only the EditorView lifecycle and the v-model link are
   here.

   The strip at the top appears in two cases. `stale` — the file changed on disk
   and the decision is the person's; the field stays editable, because their
   edits must not be lost. `blocked` — the file cannot be opened as text
   (binary, too large, not UTF-8, gone); the field is not editable. There is
   usually nothing to show in it — except for a file that vanished under an open
   tab: there the content read once stays on screen while the strip says it is
   no longer on disk.

   Both strips are quiet: the loudness in this interface is reserved for the
   card that is waiting for a person, and a strip does not take it. */
const props = defineProps({
  modelValue: { type: String, default: '' },
  notice: { type: Object, default: null },
  readOnly: { type: Boolean, default: false },
  /* The path does two jobs at once: the tail after the last "/" picks the
     highlighting language, while the whole path (already joined with the
     project root in DesktopApp.vue) is the key under which editor/states.js
     keeps this tab's document, caret, edit history and scroll position. */
  path: { type: String, default: '' }
})

/* There is deliberately no `save` here: the window listens for Cmd+S
   (DesktopApp.vue), because by the time it is pressed focus may long since have
   left the field — for a tab, a tree row, a button. A declared but never
   emitted event would promise the caller something that will not happen.
   CodeMirror does not take Mod-s, and the event bubbles up to the window
   undisturbed. */
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

/* adoptState writes the restored scroll position into scrollDOM not
   immediately but on the next frame (see there for why). Both places that save
   the scroll position read scrollDOM.scrollTop synchronously, and if a switch
   happens faster than a frame — A→B→C shorter than one requestAnimationFrame —
   B would save a scroll position that has not been applied yet, that is,
   somebody else's, left over from A. pendingScrollTop holds the value that is
   about to become real, and both save points prefer it to the live scrollDOM. */
let pendingScrollTop = null

/* readOnly is live: it is lifted when the file's first read comes back.
   readOnly specifically, not editable: selecting and copying from a binary or
   not-yet-read file is allowed, changing it is not. The language lives in a
   compartment for a similar reason — import() is asynchronous, and by the time
   it arrives the editor is already rendered. All three compartments —
   readOnlyState, languageState and updateListenerState — are declared in
   editor/compartments.js rather than here: see the comments there on why they
   are shared by every instance. */

/* Only a real edit travels outwards. The comparison with modelValue kills the
   echo: without it a value arriving from above goes straight back and knocks
   the caret about. A separate factory rather than an inline function: the
   listener closes over the current instance's props and emit, and adopting
   somebody else's state (adoptState below) recreates it with this same call —
   see compartments.js. */
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
  /* Switching to the board or to the chat unmounts the field
     (`v-if="fileTabActive"` in DesktopApp.vue), and the next file opened is
     already a new component instance. Without reading peekState here, the state
     saved in onBeforeUnmount would be dead weight: written but never read.

     The EditorView is always built with createState, even when a saved state
     exists: the constructor needs something valid here and now, while adoption
     is a separate step, identical for every caller, below. */
  view = new EditorView({ state: createState(props.modelValue), parent: host.value })
  const saved = props.path ? peekState(props.path) : null
  if (saved) adoptState(saved, props.modelValue)
  applyLanguage(props.path)
})

onBeforeUnmount(() => {
  if (view && props.path) putState(props.path, view.state, pendingScrollTop ?? view.scrollDOM.scrollTop)
  view?.destroy()
  view = null
})

/* The race here is real: while the language loads, the tab may have been
   switched. The answer is applied only if the path is still the one that was
   asked for. */
const applyLanguage = async (path) => {
  const language = await languageFor(path)
  if (!view || path !== props.path) return
  view.dispatch({ effects: languageState.reconfigure(language ?? []) })
}

const replaceDoc = (text) => {
  if (!view || text === view.state.doc.toString()) return
  /* Content arriving from disk is not a person's edit and does not enter the
     history: otherwise Cmd+Z on a freshly opened file would roll it back to an
     empty document, the emptiness would go into the buffer, and the next Cmd+S
     would write it over the real file. The same goes for Reload after stale —
     there is nothing to undo somebody else's write with, and Keep mine, which
     asks up front, exists for that. */
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: text },
    annotations: Transaction.addToHistory.of(false)
  })
}

/* The single place where somebody else's state becomes ours. There were two of
   them, and both times they disagreed it cost a person their typed text: first
   the listener was not re-pointed, then it was re-pointed in only one of the
   two paths. State outlives a component instance, so adoption has to re-bind
   EVERYTHING that closes over an instance to the live one — and to do it the
   same way whoever adopts: onMounted (a file came back after the board) and the
   watcher below (the path changed inside a live instance, but the tab's state
   may have been written by a previous one). */
const adoptState = (saved, text) => {
  view.setState(saved.state)
  /* readOnly may have changed since the state was saved — the file finished
     reading, say — so it is set anew rather than inherited. updateListenerState
     is re-pointed for a more serious reason: an inherited listener may belong to
     an already destroyed instance and emit into nowhere — and then edits would
     silently never reach the buffer. */
  view.dispatch({
    effects: [
      readOnlyState.reconfigure(EditorState.readOnly.of(props.readOnly)),
      updateListenerState.reconfigure(changeListener())
    ]
  })
  /* While the tab was inactive, the file may have been re-read from disk. The
     saved document is then out of date, and the buffer tells the truth. */
  if (saved.state.doc.toString() !== text) replaceDoc(text)
  /* The scroll position is restored after the state has rendered: before that
     scrollDOM still has somebody else's height. pendingScrollTop holds the same
     value synchronously, in case the state is saved before that frame arrives —
     see the comment at its declaration. */
  const { scrollTop } = saved
  pendingScrollTop = scrollTop
  requestAnimationFrame(() => {
    if (view) view.scrollDOM.scrollTop = scrollTop
    pendingScrollTop = null
  })
}

/* One watcher for both props instead of two: on a tab switch path and
   modelValue change in the same tick, and separate watchers would race for
   order — the new file's text would make it into the old file's state. */
watch(
  () => [props.path, props.modelValue],
  ([path, text], [prevPath] = []) => {
    if (!view) return

    if (path !== prevPath) {
      if (prevPath) putState(prevPath, view.state, pendingScrollTop ?? view.scrollDOM.scrollTop)
      const saved = peekState(path)
      if (saved) {
        adoptState(saved, text)
      } else {
        view.setState(createState(text))
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
