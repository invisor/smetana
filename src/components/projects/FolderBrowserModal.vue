<script setup>
/* Adding a project when the server is on another machine: the server's folders, and a way to clone
   a repository onto it.

   The native folder dialog shows this machine's disk, which a remote server cannot open, so this
   shows the server's instead - a listing it sends, confined to its own root, a step at a time. Two
   ways to a project, in two small modes of one window: pick a folder that is already there, or give
   an address and let the server `git clone` it into the folder being looked at. Both end the same
   way, with a path on the server that the app then adds as it adds any project.

   Presentational, like every component here: handed the listing and what is in flight, emitting
   what was pressed. The app window talks to the server (`stores/projects.js`) and announces the
   state to this window, so this renders in `?view=gallery` with nothing behind it. The rules - what
   an address may be, how the path is shown as steps - are `folderBrowser.js`'s, and the server checks
   every one of them again. */
import { computed, ref } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'
import Input from '../core/Input.vue'
import EmptyState from '../core/EmptyState.vue'
import { EMPTY_FOLDER, cloneName, cloneProblem, listingNote, steps } from './folderBrowser.js'

const props = defineProps({
  open: { type: Boolean, default: true },
  /* The server's listing: `{ root, path, parent, entries, truncated, repo, tracker }`, or `null`
     before the first answer. */
  listing: { type: Object, default: null },
  /* A listing is being read. The last one stays on screen, dimmed, so a click does not blink the
     list empty and back. */
  loading: { type: Boolean, default: false },
  /* What the server refused a listing with. */
  error: { type: String, default: null },
  /* A clone is running (it can take minutes), and what the last one failed with. */
  cloning: { type: Boolean, default: false },
  cloneError: { type: String, default: null },
  /* Which of the two modes it opens in: `browse`, or `clone` (the gallery draws both). */
  startIn: { type: String, default: 'browse' }
})

const emit = defineEmits(['close', 'open', 'choose', 'clone'])

const mode = ref(props.startIn === 'clone' ? 'clone' : 'browse')
const url = ref('')
const name = ref('')

const bar = computed(() => steps(props.listing?.root, props.listing?.path))
const note = computed(() => listingNote(props.listing))
const entries = computed(() => props.listing?.entries ?? [])
const problem = computed(() => cloneProblem(url.value, name.value.trim()))
const target = computed(() => name.value.trim() || cloneName(url.value))
const canClone = computed(
  () => url.value.trim() !== '' && problem.value === null && Boolean(props.listing) && !props.cloning
)

const clone = () => {
  if (!canClone.value) return
  emit('clone', { url: url.value.trim(), parent: props.listing.path, name: name.value.trim() || null })
}

const barStyle = {
  display: 'flex',
  alignItems: 'center',
  flexWrap: 'wrap',
  gap: 'var(--space-2)',
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-mono)',
  color: 'var(--text-secondary)'
}
const reset = { appearance: 'none', background: 'transparent', border: 'none', margin: 0, textAlign: 'left' }
const stepStyle = (last) => ({
  ...reset,
  font: 'inherit',
  cursor: last ? 'default' : 'pointer',
  color: last ? 'var(--text-primary)' : 'var(--text-secondary)',
  padding: 'var(--space-1) var(--space-2)',
  borderRadius: 'var(--radius-2)'
})
const listStyle = (dim) => ({
  display: 'flex',
  flexDirection: 'column',
  marginTop: 'var(--space-4)',
  maxHeight: 'calc(var(--row-h) * 9)',
  overflowY: 'auto',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)',
  opacity: dim ? 'var(--attn-quiet-opacity)' : 1
})
const rowStyle = {
  ...reset,
  boxSizing: 'border-box',
  width: '100%',
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  minHeight: 'var(--row-h)',
  padding: '0 var(--space-4)',
  cursor: 'pointer',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-ui-size)/var(--leading-snug) var(--font-sans)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const tagStyle = {
  marginLeft: 'auto',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-sans)'
}
const noteStyle = {
  marginTop: 'var(--space-3)',
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
const errorStyle = { ...noteStyle, color: 'var(--status-failed-fg)' }
const fieldStyle = { display: 'flex', flexDirection: 'column', gap: 'var(--space-2)', marginTop: 'var(--space-4)' }
const labelStyle = {
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-sans)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}
</script>

<template>
  <Modal
    :open="open"
    :closable="!cloning"
    title="Add project from the server"
    description="Pick a folder on the server, or clone a repository into the one you are looking at."
    @close="emit('close')"
  >
    <div :style="barStyle" role="navigation" aria-label="Folder path">
      <template v-for="(step, at) in bar" :key="step.path">
        <Icon v-if="at > 0" name="chevron-right" :size="12" />
        <button
          type="button"
          :style="stepStyle(at === bar.length - 1)"
          :disabled="at === bar.length - 1 || cloning"
          @click="emit('open', step.path)"
        >
          {{ step.name }}
        </button>
      </template>
    </div>

    <template v-if="mode === 'browse'">
      <div :style="listStyle(loading)">
        <button
          v-if="listing && listing.parent"
          type="button"
          :style="rowStyle"
          @click="emit('open', listing.parent)"
        >
          <Icon name="arrow-up" />
          <span>Up one folder</span>
        </button>
        <button v-for="entry in entries" :key="entry.path" type="button" :style="rowStyle" @click="emit('open', entry.path)">
          <Icon :name="entry.repo ? 'folder-git-2' : 'folder'" />
          <span>{{ entry.name }}</span>
          <span v-if="entry.tracker" :style="tagStyle">tracker</span>
        </button>
        <EmptyState
          v-if="listing && entries.length === 0 && !loading && !error"
          compact
          icon="folder"
          :title="EMPTY_FOLDER"
        />
      </div>
      <p v-if="note" :style="noteStyle">{{ note }}</p>
      <p v-if="error" :style="errorStyle">{{ error }}</p>
    </template>

    <template v-else>
      <div :style="fieldStyle">
        <span :style="labelStyle">Repository address</span>
        <Input
          v-model="url"
          mono
          :invalid="problem !== null"
          :disabled="cloning"
          placeholder="https://github.com/team/app.git"
          @keydown.enter="clone"
        />
      </div>
      <div :style="fieldStyle">
        <span :style="labelStyle">Folder name</span>
        <Input v-model="name" mono :disabled="cloning" :placeholder="cloneName(url) ?? 'app'" @keydown.enter="clone" />
      </div>
      <p v-if="problem" :style="errorStyle">{{ problem }}</p>
      <p v-else-if="listing" :style="noteStyle">
        {{ target ? `It will be cloned to ${listing.path}/${target}.` : `It will be cloned into ${listing.path}.` }}
        The server uses its own git credentials.
      </p>
      <p v-if="cloning" :style="noteStyle">Cloning… this can take a while for a large repository.</p>
      <p v-if="cloneError" :style="errorStyle">{{ cloneError }}</p>
    </template>

    <template #footer>
      <Button variant="ghost" :disabled="cloning" @click="emit('close')">Cancel</Button>
      <Button
        v-if="mode === 'browse'"
        variant="secondary"
        icon="git-branch"
        @click="mode = 'clone'"
      >
        Clone a repository…
      </Button>
      <Button v-else variant="secondary" :disabled="cloning" @click="mode = 'browse'">Back to folders</Button>
      <Button
        v-if="mode === 'browse'"
        variant="primary"
        :disabled="!listing || loading"
        @click="emit('choose', listing.path)"
      >
        Add this folder
      </Button>
      <Button v-else variant="primary" :disabled="!canClone" @click="clone">
        {{ cloning ? 'Cloning…' : 'Clone and add' }}
      </Button>
    </template>
  </Modal>
</template>
