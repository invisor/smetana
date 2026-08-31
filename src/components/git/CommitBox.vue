<script setup>
/* The message, and the two buttons under it: commit what the section below is
   listing, or ask the agent to write the sentence.

   Presentational like everything else here — it is handed the draft and emits
   what was pressed, so it draws in `?view=gallery` with no git and no agent
   behind it. When it may be pressed and what it says when it may not is
   `commitBox.js`, pure and tested, of the `gitActions.js` family.

   **The scope of the button is the whole list, and the label says so.** This
   app has no staging of its own, so `vcs_commit` runs `git add --all` before it
   commits and what a press takes is exactly what the rows below are showing,
   untracked files included. A button reading only "Commit" would leave the one
   surprising thing about it unsaid.

   The two buttons are gated by different questions on purpose. Committing is a
   write and takes `gitActions.js`'s verdict, the same one the branch rows take
   — a batch mid-merge must not get a commit under it. Asking for a message is a
   **read**: it runs `git diff` and a model, touches nothing, and stays live
   under a run for the same reason a folder heading in `BranchList` stays
   undimmed. Its failure is a quiet line under the field rather than the panel's
   red block, which says "Git refused this operation" and would be naming a
   party that was never asked.

   Cmd+Enter commits from the field, Ctrl+Enter beside it for the platforms
   without the first. A plain Enter deliberately does not: this is a `Textarea`,
   and a message with a second line in it is an ordinary thing to write.

   **The field is dragged taller by the separator under it, and not by the
   corner grip a browser draws.** `Textarea` turns that grip off on purpose —
   it is a control this design system never drew, and it can be dragged out of
   whatever the field sits in — so the height is this app's own `Resizer`,
   the one the side panels and the sections of this very panel are dragged by,
   and the count it moves is the `rows` the field already measured itself in. */
import { computed, ref } from 'vue'
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Resizer from '../shell/Resizer.vue'
import Textarea from '../core/Textarea.vue'
import Tooltip from '../core/Tooltip.vue'
import {
  DEFAULT_ROWS,
  canCommit,
  canSuggest,
  clampRows,
  commitHint,
  commitLabel,
  messagePlaceholder,
  resolveDragRows
} from './commitBox.js'

const props = defineProps({
  /* The draft. Held by the store per repository rather than here, so switching
     repositories and coming back finds the sentence where it was left. */
  modelValue: { type: String, default: '' },
  /* How many files a press would commit — the count the label carries. */
  changes: { type: Number, default: 0 },
  /* Which branch the commit would land on, for the field's own sentence. Null
     on a detached HEAD, which is a tree somebody can still commit to. */
  branch: { type: String, default: null },
  /* `{ allowed, reason }` from `gitActions.js`, exactly as `BranchList` takes
     it: whether this project may be written to, and the sentence to show when
     it may not. */
  actions: { type: Object, default: () => ({ allowed: true, reason: null }) },
  /* What git is doing right now, or null. Any operation at all holds this
     button: the three branch writes rewrite the very tree a commit would take. */
  busy: { type: Object, default: null },
  /* Whether the agent is being asked right now, and its refusal if it had one —
     `{ kind, message }`, drawn as it stands. */
  suggesting: { type: Boolean, default: false },
  suggestError: { type: Object, default: null },
  /* How tall the field is, in its own rows. Held outside this component for the
     reason the draft is: it is a person's preference and belongs in
     `settings.json`, beside the two section heights of this same panel, which
     are counts of rows for the same reason. A caller that passes nothing gets
     the two rows this field was fixed at before it could be dragged. */
  rows: { type: Number, default: DEFAULT_ROWS },
  /* How many paths git left unmerged in this repository, and whether it is
     part-way through a merge or a rebase at all — zero means either. The panel
     works both out; this component only draws the button. */
  conflicts: { type: Number, default: 0 }
})

const emit = defineEmits(['update:modelValue', 'commit', 'suggest', 'resize', 'resolveConflicts'])

const working = computed(() => Boolean(props.busy))

const ready = computed(() =>
  canCommit({
    message: props.modelValue,
    changes: props.changes,
    allowed: props.actions?.allowed,
    busy: working.value
  })
)
const askable = computed(() =>
  canSuggest({ changes: props.changes, suggesting: props.suggesting })
)
const hint = computed(() =>
  commitHint({
    message: props.modelValue,
    changes: props.changes,
    allowed: props.actions?.allowed,
    reason: props.actions?.reason,
    busy: working.value
  })
)
const label = computed(() => commitLabel(props.changes))

/* Which key the field names. Read once, off the platform, because it is a fact
   about the machine rather than about anything reactive — and `navigator` is
   the only thing that can answer it in a webview. Everything that is not a Mac
   is `Ctrl`, which is what the keydown handler below has always accepted. */
const mac = /mac/i.test(
  (typeof navigator === 'undefined' ? '' : navigator.platform || navigator.userAgent) ?? ''
)
const placeholder = computed(() => messagePlaceholder({ branch: props.branch, mac }))

const submit = () => {
  if (ready.value) emit('commit')
}

/* A stored count is a hint and never the truth, the rule `columnOrder.js`
   states: what is drawn is clamped here whatever the file said. */
const drawnRows = computed(() => clampRows(props.rows))

/* The drag, and the one pixel measurement in all of this.
 *
 * `rows` is a count and a pointer moves in pixels, so the two are bridged by
 * the field's own line height — read off the element rather than computed from
 * a token, because `--text-sm` and `--leading-normal` are a font size and a
 * unitless factor and their product is the browser's to work out. Read at
 * `dragstart` rather than held: the density and the app-wide font size both
 * move it, and neither re-renders this component.
 *
 * A line height that cannot be read is a drag that does nothing, which is the
 * right failure — a fallback number would move the field by the wrong amount
 * and look like the drag not tracking the pointer. */
const field = ref(null)
let drag = null

const onDragStart = () => {
  const el = field.value?.el
  const line = el ? Number.parseFloat(getComputedStyle(el).lineHeight) : Number.NaN
  drag = Number.isFinite(line) && line > 0 ? { base: drawnRows.value, line } : null
}

const onDrag = (delta) => {
  if (!drag) return
  emit('resize', resolveDragRows({ base: drag.base, delta: delta / drag.line }))
}

const onDragEnd = () => {
  drag = null
}

/* **Stuck to the top of the list rather than pinned above it**, and the
   difference is what a short panel does. Pinned — the box outside the scroller,
   the list inside its own — the section becomes two boxes in a column, and a
   panel too short for both draws the box clipped: the field is there, the
   button is under the section boundary, and nothing scrolls to it. Stuck, there
   is one scroller: the box holds the top while the rows go under it, and when
   the room runs out it scrolls into view like everything else.
 *
 * The background is not decoration either — a `sticky` element with none lets
 * the rows travel through the text. `--surface` is the panel's own, so the box
 * reads as part of it rather than as a card laid on top. */
const rootStyle = {
  position: 'sticky',
  top: 0,
  zIndex: 1,
  background: 'var(--surface)',
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  padding: 'var(--space-3) var(--space-5) var(--space-4)'
}

/* The field and the button it carries. The sparkle sits **inside** the field
   rather than on a row of its own, which is what leaves the commit button the
   whole width — and the whole width is what a person aims at without looking.
   The two are not the same kind of thing anyway: one is about the sentence in
   the box it sits in, the other is about the tree. */
const fieldStyle = { position: 'relative', display: 'block' }
/* The room the button needs at the end of the field, so a long message runs
   under it rather than into it. One box plus the space either side of it. */
const fieldPadding = { paddingRight: 'calc(var(--control-h-sm) + var(--space-4))' }
const markStyle = {
  position: 'absolute',
  top: 'var(--space-2)',
  right: 'var(--space-2)',
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: 'var(--control-h-sm)',
  height: 'var(--control-h-sm)'
}
const spinStyle = { color: 'var(--attn-live)', animation: 'sm-spin var(--dur-pulse) linear infinite' }

/* The agent's refusal, in its own words. Mono because what it carries is
   machine output — a binary that is not there, a harness that exited — and the
   same idiom the panel's git failures are drawn in. */
const errorStyle = {
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-mono)',
  color: 'var(--text-secondary)',
  whiteSpace: 'pre-wrap',
  overflowWrap: 'anywhere'
}
</script>

<template>
  <div :style="rootStyle">
    <!-- A plain box and deliberately not a `<label>`: a label wrapping the
         field would forward a click on the sparkle inside it to the textarea,
         which is a press that focuses somewhere else on its way to doing
         nothing. -->
    <div :style="fieldStyle">
      <Textarea
        ref="field"
        :model-value="modelValue"
        :rows="drawnRows"
        :placeholder="placeholder"
        :style="fieldPadding"
        @update:model-value="emit('update:modelValue', $event)"
        @keydown.meta.enter.prevent="submit"
        @keydown.ctrl.enter.prevent="submit"
      />
      <!-- The spinner stands in the button's own box rather than beside it, so
           nothing moves when the question goes out — the same trick
           `BranchList` plays with its two row buttons and `ChangeList` with its
           staged tick. -->
      <span :style="markStyle">
        <Icon
          v-if="suggesting"
          name="loader-circle"
          :size="13"
          :style="spinStyle"
          title="Writing a message"
        />
        <IconButton
          v-else
          icon="sparkles"
          label="Write the message with the agent"
          size="sm"
          :disabled="!askable"
          @click="emit('suggest')"
        />
      </span>
    </div>
    <!-- Under the field rather than over it, so downwards grows what it is
         about — and between the field and the button rather than at the foot of
         the box, since the button is not what it resizes. Double click gives
         the field its shipped two rows back, the way a separator does
         everywhere else in this app. -->
    <Resizer
      orientation="horizontal"
      label="Resize the message field"
      @dragstart="onDragStart"
      @drag="onDrag"
      @dragend="onDragEnd"
      @reset="emit('resize', DEFAULT_ROWS)"
    />
    <!-- Above the commit button, because a conflicted tree is what has to be
         answered before committing is worth thinking about — and git refuses a
         commit with unmerged paths anyway, so the button under this one is one
         git would decline.

         Secondary and not primary: the commit is what this box is for, and two
         primary buttons stacked in one column is a choice nobody made. It is
         gated by nothing — neither `gitActions.js` nor `busy` — because
         pressing it opens a dialog and writes nothing, and that dialog's own
         two buttons are already held while git works. -->
    <Button
      v-if="conflicts"
      variant="secondary"
      icon="git-merge"
      full-width
      @click="emit('resolveConflicts')"
    >
      Resolve conflicts
    </Button>
    <!-- The hint hangs on a wrapper rather than on the button: a disabled
         button takes no pointer events of its own, so a tooltip inside it would
         have nothing to open on in exactly the state that needs explaining. And
         the wrapper is a `Tooltip` only where there is something to explain, the
         same swap `BranchList` makes over its rows — an empty panel opening
         under the pointer is worse than none. -->
    <component
      :is="hint ? Tooltip : 'span'"
      v-bind="hint ? { label: hint } : {}"
      :style="{ display: 'block' }"
    >
      <Button variant="primary" icon="check" full-width :disabled="!ready" @click="submit">
        {{ label }}
      </Button>
    </component>
    <div v-if="suggestError" :style="errorStyle">{{ suggestError.message }}</div>
  </div>
</template>
