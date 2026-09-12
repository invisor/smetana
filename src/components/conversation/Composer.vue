<script setup>
/* The person's half of a driven session: the words they have not sent yet, the
   files going with them, and one button.

   **One control, not two.** While a turn is in flight the send button *is* the
   stop button — a person cannot want both at once, and a pair sitting side by
   side with one of them always dead is two controls to read where there is one
   decision to make. Enter follows the same rule rather than a rule of its own:
   it sends while there is something to send and does nothing while the agent is
   working, because at that moment there is no send affordance on the panel for
   it to be the keyboard equivalent of.

   The field grows to a ceiling and then scrolls. The ceiling is a multiple of
   `--row-h`, so it follows the density and the app-wide font size the way every
   other height in the system does; what the growth itself is measured in is the
   browser's own `scrollHeight`, which is a fact about the text that has been
   typed rather than a value this design system chooses.

   An attachment is a path and nothing more — that is what `session_send`
   carries — so a chip draws `basename` from `src/paths.js` and never a second
   copy of that function. **Nothing in this component puts a path into the
   list**: they arrive as a prop and leave through `update:attachments`, which is
   what keeps this drawable in `?view=gallery` with no window and no desktop
   behind it.

   **`waiting` is a second flag and deliberately not folded into `busy`.**
   `busy` means a turn is in flight and draws Stop; while the agent is holding
   an open question or an open permission request it is not busy at all — there
   is nothing to stop, and Stop would be a lie. `waiting` locks the field and
   the send affordance instead, and refuses rather than pretends, the same
   discipline `nothingToSend` already keeps: the field turns the colour a
   disabled `Input` does and says in words that something above it is waiting
   on an answer, since a control that quietly does nothing reads as a broken
   app. It says only that much and never which of the two calls is open —
   `ConversationView.vue` derives one flag off the same `question` it already
   computes for either card, and this component has no business knowing which
   of them it was. */
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import { basename } from '../../paths.js'

const props = defineProps({
  /* The unsent words. `v-model`, so whoever draws this decides where a draft
     lives — in the conversation panel it is the store's, which is what makes it
     outlive a tab switch. */
  modelValue: { type: String, default: '' },
  /* Absolute paths, in the order they were attached. */
  attachments: { type: Array, default: () => [] },
  /* A turn is in flight: the one button is Stop. */
  busy: { type: Boolean, default: false },
  /* The agent is holding a question or a permission request open above this
     field and will not go on until it is answered. The field and Send refuse
     input the same way a disabled control does everywhere else in this
     system, and say why in words. Never true at once with `busy` in practice
     — the two describe different states of the same session — and where it
     matters, `busy` still wins the button: see the template. */
  waiting: { type: Boolean, default: false }
})

const emit = defineEmits(['update:modelValue', 'update:attachments', 'send', 'stop'])

const field = ref(null)
const focus = ref(false)

/* How tall the field is right now, and why it is a ref in the style object
   rather than a line written onto the element.

   A textarea's height is the browser's `rows` attribute and nothing else, so
   there is no declarative way to say "as tall as what is in it": the content
   has to be measured. It is measured at `height: auto`, where `scrollHeight` is
   exactly the border box this field wants under the global `box-sizing:
   border-box` — the field carries no border of its own, the frame around it
   does. Writing the answer back through the same computed the rest of the style
   comes from is what keeps `:style` the one thing that decides how this element
   looks; an imperative `el.style.height` would be a second writer to the same
   property, and Vue's own patch is the other. */
const grown = ref('auto')

async function measure() {
  if (!field.value) return
  grown.value = 'auto'
  await nextTick()
  /* The panel can be taken away between the two halves of a measurement — a
     tab switch is one tick, and this awaits one. */
  if (!field.value) return
  /* A field with no layout behind it measures zero, and a height of zero is one
     a person cannot type their way back out of: nothing is drawn, so nothing
     grows it. `auto` is the honest answer there and it is also the right one —
     it is the single row `rows="1"` asks for. */
  const height = field.value.scrollHeight
  grown.value = height > 0 ? `${height}px` : 'auto'
}

watch(() => props.modelValue, measure)
onMounted(measure)

/* Nothing to send: no words and no files. The button is refused rather than
   pressed into nothing — `sendMessage` in the store refuses the same message
   for the same reason, and a control that looks available and does nothing is
   the thing that reads as a broken app. Blank text with a file on it is an
   ordinary message and does go: a picture is a thing to say. */
const nothingToSend = computed(() => !props.modelValue.trim() && props.attachments.length === 0)

function onKeydown(event) {
  if (event.key !== 'Enter' || event.shiftKey) return
  /* A keystroke inside an input method's composition — picking a candidate with
     Enter in Japanese, Chinese or Korean — is that method's and not this
     panel's. Without this guard, choosing a word sends the message.  */
  if (event.isComposing) return
  event.preventDefault()
  if (props.busy || props.waiting || nothingToSend.value) return
  emit('send')
}

const remove = (path) =>
  emit(
    'update:attachments',
    props.attachments.filter((other) => other !== path)
  )

const root = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  padding: 'var(--panel-pad)',
  background: 'var(--surface)',
  fontFamily: 'var(--font-sans)'
}

const strip = { display: 'flex', flexWrap: 'wrap', gap: 'var(--space-2)' }

/* The same chip `UserMessage.vue` draws a sent attachment as, plus the one
   thing that tells the two apart: this one can be taken off again. A file's
   name is an identifier, so mono. */
const chip = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  maxWidth: '100%',
  minWidth: 0,
  height: 'var(--control-h-sm)',
  padding: '0 var(--space-2) 0 var(--space-3)',
  background: 'var(--surface-raised)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)',
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)'
}

const chipName = { minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }

/* Smaller than the `sm` box, the way `Tab.vue` sizes its own close: an ordinary
   icon button is as tall as the chip it would sit inside. */
const chipClose = { width: 'var(--icon-sm)', height: 'var(--icon-sm)', flex: '0 0 auto' }

/* The frame is the row's and not the field's, and that is what makes the
   measurement above exact: `scrollHeight` counts padding and leaves out
   borders, so a border on the element being measured would come back one
   border-width short every keystroke. It also puts the button inside the same
   box as the words, which is what says the two are one control surface.

   `waiting` borrows `Input.vue`'s own disabled treatment — the sunken ground,
   the same focus ring is simply never reached because the field cannot take
   focus in a way that matters while it is read-only — rather than inventing a
   second "locked" look this system has no token for. */
const row = computed(() => ({
  display: 'flex',
  alignItems: 'flex-end',
  gap: 'var(--space-3)',
  padding: 'var(--space-3)',
  background: props.waiting ? 'var(--surface-sunken)' : 'var(--surface-raised)',
  border: `var(--border-w) solid ${focus.value && !props.waiting ? 'var(--focus-ring)' : 'var(--border)'}`,
  borderRadius: 'var(--radius-3)',
  boxShadow: focus.value && !props.waiting ? 'inset 0 0 0 1px var(--focus-ring)' : 'none',
  transition: 'var(--transition-control)'
}))

const fieldStyle = computed(() => ({
  flex: 1,
  minWidth: 0,
  height: grown.value,
  /* Six rows, and then it scrolls. A multiple of the row height rather than a
     number of its own: the ceiling has to move with the density and with the
     app-wide font size, or the field holds a different amount of text on every
     machine. */
  maxHeight: 'calc(var(--row-h) * 6)',
  padding: 'var(--space-2) var(--space-3)',
  background: 'transparent',
  color: props.waiting ? 'var(--text-muted)' : 'var(--text-primary)',
  border: 0,
  outline: 'none',
  /* Prose, so sans — a person's message to an agent is a sentence and not an
     identifier, whatever paths it happens to hold. */
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  overflowY: 'auto',
  cursor: props.waiting ? 'not-allowed' : 'text',
  /* The grip is a browser-drawn control with no place in this system, and the
     field sizes itself anyway. */
  resize: 'none'
}))

/* The field's own reason, read only while it is locked. Generic on purpose —
   see the header on `waiting` above: this component draws the same sentence
   whether a question or a permission request is what is open, and points at
   "above" rather than naming either. */
const lockHint = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)'
}
</script>

<template>
  <div :style="root">
    <div v-if="attachments.length" :style="strip">
      <span v-for="path in attachments" :key="path" :style="chip">
        <Icon name="paperclip" :size="11" />
        <span :style="chipName">{{ basename(path) }}</span>
        <IconButton
          icon="x"
          :label="`Remove ${basename(path)}`"
          size="sm"
          :style="chipClose"
          @click="remove(path)"
        />
      </span>
    </div>

    <div :style="row">
      <textarea
        ref="field"
        rows="1"
        :value="modelValue"
        placeholder="Message the agent"
        :readonly="waiting"
        :aria-disabled="waiting || undefined"
        :style="fieldStyle"
        @input="emit('update:modelValue', $event.target.value)"
        @keydown="onKeydown"
        @focus="focus = true"
        @blur="focus = false"
      />
      <!-- One control in two states, and never two controls: see the header.
           `busy` wins the button outright — a turn in flight is never also a
           question waiting on an answer — and `waiting` only ever reaches the
           `disabled` branch below it. -->
      <Button v-if="busy" size="sm" icon="square" @click="emit('stop')">Stop</Button>
      <Button
        v-else
        size="sm"
        variant="primary"
        icon="arrow-up"
        :disabled="nothingToSend || waiting"
        @click="emit('send')"
      >Send</Button>
    </div>

    <div v-if="waiting" :style="lockHint">
      <Icon name="lock" :size="11" />
      <span>Answer above to keep going</span>
    </div>
  </div>
</template>
