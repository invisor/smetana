<script setup>
/* Everything the tracker knows about one issue.

   Nothing in this component is editable and nothing in it acts, and that is
   still the rule: rewriting a title or a description is an agent's job, and a
   panel that cannot be typed into cannot silently overwrite what an agent wrote
   while it sat open.

   What has changed is where the verbs are reached from. Running an issue,
   handing it to an agent, moving it between columns and deleting it live in one
   menu — `taskMenu.js` — and that menu now has two triggers: the card's own, so
   it reaches the card under the pointer rather than only the selected one, and
   a second one in the header of the panel this component is drawn inside.

   The second trigger is not this component's, deliberately. It sits in the
   `actions` slot of the surrounding `Panel` in `views/DesktopApp.vue`, where it
   stays put while this scrolls, and where the run state and the writes it needs
   already are — none of which this file knows or needs to learn. */
import { computed } from 'vue'
import Markdown from '../markdown/Markdown.vue'
import Tooltip from '../core/Tooltip.vue'
import StatusBadge from '../status/StatusBadge.vue'
import TypeBadge from './TypeBadge.vue'
import { copyLabel } from './copyId.js'
import { priorityLabel } from './issueType.js'
import { notesForDisplay } from './noteEntries.js'

const props = defineProps({
  /* The issue in bd's own shape, straight out of the tracker store. */
  issue: { type: Object, required: true },
  /* Statuses translated to the design system's vocabulary, for the badge. */
  uiStatus: { type: String, required: true },
  /* What happened to the last attempt to copy this issue's id: `''` before
     anything was asked, `'copied'` or `'failed'` after. It changes the text of
     one tooltip and nothing else. The copying is the view's — this panel
     imports no store and acts on nothing. */
  copyState: { type: String, default: '' },
  /* The active project's absolute path, or `''` where there is none — passed
     to `Markdown.vue` as `base`, the directory a relative illustration
     resolves from, and deliberately never as `root`: a task in the inspector
     has no session and no working tree, which is exactly the case
     `MarkdownInline.vue`'s own header already draws a local link inert for,
     and giving this component a `root` would turn every plain path in an
     issue's prose into a link nothing here was asked to add. A figure has no
     such alternative — an illustration with nowhere to resolve from is one
     that never draws — so it is handed the project root anyway, `Markdown.vue`'s
     own fallback for exactly this case. */
  base: { type: String, default: '' }
})

/* A link inside one of the prose fields, raised rather than opened: no
   component in `src/components/` knows Tauri exists, so the view binds
   `openExternal` — the app's one link-opening path — to this. `open-image` is
   forwarded the same way, to whatever can reach `views/window.rs`'s image
   window. */
const emit = defineEmits(['open', 'copy-id', 'open-image'])

/* What the id's tooltip says, from `copyId.js` — the same three words the
   card's id uses, in the one place a test can read them. */
const idLabel = computed(() => copyLabel(props.copyState))

/* bd hands dates over as RFC 3339 in UTC. The panel is narrow, so the year is
   worth the four characters only because an issue can be old — the alternative,
   a relative "3 days ago", turns into a lie the moment the app is left open
   overnight. Anything unparseable is shown as it arrived rather than as
   "Invalid Date": bd's own text is more use to whoever has to explain it. */
const FORMAT = new Intl.DateTimeFormat('en-GB', {
  day: '2-digit',
  month: 'short',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit'
})

function formatDate(value) {
  if (!value) return null
  const parsed = Date.parse(value)
  return Number.isNaN(parsed) ? value : FORMAT.format(parsed)
}

/* bd gives an issue only its outgoing edges, so "blocks" cannot be counted
   here — that sum needs every issue on the board and lives in the tracker
   store. What is countable from one issue is what blocks it. Parentage travels
   as a parent-child edge and is excluded: every subtask would otherwise read as
   blocked by its own parent. */
const blockedBy = computed(() =>
  (props.issue.dependencies ?? []).filter((d) => d.type === 'blocks').map((d) => d.depends_on_id)
)

/* `notes` is a journal, not a single markdown paragraph: bd joins every
   `bd note` call with one newline, and a single newline inside a markdown
   paragraph is a soft break — collapsed to a space, which glued the whole
   log into one run-on sentence (smetana-k2mo). `noteEntries.js` holds the
   rule for telling one record from the next; this just feeds `Markdown` the
   normalized string, a blank line between records, so each becomes its own
   paragraph with no change to `Markdown`, `MarkdownInline` or the markup
   contract. */
const notesText = computed(() => notesForDisplay(props.issue.notes))

/* Only the rows the issue actually has. A fixed list with blanks in it would
   read as a form waiting to be filled in, and this panel is not one. */
const rows = computed(() => {
  const issue = props.issue
  const entries = [
    /* Type is not here: it is a badge in the header, beside the status. */
    /* null when there is none, which the filter below drops: this panel shows
       only the rows an issue actually has. */
    ['Priority', priorityLabel(issue.priority), false],
    ['Owner', issue.owner, true],
    /* Beside the owner rather than instead of it: bd emits both and they are
       two different people (smetana-a5b). `owner` owns the issue; `assignee` is
       who holds it right now — an agent session's actor while a run has it
       claimed — and it was invisible here for as long as the field was missing
       from the struct. An identifier either way, so mono. */
    ['Assignee', issue.assignee, true],
    ['Labels', issue.labels?.length ? issue.labels.join(', ') : null, false],
    ['Parent', issue.parent, true],
    /* The ids, not how many: this panel has the room, and the number alone
       leaves a person with nowhere to go next. Mono, like every other
       identifier here. */
    ['Blocked by', blockedBy.value.join(', ') || null, true],
    ['Comments', issue.comment_count || null, false],
    ['Created', formatDate(issue.created_at), true],
    ['Created by', issue.created_by, true],
    ['Started', formatDate(issue.started_at), true],
    ['Updated', formatDate(issue.updated_at), true],
    ['Closed', formatDate(issue.closed_at), true]
  ]
  return entries
    .filter(([, value]) => value !== null && value !== undefined && value !== '')
    .map(([label, value, mono]) => ({ label, value: String(value), mono }))
})

const body = { display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }

const header = { display: 'flex', alignItems: 'center', gap: 'var(--space-4)' }

/* A button with no surface of its own: the id is drawn exactly as it was, and
   the pointer is the only thing that changes. There is one id in this header,
   so unlike the card's it is worth a tab stop — Enter and Space copy it. */
const idStyle = {
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  padding: 0,
  border: 0,
  background: 'transparent',
  cursor: 'pointer'
}

const titleStyle = {
  font: `var(--weight-medium) var(--text-md)/var(--leading-snug) var(--font-sans)`,
  color: 'var(--text-primary)',
  textWrap: 'pretty'
}

/* Two columns, the label column sized to its longest word and no wider. */
const grid = {
  display: 'grid',
  gridTemplateColumns: 'max-content minmax(0, 1fr)',
  columnGap: 'var(--space-5)',
  rowGap: 'var(--space-3)',
  alignItems: 'baseline'
}

const rowLabel = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}

const rowValue = (mono) => ({
  font: mono
    ? 'var(--weight-regular) var(--text-xs)/var(--leading-snug) var(--font-mono)'
    : 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-sans)',
  color: 'var(--text-primary)',
  overflowWrap: 'anywhere'
})

/* One of bd's prose fields under its heading: acceptance criteria, design,
   notes. Prose like the description, not a row in the table — a note is a
   paragraph and may be several, since every `bd note` appends another. */
const proseSection = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)'
}

/* `.sm-prose` (`sm-prose.css:33`) is the conversation panel's own container —
   a flex column with `gap:var(--prose-turn-gap)` and `padding:var(--panel-pad)`
   — not a bare wrapper for a fragment of prose. This panel has no turns to
   space apart and its own padding already comes from `inspectorBody` in
   `DesktopApp.vue`, so every field below turns the container half of the class
   off and keeps only what actually paints the elements inside it: `display:
   block` makes the flex `gap` inert (blocks fall back to `sm-prose.css`'s own
   `margin-bottom:var(--prose-block-gap)` rhythm, and `:first-child`/
   `:last-child` still trim both ends), and zeroing the padding leaves the
   panel's own inset the only one. Do not remove this thinking it is dead
   weight — without it every paragraph here carries the turn gap on top of its
   own block margin, roughly doubling the space between them, and the
   description sits inset twice under a title and an id row that are inset
   once. */
const flatProse = { display: 'block', padding: 'var(--space-0)' }

const closeReasonBox = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  padding: 'var(--space-4)',
  background: 'var(--surface-sunken)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)'
}

const divider = {
  height: 'var(--border-w)',
  background: 'var(--border-subtle)'
}
</script>

<template>
  <div :style="body">
    <div :style="header">
      <Tooltip :label="idLabel">
        <button type="button" :style="idStyle" @click="emit('copy-id', issue.id)">{{ issue.id }}</button>
      </Tooltip>
      <StatusBadge :status="uiStatus" size="sm" />
      <!-- The panel keeps the status badge the card gave up: nothing here says
           which column the issue is in. The type joins it rather than staying a
           row of text below, so the same badge means the same thing in both
           places. -->
      <TypeBadge v-if="issue.issue_type" :type="issue.issue_type" size="sm" />
    </div>

    <div :style="titleStyle">{{ issue.title }}</div>

    <!-- Everything bd stores is markdown, and so is everything an agent writes
         into it, so all five prose fields below are drawn as markdown rather
         than as the text of it — as `sm-prose.css` (markup-contract.md,
         section 2) now paints it, so each field's own `Markdown` sits under a
         `class="sm-prose"` root. `flatProse` above is the one `:style` on that
         root, turning off the panel-container half of the class this field has
         no use for. Still read-only: nothing here is editable, and a task
         item's box is drawn by the stylesheet rather than by a control. -->
    <div v-if="issue.description" class="sm-prose" :style="flatProse">
      <Markdown :text="issue.description" :base="base" @open="emit('open', $event)" @open-image="emit('open-image', $event)" />
    </div>

    <!-- bd's other prose, in a fixed order: the two that are the spec first,
         the log that grows last. Read-only like the description — rewriting
         any of them is an agent's job. Absent fields draw nothing at all, so
         an issue without them looks exactly as it did before they existed. -->
    <div v-if="issue.acceptance_criteria" :style="proseSection">
      <span :style="rowLabel">Acceptance criteria</span>
      <div class="sm-prose" :style="flatProse">
        <Markdown :text="issue.acceptance_criteria" :base="base" @open="emit('open', $event)" @open-image="emit('open-image', $event)" />
      </div>
    </div>

    <div v-if="issue.design" :style="proseSection">
      <span :style="rowLabel">Design</span>
      <div class="sm-prose" :style="flatProse">
        <Markdown :text="issue.design" :base="base" @open="emit('open', $event)" @open-image="emit('open-image', $event)" />
      </div>
    </div>

    <div v-if="issue.notes" :style="proseSection">
      <span :style="rowLabel">Notes</span>
      <div class="sm-prose" :style="flatProse">
        <Markdown :text="notesText" :base="base" @open="emit('open', $event)" @open-image="emit('open-image', $event)" />
      </div>
    </div>

    <!-- Only when there is a record to separate: an issue carrying neither
         fields nor a close reason would otherwise draw a rule under the
         description with nothing at all beneath it. -->
    <div v-if="rows.length || issue.close_reason" :style="divider" />

    <div v-if="rows.length" :style="grid">
      <template v-for="row in rows" :key="row.label">
        <span :style="rowLabel">{{ row.label }}</span>
        <span :style="rowValue(row.mono)">{{ row.value }}</span>
      </template>
    </div>

    <div v-if="issue.close_reason" :style="closeReasonBox">
      <span :style="rowLabel">Close reason</span>
      <div class="sm-prose" :style="flatProse">
        <Markdown :text="issue.close_reason" :base="base" @open="emit('open', $event)" @open-image="emit('open-image', $event)" />
      </div>
    </div>
  </div>
</template>
