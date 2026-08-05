<script setup>
/* Choosing a branch to merge into, or naming one that does not exist yet.

   `Select` cannot do this — a native select has no search and nothing to hang
   a "new branch" action on, and a repository with sixty branches makes the
   difference between scrolling and typing three letters. So this is the one
   control in the run dialog that is built rather than reused, and it is built
   out of the same parts everything else is: a field, a panel, and no CSS.

   The whole thing has three states, and they are exclusive on purpose:

     closed  — a field showing the chosen branch
     open    — the panel: "new branch" first, then a filter, then the matches
     naming  — the field has become a text input for a name that does not exist

   `naming` replaces the panel rather than living inside it, because the two
   ask different questions: one is "which of these", the other is "what shall
   it be called", and a list underneath an empty name field invites picking
   from it and losing what you typed. */
import { computed, nextTick, ref, watch } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'

const props = defineProps({
  modelValue: { type: String, default: '' },
  branches: { type: Array, default: () => [] },
  disabled: { type: Boolean, default: false }
})

/* `create` says the chosen name is not one of `branches` — the run has to know,
   because a branch that does not exist has to be cut before anything merges
   into it, and only this control knows the difference. */
const emit = defineEmits(['update:modelValue', 'update:create'])

const open = ref(false)
const naming = ref(false)
const query = ref('')
const draft = ref('')
const cursor = ref(0)
const root = ref(null)
const filterField = ref(null)
const nameField = ref(null)

const matches = computed(() => {
  const needle = query.value.trim().toLowerCase()
  if (!needle) return props.branches
  return props.branches.filter((b) => b.toLowerCase().includes(needle))
})

/* A name typed here is new until it turns out not to be: somebody who types
   the name of a branch that already exists means that branch, and telling the
   run to create it would fail on the first command. */
const isNew = computed(() => props.modelValue !== '' && !props.branches.includes(props.modelValue))

watch(matches, () => {
  cursor.value = 0
})

const openPanel = async () => {
  if (props.disabled || naming.value) return
  open.value = true
  query.value = ''
  cursor.value = Math.max(0, props.branches.indexOf(props.modelValue))
  await nextTick()
  filterField.value?.focus()
}

const closePanel = () => {
  open.value = false
  query.value = ''
}

const choose = (branch) => {
  emit('update:modelValue', branch)
  emit('update:create', false)
  closePanel()
}

const startNaming = async () => {
  closePanel()
  naming.value = true
  draft.value = ''
  await nextTick()
  nameField.value?.focus()
}

/* Enter and losing focus both commit, which is what the field looks like it
   promises. An empty name commits nothing and simply leaves — there is no
   branch called "", and refusing with a message would be a lot of ceremony for
   somebody who has plainly changed their mind. */
const commitName = () => {
  if (!naming.value) return
  const name = draft.value.trim()
  naming.value = false
  if (!name) return
  emit('update:modelValue', name)
  emit('update:create', !props.branches.includes(name))
}

/* The X, and Escape. Nothing typed is applied and the control goes back to
   where it was — including the branch that was chosen before, which is why
   this does not touch modelValue. */
const cancelNaming = () => {
  naming.value = false
  draft.value = ''
}

const onFilterKey = (event) => {
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    const step = event.key === 'ArrowDown' ? 1 : -1
    const n = matches.value.length
    if (n) cursor.value = (cursor.value + step + n) % n
  } else if (event.key === 'Enter') {
    event.preventDefault()
    const pick = matches.value[cursor.value]
    if (pick) choose(pick)
  } else if (event.key === 'Escape') {
    event.preventDefault()
    closePanel()
  }
}

/* Pointerdown rather than click: a click that starts inside the panel and ends
   outside it — a drag over the list — would otherwise close the panel out from
   under the pointer. */
const onDocumentPointerdown = (event) => {
  if (!root.value?.contains(event.target)) closePanel()
}

watch(open, (isOpen) => {
  if (isOpen) document.addEventListener('pointerdown', onDocumentPointerdown, true)
  else document.removeEventListener('pointerdown', onDocumentPointerdown, true)
})

const fieldStyle = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  width: '100%',
  height: 'var(--control-h)',
  padding: '0 var(--space-3) 0 var(--space-4)',
  background: props.disabled ? 'var(--surface-sunken)' : 'var(--surface-raised)',
  color: props.disabled ? 'var(--text-muted)' : 'var(--text-primary)',
  border: `var(--border-w) solid ${open.value || naming.value ? 'var(--focus-ring)' : 'var(--border)'}`,
  borderRadius: 'var(--radius-3)',
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-sans)',
  cursor: props.disabled ? 'not-allowed' : 'default',
  textAlign: 'left'
}))

const inputStyle = {
  flex: 1,
  minWidth: 0,
  height: '100%',
  border: 'none',
  outline: 'none',
  background: 'transparent',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-mono)'
}

const valueStyle = {
  flex: 1,
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-mono)'
}

const panelStyle = {
  position: 'absolute',
  top: 'calc(100% + var(--space-2))',
  left: 0,
  right: 0,
  zIndex: 'var(--z-dropdown)',
  display: 'flex',
  flexDirection: 'column',
  background: 'var(--surface-overlay)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-3)',
  boxShadow: 'var(--shadow-overlay)',
  overflow: 'hidden'
}

const newRowStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-4)',
  background: 'transparent',
  border: 'none',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  color: 'var(--text-primary)',
  font: 'var(--weight-medium) var(--text-sm)/1 var(--font-sans)',
  cursor: 'default',
  width: '100%',
  textAlign: 'left'
}

const filterStyle = {
  height: 'var(--row-h)',
  padding: '0 var(--space-4)',
  border: 'none',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  outline: 'none',
  background: 'transparent',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-mono)',
  width: '100%'
}

/* Eight rows and then it scrolls: enough to see a repository's working set
   without the panel reaching the bottom of the dialog. */
const listStyle = {
  maxHeight: 'calc(8 * var(--row-h))',
  overflowY: 'auto',
  display: 'flex',
  flexDirection: 'column'
}

const optionStyle = (branch, index) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-4)',
  border: 'none',
  width: '100%',
  textAlign: 'left',
  /* The keyboard cursor and the chosen value are different things and are
     drawn differently: a surface step for where the keyboard is, a check for
     what is chosen. Colour is never the only signal here either. */
  background: index === cursor ? 'var(--surface-hover)' : 'transparent',
  color: branch === props.modelValue ? 'var(--text-primary)' : 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-mono)',
  cursor: 'default'
})

const emptyStyle = {
  padding: 'var(--space-4)',
  fontSize: 'var(--text-xs)',
  color: 'var(--text-muted)',
  fontFamily: 'var(--font-sans)'
}
</script>

<template>
  <div ref="root" :style="{ position: 'relative', width: '100%' }">
    <!-- naming: the field itself becomes the input, so the name is typed
         exactly where the branch will be read afterwards. -->
    <div v-if="naming" :style="fieldStyle">
      <input
        ref="nameField"
        v-model="draft"
        :style="inputStyle"
        placeholder="New branch name"
        aria-label="New branch name"
        @keydown.enter.prevent="commitName"
        @keydown.esc.prevent="cancelNaming"
        @blur="commitName"
      />
      <!-- Pointerdown, not click: the input's blur fires first and would
           commit the very name this button exists to discard. -->
      <IconButton icon="x" label="Discard this name" size="sm" @pointerdown.prevent="cancelNaming" />
    </div>

    <button v-else type="button" :disabled="disabled" :style="fieldStyle" @click="openPanel">
      <span :style="valueStyle">{{ modelValue || 'Pick a branch' }}</span>
      <span v-if="isNew" :style="{ fontSize: 'var(--text-2xs)', color: 'var(--text-muted)', whiteSpace: 'nowrap' }">
        will be created
      </span>
      <Icon name="chevron-down" :size="14" :style="{ color: 'var(--text-muted)' }" />
    </button>

    <div v-if="open" :style="panelStyle">
      <button type="button" :style="newRowStyle" @click="startNaming">
        <Icon name="plus" :size="13" />
        New branch
      </button>
      <input
        ref="filterField"
        v-model="query"
        :style="filterStyle"
        placeholder="Search branches"
        aria-label="Search branches"
        @keydown="onFilterKey"
      />
      <div :style="listStyle">
        <button
          v-for="(branch, i) in matches"
          :key="branch"
          type="button"
          :style="optionStyle(branch, i)"
          @mouseenter="cursor = i"
          @click="choose(branch)"
        >
          <Icon
            name="check"
            :size="12"
            :style="{ visibility: branch === modelValue ? 'visible' : 'hidden' }"
          />
          <span :style="valueStyle">{{ branch }}</span>
        </button>
        <div v-if="!matches.length" :style="emptyStyle">
          No branch matches. Use “New branch” to make one.
        </div>
      </div>
    </div>
  </div>
</template>
