<script setup>
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import Tooltip from '../core/Tooltip.vue'
import { fileIconUrl, folderIconUrl } from '../../catppuccinIcon.js'
import { documentTheme } from '../../documentTheme.js'

const GIT = {
  modified: { c: 'var(--git-modified)', l: 'M' },
  added: { c: 'var(--git-added)', l: 'A' },
  deleted: { c: 'var(--git-deleted)', l: 'D' },
  untracked: { c: 'var(--git-untracked)', l: 'U' },
  conflict: { c: 'var(--git-conflict)', l: '!' },
  ignored: { c: 'var(--git-ignored)', l: '' }
}

const props = defineProps({
  name: { type: String, required: true },
  depth: { type: Number, default: 0 },
  kind: { type: String, default: 'file' },
  expanded: { type: Boolean, default: false },
  selected: { type: Boolean, default: false },
  git: { type: String, default: undefined },
  readOnly: { type: Boolean, default: false },
  /* Whether the tree's one context menu is currently open on this row. The row
     does not decide it: `PointerMenu` hands the panel's key back with the pick
     and clears it on close, so the highlight and the panel cannot come apart. */
  menuOpen: { type: Boolean, default: false },
  /* Whether this row is what Cut put on the tree's clipboard. It is drawn
     muted until the clipboard is used or replaced — VS Code's signal, and the
     only thing on screen that says a cut is pending, since nothing has happened
     on disk yet.

     `--attn-quiet-opacity`, the token the `quiet` attention level already
     spends: a second colour for this would be a colour with no meaning in a
     system where the saturated range belongs to status, and dimming is what
     this app already means by "spoken for, not now". */
  cut: { type: Boolean, default: false }
})

const emit = defineEmits(['toggle', 'select', 'open', 'menu'])

const hover = ref(false)
const g = computed(() => (props.git ? GIT[props.git] : null))

/* What the name is, drawn — a `data:` URL rather than a glyph name, since the
   set's colours live inside the SVG. Reading `documentTheme` here is what makes
   the row repaint when the theme flips: the URL is rebuilt against the other
   palette, because nothing in a `data:` URL can be reached by the stylesheet. */
const iconUrl = computed(() =>
  props.kind === 'dir'
    ? folderIconUrl(props.name, props.expanded, documentTheme.value)
    : fileIconUrl(props.name, documentTheme.value)
)

const style = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  paddingLeft: `calc(var(--space-4) + ${props.depth} * var(--tree-indent))`,
  paddingRight: 'var(--space-4)',
  /* The menu's own highlight sits between the two, and is the hover surface
     rather than the selected one on purpose: a right click deliberately does
     not move the selection, and painting the row as selected would say it had.
     A row that *is* selected keeps its own surface underneath. */
  background: props.selected
    ? 'var(--surface-selected)'
    : hover.value || props.menuOpen
      ? 'var(--surface-hover)'
      : 'transparent',
  color: props.git === 'ignored' ? 'var(--text-muted)' : 'var(--text-primary)',
  /* The cut wins over the ignored row's own dimming rather than multiplying
     with it: two reasons to be faint would leave an ignored file that has been
     cut fainter than either, and nothing on screen to say which of the two it
     is. */
  opacity: props.cut ? 'var(--attn-quiet-opacity)' : props.git === 'ignored' ? 0.7 : 1,
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  cursor: 'default',
  transition: 'var(--transition-control)'
}))

const nameStyle = computed(() => ({
  flex: 1,
  minWidth: 0,
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  textDecoration: props.git === 'deleted' ? 'line-through' : undefined
}))

const onClick = () => emit(props.kind === 'dir' ? 'toggle' : 'select')

/* A double click on a file opens it as a permanent tab — the same as in VS
   Code. No delay is needed: the first click has already opened the preview and
   the second makes it permanent, and no intermediate state that would have to
   be undone arises here. */
const onDoubleClick = () => {
  if (props.kind !== 'dir') emit('open')
}

/* The secondary click, handed up with the point it happened at — where the
   panel goes is the tree's business, since there is one panel for the whole of
   it rather than one per row.

   `prevent` is not what stops the platform's own menu: `src/nativeMenu.js`
   already refuses every `contextmenu` in the document, in capture, so this is
   the second refusal of an event that is already refused — kept because a row
   that opens a menu of its own should say so where the handler is. `stop` does
   carry weight: the tree's own listener sits on the container below the rows
   and opens the root's menu, and without this every row would open two.
   Neither touches the capture listener, which has already run. */
const onContextMenu = (event) => {
  event.preventDefault()
  event.stopPropagation()
  emit('menu', event)
}
</script>

<template>
  <div
    role="treeitem"
    :aria-expanded="kind === 'dir' ? expanded : undefined"
    :aria-selected="selected"
    :tabindex="selected ? 0 : -1"
    :style="style"
    @mouseenter="hover = true"
    @mouseleave="hover = false"
    @click="onClick"
    @dblclick="onDoubleClick"
    @contextmenu="onContextMenu"
  >
    <span :style="{ width: '12px', display: 'flex', color: 'var(--text-muted)' }">
      <Icon v-if="kind === 'dir'" :name="expanded ? 'chevron-down' : 'chevron-right'" :size="12" />
    </span>
    <!-- 15px and not the row's usual 13: this set draws on a 16 grid with its
         own padding, where a lucide glyph fills its box. `alt` is empty because
         the name is right beside it. -->
    <img :src="iconUrl" alt="" width="15" height="15" :style="{ display: 'block', flex: '0 0 auto' }" />
    <span :style="nameStyle">{{ name }}</span>
    <Icon
      v-if="readOnly"
      name="lock"
      :size="11"
      title="Locked by an agent"
      :style="{ color: 'var(--status-blocked-fg)' }"
    />
    <Tooltip v-if="g && g.l" :label="git">
      <span
        :style="{ color: g.c, width: '9px', textAlign: 'center', fontWeight: 'var(--weight-semibold)' }"
      >{{ g.l }}</span>
    </Tooltip>
  </div>
</template>
