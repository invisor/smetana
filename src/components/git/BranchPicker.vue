<script setup>
/* One branch out of a project's list, picked from a list that sits in the
   document rather than over it.

   **This is the component the review window's redesign is for.** The branch was
   picked with `core/Dropdown.vue` until now, and that control is right
   everywhere it is used and wrong here for one reason: it teleports its panel
   out and places it in window coordinates. That is exactly what a panel inside
   a scrolling container needs, and the review dialog is not one — it is a
   separate, OS-level, non-resizable window whose height is computed from its
   content (`views/DialogWindow.vue`, `src-tauri/src/window.rs::height_to_set`).
   A window sized for one table row is about 330px tall, a filter and eight
   branches is about 300px of panel, and `place()` then clamps the panel against
   the window's own edge. `feature/smetana-4nsa-remote-branches-repo` in a panel
   as wide as the field was, in practice, unpickable.

   So this one is an ordinary block. Nothing is teleported, nothing is measured
   against the window, and the only scrolling is the list's own — which is what
   makes it safe in a window whose height is decided by what is inside it. It
   knows nothing about that window: it is a block in a flow, and where it is
   drawn is the caller's business.

   **`origin` is a prefix and not a second side.** There were two dropdowns
   here, one for the name and one for `local`/`origin`, which is four controls
   on a row that asks two questions. There is one list now, holding every branch
   twice — itself, then its `origin/` variant — and what comes out is a name plus
   a flag saying which of the two it was. The reasoning, the order and the meta
   line all live in `branchPicker.js`, for the reason that whole family exists:
   a `.vue` file is the one thing no test in this repository can reach.

   Nothing here is wired to the review window yet. Adding it is the window's own
   task; this one builds the component and leaves it in the gallery. */
import { computed, nextTick, ref, watch } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import {
  BRANCH_FILTER_LABEL,
  NO_BRANCH_MATCHES,
  PICKER_KEY_HINT,
  branchCountLabel,
  matchingBranches,
  pickerRows,
  stepCursor
} from './branchPicker.js'

const props = defineProps({
  /* `target_branches`' answer, `{ name, missing_in, at }` apiece — the same
     list `ReviewChangesDialog.vue` and `run/BranchSelect.vue` are handed,
     extended with `at`, the branch's own last touch in epoch seconds and null
     where git could not say. A list from before that field existed draws a meta
     line one piece shorter and nothing else.

     `at` and emphatically not `updated_at`: this front end already reads that
     name off a bd issue in four places (`stores/tracker.js`,
     `kanban/boardView.js`, `kanban/TaskInspector.vue`,
     `shell/commandPalette.js`), where it is an ISO 8601 string. One name for a
     string of one shape and a number of another is the kind of trap that costs
     somebody an afternoon. */
  branches: { type: Array, default: () => [] },
  /* How many repositories the project has, which is the other half of what
     `missing_in` means: the field says where a branch is absent, so the count
     of where it is present cannot be worked out from it alone. */
  repos: { type: Number, default: 0 },
  /* When origin was last fetched, in epoch seconds, or null for a repository
     nobody has fetched into. It is a fact about the repository rather than
     about any one branch, which is why it is one prop and not a field on every
     row. */
  fetchedAt: { type: Number, default: null },
  /* The clock, in epoch seconds, so a window that ticks can keep the ages
     honest while it sits open — the objection `TaskInspector.vue` records about
     relative labels, answered the way `agent/SessionRow.vue` answers it. The
     default reads the clock once, which is right for a window that is opened,
     used and closed inside a minute. Seconds and not milliseconds: everything
     `branchPicker.js` takes is seconds, and the one division lives here. */
  now: { type: Number, default: () => Math.floor(Date.now() / 1000) },
  /* What is picked, as the two facts it is made of. Two props rather than one
     `modelValue`, because the answer is a pair and a `v-model` over an object
     would make every parent build a new object to say that one half of it
     changed. */
  selected: { type: String, default: '' },
  selectedOrigin: { type: Boolean, default: false },
  /* The right-hand end of the footer: what picking here applies to. The words
     are the caller's — this component has no idea how many repositories a
     review will touch — and an empty string simply leaves that end blank. */
  scope: { type: String, default: '' }
})

const emit = defineEmits(['select', 'close'])

const query = ref('')
const cursor = ref(0)
const filterField = ref(null)
const listBox = ref(null)

const matched = computed(() => matchingBranches(props.branches, query.value))
const rows = computed(() =>
  pickerRows(matched.value, {
    repos: props.repos,
    now: props.now,
    fetchedAt: props.fetchedAt
  })
)
/* Branches and not rows — see `branchCountLabel` for why the two differ here,
   and why the honest number is the one somebody is choosing between. */
const count = computed(() => branchCountLabel(matched.value.length, props.branches.length))

const isSelected = (row) => row.name === props.selected && row.origin === props.selectedOrigin

/* The highlight opens on what is already picked, so a list of forty branches
   does not open on the first one when the answer is the thirty-first. */
const seat = () => {
  const at = rows.value.findIndex(isSelected)
  cursor.value = at >= 0 ? at : 0
}
seat()

/* Filtering re-seats rather than clamps: the row under the highlight is gone as
   often as not, and a cursor left at index 4 of a list that shrank to two would
   arm Enter over whatever moved into that place.

   Watched on the rows' keys and deliberately not on `rows` itself. A computed
   array is a new array every time anything it reads changes, and one of those
   things is the clock: a window ticking `now` to keep the ages honest would
   otherwise throw the highlight back to the top once a minute, under the hands
   of whoever was walking the list. The keys change when the list does and not
   when a label does. */
watch(
  () => rows.value.map((row) => row.key).join('\n'),
  () => {
    seat()
    if (rows.value.length) nextTick(reveal)
  }
)

watch(cursor, () => nextTick(reveal))

/* Brings the highlighted row inside the nine the list shows, by hand and never
   through `scrollIntoView` — that one is free to scroll every scrollable
   ancestor, and this list sits inside a window whose content is the thing being
   scrolled past. The same arithmetic `Dropdown.vue`'s `reveal()` does, measured
   in window coordinates rather than through `offsetTop`, which would be
   relative to whichever ancestor happens to be positioned. */
function reveal() {
  const box = listBox.value
  const row = box?.children[cursor.value]
  if (!row) return
  const rowAt = row.getBoundingClientRect()
  const listAt = box.getBoundingClientRect()
  if (rowAt.top < listAt.top) box.scrollTop -= listAt.top - rowAt.top
  else if (rowAt.bottom > listAt.bottom) box.scrollTop += rowAt.bottom - listAt.bottom
}

const choose = (row) => {
  if (!row) return
  emit('select', { name: row.name, origin: row.origin })
}

/* Which controls Enter belongs to, and `Dropdown.vue` carries the same guard
   for the same reason: a `<button>` is activated by Enter as well as by Space,
   so an Enter pressed on the close button would choose a row nobody pointed at
   *and* swallow the press that was meant for the button. The filter field and
   the rows are the two places the highlight is a highlight of; everything else
   inside here keeps its own key.

   The arrows and Escape are deliberately not asked the same question. Walking
   the list and closing it are right from anywhere in the component, and neither
   cancels anything another control was about to do. */
const ownsEnter = (target) => target === filterField.value || !!listBox.value?.contains(target)

/* Bound on the shell, so the arrows are seen whether the keyboard is in the
   filter field or on a row. */
const onKeydown = (event) => {
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    cursor.value = stepCursor(cursor.value, event.key === 'ArrowDown' ? 1 : -1, rows.value.length)
  } else if (event.key === 'Enter') {
    if (!ownsEnter(event.target)) return
    event.preventDefault()
    choose(rows.value[cursor.value])
  } else if (event.key === 'Escape') {
    event.preventDefault()
    emit('close')
  }
}

/* The keyboard is handed over rather than taken: a block in a flow that focused
   itself on mount would steal the caret from whatever the window opened with.
   The window that draws this one is the side that knows whether the branch is
   the first thing to answer. */
defineExpose({ focus: () => filterField.value?.focus() })

/* The one number in this file that is allowed to be a number: a glyph's size,
   in the design system's own units, exactly as every other component here
   passes one to `Icon`. Everything else is a token, because both densities and
   the app-wide font size have to move it. */
const GLYPH = 13

const shellStyle = {
  display: 'flex',
  flexDirection: 'column',
  width: '100%',
  background: 'var(--surface-raised)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-3)',
  overflow: 'hidden',
  outline: 'none'
}

const filterRowStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--control-h)',
  flex: 'none',
  padding: '0 var(--space-5)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}

const inputStyle = {
  flex: 1,
  minWidth: 0,
  border: 'none',
  outline: 'none',
  background: 'transparent',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-mono)'
}

const countStyle = {
  flex: 'none',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)',
  whiteSpace: 'nowrap'
}

/* Nine rows and then it scrolls. Written as the arithmetic and never as the
   252px it comes to in the comfortable density: the height of a row moves with
   both the density and `--ui-scale`, and a number here would be the one thing
   in the component that stayed still while everything inside it grew — which is
   precisely the failure this window is being rebuilt to stop. */
const listStyle = {
  maxHeight: 'calc(9 * var(--row-h))',
  overflowY: 'auto',
  display: 'flex',
  flexDirection: 'column'
}

/* A row is the full width of the list, which is the whole of what this
   component fixes: nothing here is ellipsised, so a name of any length is read
   rather than guessed at. A long one wraps and takes the row with it, which is
   why the height is a floor rather than a height.

   The highlight is a surface step and the selection is a surface step and a
   bar — never a colour change and never a transform, so a row cannot jump under
   the pointer in a list this dense. The bar's width is spent on every row,
   transparent where there is nothing to mark, so that picking a row does not
   shift the name beside it by three pixels. */
const rowStyle = (row, index) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  width: '100%',
  minHeight: 'var(--row-h)',
  flexShrink: 0,
  padding: 'var(--space-2) var(--space-5)',
  textAlign: 'left',
  border: 'none',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  borderLeft: `var(--accent-bar-w) solid ${isSelected(row) ? 'var(--focus-ring)' : 'transparent'}`,
  background: isSelected(row)
    ? 'var(--surface-selected)'
    : index === cursor.value
      ? 'var(--surface-hover)'
      : 'transparent',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-mono)',
  cursor: 'default',
  transition: 'var(--transition-control)'
})

const nameStyle = {
  flex: '1 1 auto',
  minWidth: 0,
  /* The one place a break is allowed inside a word: a branch name is one word
     to the browser, and a name longer than the list has to wrap somewhere
     rather than push the meta off the end. */
  overflowWrap: 'anywhere'
}

const prefixStyle = { color: 'var(--text-muted)' }

const metaStyle = {
  flex: 'none',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)',
  whiteSpace: 'nowrap'
}

const emptyStyle = {
  padding: 'var(--space-5)',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)'
}

/* The strip under the list: what the keys do on the left, what a choice here
   applies to on the right. On `--surface` rather than the shell's own
   `--surface-raised`, which is what makes it read as a footing under the list
   instead of a tenth row of it. */
const footerStyle = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 'var(--space-4)',
  flex: 'none',
  height: 'var(--control-h-sm)',
  padding: '0 var(--space-5)',
  background: 'var(--surface)',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)'
}

const footerEndStyle = {
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
</script>

<template>
  <div :style="shellStyle" @keydown="onKeydown">
    <div :style="filterRowStyle">
      <Icon name="search" :size="GLYPH" :style="{ color: 'var(--text-muted)' }" />
      <input
        ref="filterField"
        v-model="query"
        :style="inputStyle"
        :placeholder="BRANCH_FILTER_LABEL"
        :aria-label="BRANCH_FILTER_LABEL"
      />
      <span :style="countStyle">{{ count }}</span>
      <IconButton icon="x" label="Close" size="sm" @click="emit('close')" />
    </div>

    <div ref="listBox" :style="listStyle">
      <!-- A row is a real `<button>`: the browser gives it the semantics and the
           click, and the styles above take away everything it draws by default.
           The pointer moves the highlight as well as pressing the row, so that
           Enter always lands where the eye is — the same rule `Dropdown.vue`
           keeps for the same reason. -->
      <button
        v-for="(row, i) in rows"
        :key="row.key"
        type="button"
        :style="rowStyle(row, i)"
        @mouseenter="cursor = i"
        @click="choose(row)"
      >
        <!-- The cloud is what says a row is not on this machine. It is drawn
             beside the prefix rather than instead of it: `origin/` is part of
             what would be typed, and the glyph is what makes the pair of rows
             tell themselves apart at a glance. -->
        <Icon
          :name="row.origin ? 'cloud' : 'git-branch'"
          :size="GLYPH"
          :style="{ flex: 'none', color: 'var(--text-muted)' }"
        />
        <span :style="nameStyle"
          ><span v-if="row.origin" :style="prefixStyle">origin/</span>{{ row.name }}</span
        >
        <span :style="metaStyle">{{ row.meta }}</span>
      </button>
      <div v-if="!rows.length" :style="emptyStyle">{{ NO_BRANCH_MATCHES }}</div>
    </div>

    <div :style="footerStyle">
      <span :style="footerEndStyle">{{ PICKER_KEY_HINT }}</span>
      <span :style="footerEndStyle">{{ scope }}</span>
    </div>
  </div>
</template>
