<script setup>
/* The Storage tab: how much the attachment store weighs, and the one button in
   this app that deletes somebody's files.

   Two rows, because there are two questions and they have different answers.
   The first is about the whole directory — every project's images and whatever
   is left in its root — since a person asking how big this has got is asking
   about the folder. The second is about what a press would take, which is only
   ever the active project's own share of it: the store is laid out by project
   and the sweep is given one directory, so another project's images are out of
   reach rather than spared by a condition.

   **Nothing is deleted until somebody presses.** The app removes no attachment
   at any other moment — not on start, not on a schedule, not when the new-task
   dialog closes on images nobody filed. That is why the numbers are on screen
   before the button rather than in a report after it.

   Presentational, like every component here: handed values, emitting what was
   pressed. The window does the asking and the telling, so this renders in
   `?view=gallery` with nothing behind it. */
import { computed } from 'vue'
import Button from '../core/Button.vue'
import SettingsRow from './SettingsRow.vue'
import { canClear, removalLine, removedLine, scopeLine, storageLine } from './storage.js'

const props = defineProps({
  /* `attachments_survey`'s answer whole, in Rust's own shape, or `null` while
     it is still on its way. */
  survey: { type: Object, default: null },
  /* Something is in flight: reading the store, or sweeping it. Both hold the
     button, since pressing twice would ask a worker mid-sweep to sweep again. */
  busy: { type: Boolean, default: false },
  /* What the last press or read was refused with, in one readable line. */
  error: { type: String, default: null },
  /* `attachments_clean`'s answer, once there has been a press: what actually
     went, which need not be what was offered a moment earlier. */
  cleaned: { type: Object, default: null }
})

const emit = defineEmits(['clear'])

const size = computed(() => storageLine(props.survey))
const removal = computed(() => removalLine(props.survey))
const scope = computed(() => scopeLine(props.survey))
const outcome = computed(() => removedLine(props.cleaned))
const ready = computed(() => canClear(props.survey) && !props.busy)

/* The size reads as a fact rather than as a control: mono, because it is a
   measurement, and secondary, because the label beside it is the thing being
   named. */
const sizeStyle = {
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-ui-size)/1 var(--font-mono)',
  whiteSpace: 'nowrap'
}
/* Under the rows rather than inside one: both of these are about what just
   happened, not about a setting, and a row's description column is for what a
   control cannot say itself. */
const noteStyle = {
  marginTop: 'var(--space-4)',
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
const errorStyle = {
  marginTop: 'var(--space-4)',
  color: 'var(--status-failed-fg)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
</script>

<template>
  <div>
    <SettingsRow
      label="Attachment storage"
      description="Images attached to a task are copied here, so a screenshot deleted from your desktop still opens from the issue."
      control-width="22ch"
    >
      <span :style="sizeStyle">{{ size }}</span>
    </SettingsRow>
    <SettingsRow label="Clear storage" :description="removal" control-width="18ch">
      <Button variant="danger" icon="trash-2" :disabled="!ready" @click="emit('clear')">
        Clear storage
      </Button>
    </SettingsRow>
    <p v-if="scope" :style="noteStyle">{{ scope }}</p>
    <p v-if="outcome" :style="noteStyle">{{ outcome }}</p>
    <p v-if="props.error" :style="errorStyle">{{ props.error }}</p>
  </div>
</template>
