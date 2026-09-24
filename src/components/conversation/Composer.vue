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
   behind it. The paperclip button beside the field is the same rule at the
   other end — a system file picker is exactly the kind of thing this component
   may not open for itself, so a click raises `attach` with nothing attached to
   it and leaves opening the dialog, and putting whatever it returns into the
   list, to whoever draws this (`ConversationView.vue`, through `pickFiles` in
   `stores/app.js`). It is never disabled by `busy`: a drop onto the panel is
   already allowed while a turn is in flight, and a file attached mid-turn
   simply travels with the next message, the same way a dropped one already
   does.

   **The name and the cross are two controls now, not one (smetana-4x3w) — and
   the name is a control at all only for some attachments.** The cross always
   removes; clicking an openable name raises `open-attachment` with the bare
   path and leaves deciding what that means to whoever is drawing this —
   `ConversationView.vue`, which already owns the identical decision for a
   figure and a local link in the prose above the field, through
   `attachmentAction.js` beside it. This component cannot ask that question
   itself (it stays drawable in `?view=gallery` with no store and no window
   behind it, see the header above), so `openAttachments` arrives as a prop:
   the subset of `attachments` worth a click. A path outside it draws as
   plain text, `chipNameStatic` below rather than `chipNameStyle`, with no
   button, no hover and no cursor of its own — there is genuinely nowhere for
   such a click to go (an editor tab needs the attachment inside the open
   project, and the image window only takes a picture), so a control that
   looked pressable and did nothing would be the one lie a disabled state
   elsewhere in this system never tells. Two separate `<button>`s side by
   side rather than one wrapping the other for the openable case, because a
   button inside a button is not markup this system draws anywhere else, and
   the two already do different things: one opens, one removes. Hover is
   tracked per chip in `hoveredAttachment` rather than through
   `useInteractive` — that composable is written for one control per component
   instance, and this row draws one name button per attachment.
   `aria-label="Open …"` carries a verb the visible, possibly-ellipsized name
   does not — without it the cross beside it, labelled "Remove …", would read
   as the only action a screen reader can hear on the chip. Not the native
   `title`: `IconButton.vue` rules that out for this system already.

   **This field is never drawn while the agent is holding an open question or
   an open permission request.** `ConversationView.vue` hides the whole
   composer for that stretch instead of locking it in place — there is
   nothing for a person to type until the card above is answered, and a
   locked field sitting under it was a control that took up room and refused
   everything typed into it. This component takes no prop for that state and
   has no business knowing which of the two cards it was. */
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
  /* The subset of `attachments` worth drawing as a control — see the header
     above for why this component cannot work that out for itself. */
  openAttachments: { type: Array, default: () => [] },
  /* A turn is in flight: the one button is Stop. */
  busy: { type: Boolean, default: false },
  canStop: { type: Boolean, default: true }
})

const emit = defineEmits([
  'update:modelValue',
  'update:attachments',
  'send',
  'stop',
  /* The bare path, raised only for an attachment `openAttachments` names;
     whoever draws this decides whether it is a picture or a file — see the
     header above. */
  'open-attachment',
  /* The paperclip button, pressed. No argument: opening the system dialog and
     deciding what its answer means is not this component's to do — see the
     header above. */
  'attach'
])

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
  if (props.busy || nothingToSend.value) return
  emit('send')
}

const remove = (path) =>
  emit(
    'update:attachments',
    props.attachments.filter((other) => other !== path)
  )

/* Which chip's name is under the pointer right now — see the header above
   for why this is a plain ref keyed by path rather than `useInteractive`. */
const hoveredAttachment = ref(null)

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
   name is an identifier, so mono.

   The left padding is `--space-2`, matching the right rather than the
   `--space-3` it used to be on its own — reduced by about what the name
   button below now spends on its own left inset, so the hover surface
   `chipNameStyle` draws does not read as the chip quietly growing wider. */
const chip = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  maxWidth: '100%',
  minWidth: 0,
  height: 'var(--control-h-sm)',
  padding: '0 var(--space-2)',
  background: 'var(--surface-raised)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)',
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)'
}

/* The name, as its own control (smetana-4x3w) when there is somewhere for a
   click to go: a click opens the attachment, the way `UserMessage.vue`'s own
   chip does once it is sent. `cursor: 'default'` matches
   `Button.vue`/`IconButton.vue` — every real button in this system reads as
   one by the surface stepping up under a pointer, not by the cursor changing
   shape — and the hover step is `--surface-hover` on a flat
   `background:transparent` otherwise, never a colour change
   (`core/interactive.js`'s own rule), eased by the same
   `--transition-control` the message half's `:hover` rule uses, rather than
   snapping. Reset to a plain span's box model since the browser's own button
   chrome has no place here: no border, text inheriting the chip's own colour
   and font rather than a button's defaults. `--space-1` on either side is a
   small inset so the hover surface reads as a control's own box rather than
   a highlighted word flush against the text — `chip`'s own left padding above
   is reduced to keep the row from visibly growing wider for it. */
function chipNameStyle(path) {
  return {
    minWidth: 0,
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
    padding: '0 var(--space-1)',
    border: 0,
    borderRadius: 'var(--radius-2)',
    background: hoveredAttachment.value === path ? 'var(--surface-hover)' : 'transparent',
    color: 'inherit',
    font: 'inherit',
    textAlign: 'left',
    cursor: 'default',
    transition: 'var(--transition-control)'
  }
}

/* The same box, for an attachment `openAttachments` does not name — a
   non-picture outside the open project, with no channel this app can open it
   through (see the header above). Same layout as `chipNameStyle`'s own —
   the same padding and the same ellipsis rule, so a chip does not visibly
   resize depending on whether it happens to be openable — and nothing else:
   no `cursor`, no `background` and no `transition`, because there is no hover
   state to ease into and no pointer affordance to draw. No `cursor` key at
   all, rather than `'auto'` written out, lets the browser's own default
   stand — which is what a run of plain inline text gets when nothing has
   asked for anything else. */
const chipNameStatic = {
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  padding: '0 var(--space-1)'
}

/* Smaller than the `sm` box, the way `Tab.vue` sizes its own close: an ordinary
   icon button is as tall as the chip it would sit inside. */
const chipClose = { width: 'var(--icon-sm)', height: 'var(--icon-sm)', flex: '0 0 auto' }

/* The frame is the row's and not the field's, and that is what makes the
   measurement above exact: `scrollHeight` counts padding and leaves out
   borders, so a border on the element being measured would come back one
   border-width short every keystroke. It also puts the button inside the same
   box as the words, which is what says the two are one control surface. */
const row = computed(() => ({
  display: 'flex',
  alignItems: 'flex-end',
  gap: 'var(--space-3)',
  padding: 'var(--space-3)',
  background: 'var(--surface-raised)',
  border: `var(--border-w) solid ${focus.value ? 'var(--focus-ring)' : 'var(--border)'}`,
  borderRadius: 'var(--radius-3)',
  boxShadow: focus.value ? 'inset 0 0 0 1px var(--focus-ring)' : 'none',
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
  color: 'var(--text-primary)',
  border: 0,
  outline: 'none',
  /* Prose, so sans — a person's message to an agent is a sentence and not an
     identifier, whatever paths it happens to hold. */
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  overflowY: 'auto',
  cursor: 'text',
  /* The grip is a browser-drawn control with no place in this system, and the
     field sizes itself anyway. */
  resize: 'none'
}))
</script>

<template>
  <div :style="root">
    <div v-if="attachments.length" :style="strip">
      <span v-for="path in attachments" :key="path" :style="chip">
        <Icon name="paperclip" :size="11" />
        <button
          v-if="openAttachments.includes(path)"
          type="button"
          :aria-label="`Open ${basename(path)}`"
          :style="chipNameStyle(path)"
          @mouseenter="hoveredAttachment = path"
          @mouseleave="hoveredAttachment = null"
          @click="emit('open-attachment', path)"
        >{{ basename(path) }}</button>
        <span v-else :style="chipNameStatic">{{ basename(path) }}</span>
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
      <!-- Pinned to the row's own bottom edge with Send/Stop, never disabled
           by `busy` — see the header above. -->
      <IconButton
        icon="paperclip"
        label="Attach files"
        size="sm"
        variant="ghost"
        @click="emit('attach')"
      />
      <textarea
        ref="field"
        rows="1"
        :value="modelValue"
        placeholder="Message the agent"
        :style="fieldStyle"
        @input="emit('update:modelValue', $event.target.value)"
        @keydown="onKeydown"
        @focus="focus = true"
        @blur="focus = false"
      />
      <!-- One control in two states, and never two controls: see the header. -->
      <Button v-if="busy && canStop" size="sm" icon="square" @click="emit('stop')">Stop</Button>
      <Button
        v-else
        size="sm"
        variant="primary"
        icon="arrow-up"
        :disabled="nothingToSend"
        @click="emit('send')"
      >Send</Button>
    </div>
  </div>
</template>
