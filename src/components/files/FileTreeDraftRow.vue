<script setup>
/* The row for a file or a folder that does not exist yet: a field where the
   entry will be, rather than a dialog over the tree. VS Code's form, and the
   reason for it is that the answer to "where is this going" is the row's own
   position — indented under the folder it lands in, among the names it will sit
   between — which a modal in the middle of the screen cannot say at all.

   Everything about it is `FileTreeRow`'s measurements: the same height, the
   same indent per level, the same two glyph slots and the same mono type. That
   is not decoration — a draft that sat a pixel off would read as a thing
   floating over the tree rather than a place in it, and the two files are the
   one place this can go wrong, so they are checked side by side in the gallery.

   The icon is the file-type set's, resolved from what is being typed as it is
   typed: `notes.md` grows a markdown glyph before the file exists. `Icon`'s
   `file-plus` would have been the other choice and says the wrong thing — it
   belongs to the menu row that opened this, where the question is what the verb
   does; here the question is what the thing is.

   Three ways out and only one of them makes anything. Enter commits, Esc
   cancels, and losing the focus cancels — the last because a draft left behind
   a click elsewhere is a field nothing on screen explains, and because the tree
   redraws under it whenever `catchUp` re-lists the folder. The name goes up raw:
   what a name typed here comes to is `newEntry.js`'s, which is a rule and lives
   where a test can reach it. */
import { computed, onMounted, ref } from 'vue'
import { fileIconUrl, folderIconUrl } from '../../catppuccinIcon.js'
import { documentTheme } from '../../documentTheme.js'

const props = defineProps({
  /* 'file' or 'dir' — which verb opened this, and so which glyph and which
     placeholder. */
  kind: { type: String, default: 'file' },
  /* How deep the row sits, in the same units `FileTreeRow` counts: the folder
     this is being made in, plus one. */
  depth: { type: Number, default: 0 },
  /* Off in the gallery and nowhere else. In the app the field takes the
     keyboard the instant it appears — that is the whole gesture — but the
     gallery draws this row on a page of eighty other components, and a field
     that focuses itself there scrolls the page to itself on load and pulls the
     keyboard out of whatever was being checked. Nothing about how the row looks
     depends on it: the ring is on the border, not on `:focus`. */
  focusOnMount: { type: Boolean, default: true }
})

const emit = defineEmits(['commit', 'cancel'])

const name = ref('')
const field = ref(null)

/* One way out, taken once. Enter removes the field, and the blur that follows
   would otherwise arrive as a cancel a moment after the commit — and a cancel
   after a commit is a draft closed twice, which the caller cannot tell from a
   person changing their mind. */
const done = ref(false)
const leave = (how, value) => {
  if (done.value) return
  done.value = true
  emit(how, value)
}

onMounted(() => {
  if (props.focusOnMount) field.value?.focus()
})

const iconUrl = computed(() =>
  props.kind === 'dir'
    ? folderIconUrl(name.value, false, documentTheme.value)
    : fileIconUrl(name.value, documentTheme.value)
)

const rowStyle = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  paddingLeft: `calc(var(--space-4) + ${props.depth} * var(--tree-indent))`,
  paddingRight: 'var(--space-4)',
  background: 'var(--surface-hover)',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}))

/* A bare field rather than `core/Input`, and the height is the whole reason:
   that one is `--control-h` tall by design, which is a control's height and not
   a tree row's, so a draft built from it would push the rows below it down by
   the difference. What it keeps of that component is the raised surface and the
   ring; what it does not is the shape — `--radius-2` and a plain border rather
   than `--radius-3` and an inset shadow, because a shadow inside a box this
   short reads as a second border, and the ring is permanent here where in an
   `Input` it comes and goes with the focus. */
const fieldStyle = {
  flex: 1,
  minWidth: 0,
  height: 'calc(var(--row-h) - var(--space-2))',
  padding: '0 var(--space-2)',
  border: 'var(--border-w) solid var(--focus-ring)',
  borderRadius: 'var(--radius-2)',
  outline: 'none',
  background: 'var(--surface-raised)',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}
</script>

<template>
  <div :style="rowStyle">
    <!-- The chevron's slot, empty: a draft has nothing to expand, and taking
         the width back would slide the field out of line with every row above
         it. -->
    <span :style="{ width: '12px', display: 'flex' }" />
    <!-- 15px, for the reason `FileTreeRow` carries: this set draws on a 16 grid
         with padding of its own. -->
    <img :src="iconUrl" alt="" width="15" height="15" :style="{ display: 'block', flex: '0 0 auto' }" />
    <input
      ref="field"
      v-model="name"
      type="text"
      spellcheck="false"
      autocomplete="off"
      :aria-label="kind === 'dir' ? 'New folder name' : 'New file name'"
      :placeholder="kind === 'dir' ? 'Folder name' : 'File name'"
      :style="fieldStyle"
      @keydown.enter.prevent="leave('commit', name)"
      @keydown.esc.prevent.stop="leave('cancel')"
      @blur="leave('cancel')"
    />
  </div>
</template>
