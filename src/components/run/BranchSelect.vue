<script setup>
/* Choosing a branch to merge into, or naming one that does not exist yet.

   The panel, the filter and the list are `Dropdown`'s — this adds the two
   things only a branch field needs: a way to name a branch that is not there,
   and the state the field enters while that name is being typed.

   Naming replaces the panel rather than living inside it, because the two ask
   different questions: one is "which of these", the other is "what shall it be
   called", and a list underneath an empty name field invites picking from it
   and losing what you typed. */
import { computed, nextTick, ref } from 'vue'
import Dropdown from '../core/Dropdown.vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import { branchHint, branchOptions, needsCutting } from './branchChoice.js'

const props = defineProps({
  modelValue: { type: String, default: '' },
  branches: { type: Array, default: () => [] },
  disabled: { type: Boolean, default: false }
})

/* `create` says the chosen name is missing from at least one repository — which
   includes a name nothing carries, and also a branch that is right there in the
   list and short of one of four. Both sites ask `needsCutting`, never "is it in
   `branches`": the run has to carry permission to cut the branch where it turns
   out to be absent, and this control is the only place that has seen the list. */
const emit = defineEmits(['update:modelValue', 'update:create'])

const naming = ref(false)
const draft = ref('')
const nameField = ref(null)
const dropdown = ref(null)

const options = computed(() => branchOptions(props.branches))

/* A name typed here is new until it turns out not to be, and a branch picked
   from the list may still need cutting where a repository lacks it. Which of
   the two sentences that is worth is `branchChoice.js`'s, like every other rule
   of this field and for the same reason — a `.vue` is the one thing no test in
   this repository can reach, so no wording of it lives here. How it is drawn is
   `Dropdown`'s: an info glyph with the sentence in its tooltip, since a field
   holding a branch name has room for one icon and not for a phrase. */
const hint = computed(() => branchHint(props.branches, props.modelValue))

const startNaming = async (closePanel) => {
  closePanel()
  naming.value = true
  draft.value = ''
  await nextTick()
  nameField.value?.focus()
}

/* A double-click is the one other way in, and it starts from the name
   already there rather than a blank one — the opposite of "+ New branch",
   which is always a fresh name. The first click of the pair has already
   opened the panel; closing it here is what a single click promised and
   returns the keyboard to the field on its own, same as any other close. The
   text is then selected whole, Finder-rename style: typing replaces the name,
   and a click inside the input is how somebody fixes one letter of it. */
const startRenaming = async () => {
  if (props.disabled) return
  dropdown.value?.close()
  naming.value = true
  draft.value = props.modelValue
  await nextTick()
  nameField.value?.focus()
  nameField.value?.select()
}

/* Enter and losing focus both commit, which is what the field looks like it
   promises. An empty name commits nothing and simply leaves — there is no
   branch called "", and refusing with a message would be ceremony for somebody
   who has plainly changed their mind. */
const commitName = () => {
  if (!naming.value) return
  const name = draft.value.trim()
  naming.value = false
  if (!name) return
  emit('update:modelValue', name)
  emit('update:create', needsCutting(props.branches, name))
}

/* The X, and Escape. Nothing typed is applied and the control goes back to
   where it was — including the branch chosen before, which is why this does not
   touch modelValue. */
const cancelNaming = () => {
  naming.value = false
  draft.value = ''
}

const pick = (branch) => {
  emit('update:modelValue', branch)
  emit('update:create', needsCutting(props.branches, branch))
}

/* The naming field borrows the dropdown's own field silhouette so the control
   does not change shape underneath the pointer when it changes mode. */
const fieldStyle = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  width: '100%',
  height: 'var(--control-h)',
  padding: '0 var(--space-3) 0 var(--space-4)',
  background: 'var(--surface-raised)',
  color: 'var(--text-primary)',
  border: 'var(--border-w) solid var(--focus-ring)',
  borderRadius: 'var(--radius-3)'
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
</script>

<template>
  <!-- naming: the field itself becomes the input, so the name is typed exactly
       where the branch will be read afterwards. -->
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
    <!-- Pointerdown, not click: the input's blur fires first and would commit
         the very name this button exists to discard. -->
    <IconButton icon="x" label="Discard this name" size="sm" @pointerdown.prevent="cancelNaming" />
  </div>

  <Dropdown
    v-else
    ref="dropdown"
    :model-value="modelValue"
    :options="options"
    :disabled="disabled"
    searchable
    search-label="Search branches"
    mono
    placeholder="Pick a branch"
    :hint="hint"
    @update:model-value="pick"
    @dblclick="startRenaming"
  >
    <template #header="{ close }">
      <button type="button" :style="newRowStyle" @click="startNaming(close)">
        <Icon name="plus" :size="13" />
        New branch
      </button>
    </template>
    <template #empty>No branch matches. Use “New branch” to make one.</template>
  </Dropdown>
</template>
