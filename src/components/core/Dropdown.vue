<script setup>
/* A value picked from a list, drawn by us rather than by the platform.

   **This is the product's list control, and `Select` is now drawn nowhere but
   the gallery.** `Select` was kept for a short list in an ordinary form — one
   element, accessible for free, costing nothing — and the settings window was
   the last place taking that bargain. What it actually bought there was a menu
   the operating system paints: its own colours, its own font, its own row
   height, none of them reachable by a token, none of them following the theme,
   the density or the app-wide font size that the very window it opened over
   exists to change. A theme switcher whose own list ignores the theme is the
   plainest version of that. The component stays in the library and in the
   gallery — it is not broken — but nothing in the app reaches for it.

   Beyond looking like the rest of the app, this does two things a native select
   cannot: put anything of our own in the panel, and filter a long list.
   `BranchSelect` needs both.

   The panel is teleported out and positioned in window coordinates, for the
   reason `Tooltip` was: an ancestor with `overflow` clips an absolutely
   positioned descendant however it is positioned, and this is used inside
   `Panel`'s scroll container as well as inside a dialog that has none. It flips
   above the field when there is no room below, and it follows the field while
   anything scrolls. */
import { computed, nextTick, ref, watch } from 'vue'
import Icon from './Icon.vue'

const props = defineProps({
  modelValue: { type: [String, Number], default: '' },
  /* Accepts ['a','b'] or [{ value, label }], exactly as `Select` does — the two
     are interchangeable at the call site on purpose. */
  options: { type: Array, default: () => [] },
  disabled: { type: Boolean, default: false },
  /* A filter above the list. Off by default: it is worth the extra field only
     where the list is long enough that scanning it is work. */
  searchable: { type: Boolean, default: false },
  searchLabel: { type: String, default: 'Search' },
  /* Identifiers are drawn in mono and prose in sans — a branch name is one, a
     mode is the other. */
  mono: { type: Boolean, default: false },
  placeholder: { type: String, default: 'Select' },
  /* The same two `Select` has, and the same names: a dense panel wants the
     short one. */
  size: { type: String, default: 'md' },
  /* Something to say about the current value, muted, before the chevron. */
  hint: { type: String, default: '' }
})

const emit = defineEmits(['update:modelValue', 'open'])

const open = ref(false)
const query = ref('')
const cursor = ref(0)
const root = ref(null)
const field = ref(null)
const panel = ref(null)
const list = ref(null)
const filterField = ref(null)
/* Where the panel goes, in window coordinates. Null until measured — the one
   state it must not be seen in, since it would be sitting in the corner. */
const at = ref(null)
/* The field's width, taken before the panel is rendered rather than with the
   rest of the placement. It has to be right on the very first layout: a panel
   measured at zero width wraps every label and comes out far taller than it
   will end up, and the flip below would then be decided from that height. */
const width = ref(0)

/* The distance from the field, and the closest the panel may come to the
   window's edge. Operands in arithmetic against getBoundingClientRect rather
   than values handed to the browser, which is why they are numbers here and not
   token references — the same note `Tooltip` carries. */
const GAP = 4
const EDGE = 8

const items = computed(() =>
  props.options.map((o) => (typeof o === 'object' && o !== null ? o : { value: o, label: String(o) }))
)

/* Filtering hides options, and a caption whose whole group went with them is a
   heading over nothing. A header survives only when something choosable follows
   it before the next one does. */
const matches = computed(() => {
  const needle = query.value.trim().toLowerCase()
  const kept = needle
    ? items.value.filter((o) => o.header || String(o.label).toLowerCase().includes(needle))
    : items.value
  return kept.filter((o, i) => !o.header || (!!kept[i + 1] && !kept[i + 1].header))
})

/* A value that is not in the list is still shown, as itself. The list is what
   can be picked, not what can be held: a branch just named here is not in it
   yet, and a value kept in settings may have gone since. Blanking the field to
   the placeholder in either case would say "nothing is chosen" about something
   that is about to be acted on. */
const selectedLabel = computed(() => {
  const found = items.value.find((o) => !o.header && o.value === props.modelValue)
  if (found) return String(found.label)
  return props.modelValue === '' || props.modelValue == null ? '' : String(props.modelValue)
})

/* Never a header: the cursor is where Enter lands, and Enter on a caption would
   do nothing while looking exactly like a row that refuses to be picked. */
watch(matches, () => {
  cursor.value = Math.max(0, matches.value.findIndex((o) => !o.header))
})

const show = async () => {
  if (props.disabled) return
  open.value = true
  query.value = ''
  const chosen = items.value.findIndex((o) => !o.header && o.value === props.modelValue)
  cursor.value = chosen >= 0 ? chosen : Math.max(0, items.value.findIndex((o) => !o.header))
  at.value = null
  width.value = field.value?.getBoundingClientRect().width ?? 0
  emit('open')
  await nextTick()
  place()
  reveal()
  /* Whichever of the two can take the keyboard. Without focus landing
     somewhere inside the panel the arrow keys would go to the page, and a list
     that cannot be walked by keyboard is a list a native select did better. */
  ;(props.searchable ? filterField.value : panel.value)?.focus()
}

/* Measured once on opening and again whenever anything moves under it. Below
   the field when it fits, above when it does not, and clamped to the window
   either way — the last of those is what stands between a field near the bottom
   of a small window and a list drawn off the edge. */
function place() {
  const anchor = field.value?.getBoundingClientRect()
  const box = panel.value?.getBoundingClientRect()
  if (!anchor || !box) return
  const room = window.innerHeight - anchor.bottom - GAP - EDGE
  const above = box.height > room && anchor.top - GAP - EDGE > room
  const top = above ? anchor.top - GAP - box.height : anchor.bottom + GAP
  width.value = anchor.width
  at.value = {
    left: anchor.left,
    top: Math.max(EDGE, Math.min(top, window.innerHeight - box.height - EDGE))
  }
}

/* Brings the cursor's row inside the eight the list shows.

   Wanted in two moments: opening on a value that sits below the eighth row —
   the settings window's font sizes are fifteen and the shipped one is fourth,
   so anything above about 17px opened on a list with no visible answer in it —
   and walking past either end with the arrow keys. Measured in window
   coordinates rather than through `offsetTop`, which would be relative to
   whichever ancestor happens to be positioned, and adjusted by hand rather
   than with `scrollIntoView`: that one is free to scroll every scrollable
   ancestor, and this panel hangs off the body while the field it belongs to
   may be deep inside a scrolling one. */
function reveal() {
  const box = list.value
  const row = box?.children[cursor.value]
  if (!row) return
  const rowAt = row.getBoundingClientRect()
  const listAt = box.getBoundingClientRect()
  if (rowAt.top < listAt.top) box.scrollTop -= listAt.top - rowAt.top
  else if (rowAt.bottom > listAt.bottom) box.scrollTop += rowAt.bottom - listAt.bottom
}

watch(cursor, () => {
  if (open.value) reveal()
})

const hide = () => {
  open.value = false
  query.value = ''
}

const choose = (value) => {
  emit('update:modelValue', value)
  hide()
}

/* One step in a list that has rows the keyboard must pass over. Bounded by the
   list's own length, so a list of nothing but captions stops rather than
   spinning. */
const step = (delta) => {
  const n = matches.value.length
  if (!n) return
  let i = cursor.value
  for (let taken = 0; taken < n; taken += 1) {
    i = (i + delta + n) % n
    if (!matches.value[i].header) {
      cursor.value = i
      return
    }
  }
}

const onKeydown = (event) => {
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    step(event.key === 'ArrowDown' ? 1 : -1)
  } else if (event.key === 'Enter') {
    event.preventDefault()
    const pick = matches.value[cursor.value]
    if (pick && !pick.header) choose(pick.value)
  } else if (event.key === 'Escape') {
    event.preventDefault()
    hide()
  }
}

/* Pointerdown rather than click: a press that starts inside the panel and ends
   outside it — a drag across the list — would otherwise close the panel out
   from under the pointer.

   Both boxes are checked, and the panel one is not optional: it is teleported
   to the body and is therefore not inside `root` at all. Without it a press on
   an option would close the panel on pointerdown and the click would land on a
   node that had already been removed — the list would look completely dead. */
const onDocumentPointerdown = (event) => {
  const inside =
    root.value?.contains(event.target) || panel.value?.contains(event.target)
  if (!inside) hide()
}

watch(open, (isOpen) => {
  const method = isOpen ? 'addEventListener' : 'removeEventListener'
  document[method]('pointerdown', onDocumentPointerdown, true)
  // Capture, so a scroll inside any ancestor is seen, not only the window's.
  window[method]('scroll', place, true)
  window[method]('resize', place)
})

/* The list changes height as it is filtered, and a panel opened upwards has to
   move with it or it drifts away from its field. */
watch(matches, () => {
  if (open.value) nextTick(place)
})

defineExpose({ close: hide })

/* The field is deliberately the same silhouette as `Select`'s: same height,
   same padding, same border and the same chevron, so a dialog mixing the two
   does not look like a dialog mixing two libraries. */
const fieldStyle = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  width: '100%',
  height: props.size === 'sm' ? 'var(--control-h-sm)' : 'var(--control-h)',
  padding: '0 var(--space-4)',
  background: props.disabled ? 'var(--surface-sunken)' : 'var(--surface-raised)',
  color: props.disabled ? 'var(--text-muted)' : 'var(--text-primary)',
  border: `var(--border-w) solid ${open.value ? 'var(--focus-ring)' : 'var(--border)'}`,
  borderRadius: 'var(--radius-3)',
  font: `var(--weight-regular) var(--text-sm)/1 ${props.mono ? 'var(--font-mono)' : 'var(--font-sans)'}`,
  cursor: props.disabled ? 'not-allowed' : 'default',
  textAlign: 'left'
}))

const valueStyle = computed(() => ({
  flex: 1,
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  color: selectedLabel.value === '' ? 'var(--text-muted)' : 'inherit'
}))

const panelStyle = computed(() => ({
  position: 'fixed',
  top: `${at.value?.top ?? 0}px`,
  left: `${at.value?.left ?? 0}px`,
  width: `${width.value}px`,
  // Hidden until measured: one frame of it in the window's corner reads as a
  // flash, and it has to be in the document at its natural height to be
  // measured at all.
  visibility: at.value ? 'visible' : 'hidden',
  zIndex: 'var(--z-popover)',
  display: 'flex',
  flexDirection: 'column',
  background: 'var(--surface-overlay)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-3)',
  boxShadow: 'var(--shadow-overlay)',
  overflow: 'hidden',
  outline: 'none'
}))

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

/* Eight rows and then it scrolls: enough to hold a working set without the
   panel running past the bottom of whatever it opened in. */
const listStyle = {
  maxHeight: 'calc(8 * var(--row-h))',
  overflowY: 'auto',
  display: 'flex',
  flexDirection: 'column'
}

const optionStyle = (option, index) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  /* The list above is a column flex container, and a flex item shrinks by
     default — so past the eighth row `height` stopped being a height and became
     a starting point, and fifteen rows shared the ceiling between them at 15px
     each instead of scrolling at 28. The list never overflowed, so nothing
     scrolled and nothing looked broken; it just quietly drew a denser list the
     longer it got. The font-size lists in the settings window are fifteen rows
     and are where it finally showed. */
  flexShrink: 0,
  padding: '0 var(--space-4)',
  border: 'none',
  width: '100%',
  textAlign: 'left',
  /* Where the keyboard is and what is chosen are different facts and are drawn
     differently: a surface step for the first, a check for the second. Colour
     is never the only signal here either. */
  background: index === cursor.value ? 'var(--surface-hover)' : 'transparent',
  color: option.value === props.modelValue ? 'var(--text-primary)' : 'var(--text-secondary)',
  font: `var(--weight-regular) var(--text-sm)/1 ${props.mono ? 'var(--font-mono)' : 'var(--font-sans)'}`,
  cursor: 'default'
})

/* The label, and which of the two spans in a row gives way.

   Flex shrink factors cannot say "this one only after that one has nothing
   left": a factor distributes the shortfall in proportion, so the label always
   keeps some share of it, and `text-overflow: ellipsis` fires on a fraction of
   a pixel by removing whole characters — 0.05px of overflow cost a branch name
   two of them. Weighting the note at 100 made the share 0.28px and clipped
   `spike/auth` to `spike/au…`; raising the number only moves which labels round
   the wrong way. So the ordering is stated rather than approximated: no shrink
   while a note is there to take it, ordinary shrink when the label is alone.

   `1 1 auto` and not `flex: 1`: that is `1 1 0%`, a flex-basis of zero, which
   leaves the label with no base size to defend — it absorbs all free space, the
   row never overflows, and no clipping rule on the note can ever fire. It still
   grows into free space, so a row with no note looks exactly as it always did.

   `maxWidth` is the backstop for the pathological row, where the name alone is
   wider than the panel: the note is squeezed away first, and then the label
   ellipsises at the row's own width instead of spilling past it. */
const labelStyle = (option) => ({
  flex: '1 1 auto',
  flexShrink: option.note ? 0 : 1,
  maxWidth: '100%',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
})

const emptyStyle = {
  padding: 'var(--space-4)',
  fontSize: 'var(--text-xs)',
  color: 'var(--text-muted)',
  fontFamily: 'var(--font-sans)'
}

const hintStyle = {
  fontSize: 'var(--text-2xs)',
  color: 'var(--text-muted)',
  whiteSpace: 'nowrap',
  fontFamily: 'var(--font-sans)'
}

/* A row's own note. The field's `hintStyle` colour and size, deliberately not
   `hintStyle` itself: this one has to lose a negotiation the field's hint never
   enters. It is the only span in the row that shrinks while both are present —
   see `labelStyle` for why that ordering is stated outright rather than
   weighted — so the whole of the shortfall lands here and ellipsises here. */
const noteStyle = {
  fontSize: 'var(--text-2xs)',
  color: 'var(--text-muted)',
  fontFamily: 'var(--font-sans)',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}

/* A caption over a group. Muted and small, so it reads as structure rather than
   as an option somebody failed to make choosable, and it keeps `--row-h` so the
   list's rhythm does not break where a group starts. The rule belongs to the
   boundary, so the first caption in a list has none. */
const headerStyle = (index) => ({
  display: 'flex',
  alignItems: 'center',
  height: 'var(--row-h)',
  flexShrink: 0,
  padding: '0 var(--space-4)',
  color: 'var(--text-muted)',
  font: `var(--weight-medium) var(--text-2xs)/1 var(--font-sans)`,
  borderTop: index === 0 ? 'none' : 'var(--border-w) solid var(--border-subtle)'
})
</script>

<template>
  <div ref="root" :style="{ position: 'relative', width: '100%' }">
    <button
      ref="field"
      type="button"
      :disabled="disabled"
      :aria-expanded="open"
      :style="fieldStyle"
      @click="show"
    >
      <span :style="valueStyle">{{ selectedLabel || placeholder }}</span>
      <span v-if="hint" :style="hintStyle">{{ hint }}</span>
      <Icon name="chevron-down" :size="14" :style="{ color: 'var(--text-muted)' }" />
    </button>

    <Teleport to="body">
      <div v-if="open" ref="panel" tabindex="-1" :style="panelStyle" @keydown="onKeydown">
      <!-- Anything of our own that belongs above the list. This is the whole
           reason this component exists rather than a `Select`. -->
      <slot name="header" :close="hide" />
      <input
        v-if="searchable"
        ref="filterField"
        v-model="query"
        :style="filterStyle"
        :placeholder="searchLabel"
        :aria-label="searchLabel"
        @keydown="onKeydown"
      />
      <div ref="list" :style="listStyle">
        <template v-for="(option, i) in matches" :key="option.header ? `h:${option.label}` : option.value">
          <!-- A caption is a `div` and not a disabled `button`: it is not a
               choice that happens to be unavailable, and a disabled control in
               the tab order is a stop for no reason. It stays a child of this
               same list, which is what keeps `reveal`'s `children[cursor]`
               pointing at the row the cursor names. -->
          <div v-if="option.header" :style="headerStyle(i)">{{ option.label }}</div>
          <button
            v-else
            type="button"
            :style="optionStyle(option, i)"
            @mouseenter="cursor = i"
            @click="choose(option.value)"
          >
            <Icon
              name="check"
              :size="12"
              :style="{ visibility: option.value === modelValue ? 'visible' : 'hidden' }"
            />
            <span :style="labelStyle(option)">
              {{ option.label }}
            </span>
            <!-- Something to say about this row in particular: the field's
                 `hintStyle` voice, in its own style rather than that one.
                 Reusing it would carry `white-space: nowrap` with nothing to
                 shrink or clip it, and in a flex row that means the *label*
                 gives way — a long note eating the branch name it is about,
                 which is the opposite of what a note is for. So it keeps the
                 colour and size and adds what a row needs: it may shrink, and
                 it clips itself when it does. -->
            <span v-if="option.note" :style="noteStyle">{{ option.note }}</span>
          </button>
        </template>
        <div v-if="!matches.length" :style="emptyStyle">
          <slot name="empty">Nothing matches.</slot>
        </div>
      </div>
      </div>
    </Teleport>
  </div>
</template>
