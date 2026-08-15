<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { EditorState } from '@codemirror/state'
import { MergeView } from '@codemirror/merge'
import { editorExtensions } from './extensions.js'
import { languageFor } from './languages.js'
import { languageState, readOnlyState } from './compartments.js'

/* One changed file, side by side: HEAD on the left, the working tree on the
   right, both read-only.

   `MergeView` from @codemirror/merge is the same CodeMirror the editor beside
   it is built from, so everything it draws is themed in `editor/theme.js` —
   the documented exception, and the only place in this system where CSS rules
   are written.

   Read-only in both panes and nothing else: editing a diff, resolving a
   conflict in it, and comparing two arbitrary revisions are all deliberately
   out. What the panel answers is one question — what has changed in this file
   since the last commit — and a field somebody can type into would promise a
   second one.

   No `states.js` here, and that is the difference from `FileEditor.vue`: an
   editor keeps a caret, a selection and an undo history worth carrying across a
   tab switch, while a diff has none of the three. The panes are rebuilt from
   the two texts whenever they change, which is once per read. */
const props = defineProps({
  /* Relative to the repository. The tail after the last "/" is what picks the
     highlighting; nothing here joins it to anything. */
  path: { type: String, default: '' },
  /* The file as HEAD has it, and the working tree's copy. Both are plain text
     by the time they arrive: the refusals are `notice`'s. */
  head: { type: String, default: '' },
  work: { type: String, default: '' },
  /* HEAD does not have this file at all — added, untracked, or a repository
     with no commit in it yet. The left side is empty either way; this is what
     lets the caption say which of the two empties it is. */
  missingAtHead: { type: Boolean, default: false },
  /* A refusal, in words: binary, too large, not UTF-8, outside the project. The
     strip is drawn and the panes are not — a diff of what could not be read
     would be two empty columns saying nothing. */
  notice: { type: String, default: '' }
})

const rootStyle = {
  display: 'flex',
  flexDirection: 'column',
  flex: 1,
  minWidth: 0,
  minHeight: 0,
  background: 'var(--editor-bg)'
}

/* The same quiet strip `FileEditor` puts a read refusal in, and quiet for the
   same reason: the loudness in this interface belongs to the card waiting for a
   person. */
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

/* Which column is which. Without it the two sides are two piles of code and
   nothing on screen says which way round the change runs. Mono, because both
   captions name a thing git names: a revision and a working tree. The halves
   are `flex: 1` with a zero basis, matching what @codemirror/merge gives its
   own two panes. */
const captionsStyle = {
  display: 'flex',
  flex: '0 0 auto',
  height: 'var(--row-h)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)',
  color: 'var(--text-muted)'
}
const captionStyle = {
  flex: '1 1 0',
  minWidth: 0,
  display: 'flex',
  alignItems: 'center',
  padding: '0 var(--space-5)',
  overflow: 'hidden',
  whiteSpace: 'nowrap',
  textOverflow: 'ellipsis'
}

/* `minWidth: 0` beside `minHeight: 0`, and both are load-bearing — the rule
   `TerminalView.vue` paid for. A flex item defaults to `min-width: auto` and
   refuses to shrink below its own content, so narrowing the centre column would
   leave two panes of code painted over the task panel on the right. */
const hostStyle = { flex: 1, minWidth: 0, minHeight: 0, overflow: 'hidden' }

const host = ref(null)
let view = null

/* Two renders can overlap across the `nextTick` below — a text arriving in the
   same tick the notice clears — and without this the second would leave the
   first's merge view attached to a host nobody destroys. */
let generation = 0

/* The two panes are built from `editorExtensions()` exactly as the editor's own
   state is: the same line numbers, the same search, and above all the same
   theme, so a file looks the same on both sides of the app.

   `EditorState.readOnly` rather than `EditorView.editable`, matching
   `FileEditor`: selecting and copying out of a diff is the whole point of
   having one, changing it is what is refused. The compartments are the shared
   ones from `compartments.js` — a compartment is a key and the value lives in
   the state, so per-instance keys would leave `reconfigure` silently doing
   nothing. `languageState` opens empty because the language is an `import()`
   that lands after the first paint. */
const side = (doc) => ({
  doc,
  extensions: [
    ...editorExtensions(),
    readOnlyState.of(EditorState.readOnly.of(true)),
    languageState.of([])
  ]
})

/* The race is `FileEditor`'s: while the chunk loads, the tab may have moved on.
   The answer is applied only if the path is still the one that was asked for,
   and to both panes — one highlighted side and one plain would read as a change
   nobody made. */
const applyLanguage = async (path) => {
  const language = await languageFor(path)
  if (!view || path !== props.path) return
  const effects = languageState.reconfigure(language ?? [])
  view.a.dispatch({ effects })
  view.b.dispatch({ effects })
}

const render = async () => {
  const mine = ++generation
  view?.destroy()
  view = null
  /* The host is `v-if`'d off while there is a notice, so the DOM has to catch
     up with the props before anything is attached to it. */
  await nextTick()
  if (mine !== generation || !host.value) return
  view = new MergeView({ a: side(props.head), b: side(props.work), parent: host.value })
  /* The one thing that cannot be said in the theme. @codemirror/merge's own
     base theme forces `height: auto` on the two editors and makes its container
     the scroller, and that container is an ancestor of the elements a
     CodeMirror theme can reach — every selector there is scoped to the editor.
     So the height goes on the element itself, and it is a layout value of the
     kind every style object in this app already carries rather than a colour or
     a size the system holds a token for. */
  view.dom.style.height = '100%'
  applyLanguage(props.path)
}

onMounted(render)
watch(() => [props.path, props.head, props.work, props.notice], render)
onBeforeUnmount(() => {
  view?.destroy()
  view = null
})
</script>

<template>
  <div :style="rootStyle">
    <div v-if="notice" :style="noticeStyle">{{ notice }}</div>
    <template v-else>
      <div :style="captionsStyle">
        <span :style="captionStyle">{{ missingAtHead ? 'Not in HEAD' : 'HEAD' }}</span>
        <span :style="captionStyle">Working tree</span>
      </div>
      <div ref="host" :style="hostStyle" />
    </template>
  </div>
</template>
