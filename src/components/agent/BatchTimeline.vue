<script setup>
/* The Agent tab's pane for a Claude Code Autopilot batch: the run drawn as a
   timeline instead of xterm over its text transcript. The agent's remarks are
   sans prose that break the stream into chapters; under each, one compact mono
   row per tool call — time, tool, command, and a tag naming the worktree the
   call ran in. The call still open pulses. Design variant 1a of the agent log
   handoff; the rules for what a row says are `batchTimeline.js` and
   `batchScope.js`, and this file only draws them.

   **It reads the terminals store, as `TerminalView.vue` does.** It attaches to the session and listens on the
   store's log channel (`subscribeLog`); the byte channel carries the same
   session's text transcript, which this pane deliberately does not draw. The
   `entries` prop is the gallery's way in: given, the component draws exactly
   those and never attaches, because a browser has no worker to attach to.

   **Following the tail.** The view sticks to the bottom while the person is
   within one row of it and lets go the moment they scroll further up, so
   reading an earlier call while the batch pours rows in does not drag them
   back; scrolling to the bottom again resumes following. The same rule
   `LogView.vue` draws its `follow` from.

   **Hover** is held here as the key of the row under the pointer rather than
   through `useInteractive` per row: a row is not a component, and one ref is
   all a list of thousands needs. It is still a surface step and never a
   transform. */
import { computed, nextTick, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import EmptyState from '../core/EmptyState.vue'
import Icon from '../core/Icon.vue'
import Tooltip from '../core/Tooltip.vue'
import MarkdownInline from '../markdown/MarkdownInline.vue'
import { parseInline } from '../markdown/markdown.js'
import { openExternal } from '../../stores/app.js'
import { attach, detach, isStarting, subscribeLog } from '../../stores/terminals.js'
import { scopeOf } from './batchScope.js'
import { timelineRows } from './batchTimeline.js'

const props = defineProps({
  sessionId: { type: [String, Number], default: null },
  project: { type: String, default: '' },
  issueIds: { type: Array, default: () => [] },
  appDir: { type: String, default: '' },
  exited: { type: Boolean, default: false },
  entries: { type: Array, default: null }
})

/* The worker keeps this many (`LOG_BUDGET` in `terminal/service.rs`); a pane
   left open for a night must not grow past what a fresh attach would hand it. */
const LOG_CAP = 4000

const log = shallowRef([])
const expanded = ref(null)
const hovered = ref(null)
const scroller = ref(null)
let pinned = true
let attached = null

const source = computed(() => props.entries ?? log.value)

const rows = computed(() => {
  const scope = { project: props.project, issueIds: props.issueIds, appDir: props.appDir }
  return timelineRows(source.value, { project: props.project, exited: props.exited }).map((row) => {
    if (row.kind === 'say') return { ...row, nodes: parseInline(row.text) }
    // Only a shell command has a directory of its own; a Read or a Task names
    // no place to put a tag on.
    if (row.kind === 'tool' && row.tool === 'Bash') return { ...row, scope: scopeOf(row.cwd, scope) }
    return row
  })
})

function toBottom() {
  const el = scroller.value
  if (el) el.scrollTop = el.scrollHeight
}

function onScroll() {
  const el = scroller.value
  if (!el) return
  const row = el.lastElementChild?.offsetHeight ?? 0
  pinned = el.scrollHeight - el.scrollTop - el.clientHeight <= row
}

/* Keyed on the last entry's `seq` and not on the row count: the count stops
   moving once the log sits at its cap, and a result folding into the call above
   it adds no row, yet both are news at the bottom that should keep a docked
   pane docked. */
watch(
  () => source.value[source.value.length - 1]?.seq,
  async () => {
    if (!pinned) return
    await nextTick()
    toBottom()
  }
)

let unsubscribe = null
if (props.entries == null) {
  unsubscribe = subscribeLog((change) => {
    if (change.reset) {
      log.value = change.entries.slice(-LOG_CAP)
      expanded.value = null
      pinned = true
    } else {
      log.value = [...log.value, ...change.entries].slice(-LOG_CAP)
    }
  })
  watch(
    () => props.sessionId,
    (id) => {
      if (attached != null) detach(attached)
      attached = null
      log.value = []
      expanded.value = null
      pinned = true
      if (id == null || isStarting(id)) return
      attached = id
      attach(id)
    },
    { immediate: true }
  )
}

onBeforeUnmount(() => {
  unsubscribe?.()
  if (attached != null) detach(attached)
  attached = null
})

function toggle(key) {
  expanded.value = expanded.value === key ? null : key
}

/* The handoff's own numbers: a time column, a tool column, and the small rhythm
   of a dense mono list. They are the design's and not a scale this system has a
   token for. */
const TIME_COL = '76px'
const TOOL_COL = '64px'

const scrollStyle = {
  flex: 1,
  minWidth: 0,
  minHeight: 0,
  overflowY: 'auto',
  background: 'var(--canvas)',
  padding: 'var(--space-3) 0'
}

const toolGrid = {
  display: 'grid',
  gridTemplateColumns: `${TIME_COL} ${TOOL_COL} minmax(0, 1fr) auto`,
  columnGap: 'var(--space-3)',
  alignItems: 'baseline',
  padding: '3px var(--space-4)',
  fontFamily: 'var(--font-mono)',
  fontSize: 'var(--text-sm)',
  lineHeight: 1.5,
  cursor: 'pointer'
}

const timeStyle = {
  fontSize: 'var(--text-xs)',
  color: 'var(--text-muted)',
  opacity: 0.7,
  fontVariantNumeric: 'tabular-nums'
}

const commandStyle = {
  minWidth: 0,
  overflow: 'hidden',
  whiteSpace: 'nowrap',
  textOverflow: 'ellipsis'
}

const scopeCell = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-3)'
}

const dotStyle = {
  width: '6px',
  height: '6px',
  borderRadius: '50%',
  background: 'var(--attn-live)',
  flex: '0 0 auto',
  animation: 'sm-pulse var(--dur-pulse) var(--ease-in-out) infinite'
}

const tagStyle = (colors) => ({
  fontSize: 'var(--text-2xs)',
  padding: '1px 5px',
  borderRadius: 'var(--radius-2)',
  border: `var(--border-w) solid ${colors.border}`,
  background: colors.bg,
  color: colors.fg,
  whiteSpace: 'nowrap'
})

const wrapStyle = (key) => ({
  background: hovered.value === key ? 'var(--surface-hover)' : 'transparent',
  transition: 'background var(--dur-fast) var(--ease-out)'
})

const saySheet = {
  display: 'grid',
  gridTemplateColumns: `${TIME_COL} minmax(0, 1fr)`,
  columnGap: 'var(--space-3)',
  padding: '14px var(--space-4) 6px',
  marginTop: '6px',
  borderTop: 'var(--border-w) solid var(--border-subtle)'
}

const sayTime = {
  ...timeStyle,
  fontFamily: 'var(--font-mono)',
  paddingTop: '2px'
}

/* `.sm-prose` is what paints a `<code>` out of a backtick; the class's own
   panel-container half is switched off, the way `TaskInspector.vue` does it. */
const sayText = {
  display: 'block',
  padding: 'var(--space-0)',
  minWidth: 0,
  fontFamily: 'var(--font-sans)',
  fontSize: 'var(--text-md)',
  lineHeight: 1.5,
  color: 'var(--text-primary)',
  whiteSpace: 'pre-line',
  textWrap: 'pretty'
}

const noteGrid = {
  ...toolGrid,
  gridTemplateColumns: `${TIME_COL} minmax(0, 1fr)`,
  cursor: 'default',
  color: 'var(--text-muted)',
  fontSize: 'var(--text-xs)'
}

const dayStyle = {
  marginTop: 'var(--space-3)',
  padding: 'var(--space-3) var(--space-4) 0',
  borderTop: 'var(--border-w) solid var(--border-subtle)',
  textAlign: 'center',
  fontFamily: 'var(--font-mono)',
  fontSize: 'var(--text-2xs)',
  color: 'var(--text-muted)'
}

const detailStyle = {
  padding: `var(--space-2) var(--space-4) var(--space-3) calc(var(--space-4) + ${TIME_COL} + ${TOOL_COL} + 2 * var(--space-3))`,
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  fontFamily: 'var(--font-mono)',
  fontSize: 'var(--text-sm)'
}

const fullCommand = {
  margin: 0,
  padding: 'var(--space-2) var(--space-3)',
  background: 'var(--surface-sunken)',
  borderRadius: 'var(--radius-2)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-mono)',
  whiteSpace: 'pre-wrap',
  wordBreak: 'break-word'
}

const outcomeLine = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  color: 'var(--text-secondary)',
  fontSize: 'var(--text-xs)'
}
</script>

<template>
  <div v-if="rows.length === 0" :style="{ ...scrollStyle, display: 'flex', alignItems: 'center', justifyContent: 'center' }">
    <EmptyState icon="inbox" title="No activity yet" description="The batch's remarks and tool calls appear here as they happen." />
  </div>
  <div v-else ref="scroller" :style="scrollStyle" @scroll="onScroll">
    <template v-for="row in rows" :key="row.key">
      <div v-if="row.kind === 'day'" :style="dayStyle">{{ row.date }}</div>

      <div v-else-if="row.kind === 'say'" :style="saySheet">
        <span :style="sayTime">{{ row.time }}</span>
        <div class="sm-prose" :style="sayText">
          <MarkdownInline :nodes="row.nodes" @open="openExternal" />
        </div>
      </div>

      <div v-else-if="row.kind === 'note'" :style="noteGrid">
        <span :style="timeStyle">{{ row.time }}</span>
        <span :style="commandStyle">{{ row.text }}</span>
      </div>

      <div
        v-else
        :style="wrapStyle(row.key)"
        @mouseenter="hovered = row.key"
        @mouseleave="hovered = null"
      >
        <div :style="toolGrid" @click="toggle(row.key)">
          <span :style="timeStyle">{{ row.time }}</span>
          <Tooltip :label="row.tool" :enabled="row.tool !== row.toolLabel">
            <span :style="{ fontSize: 'var(--text-xs)', color: row.tone }">{{ row.toolLabel }}</span>
          </Tooltip>
          <span :style="commandStyle"
            ><span :style="{ color: 'var(--text-primary)' }">{{ row.head }}</span
            ><span v-if="row.rest" :style="{ color: 'var(--text-secondary)' }">{{ ' ' + row.rest }}</span></span
          >
          <span :style="scopeCell">
            <span v-if="row.live" :style="dotStyle" />
            <Tooltip v-if="row.scope" :label="row.scope.title" side="left">
              <span :style="tagStyle(row.scope.colors)">{{ row.scope.label }}</span>
            </Tooltip>
          </span>
        </div>

        <div v-if="expanded === row.key" :style="detailStyle">
          <pre v-if="row.command" :style="fullCommand">{{ row.command }}</pre>
          <div v-if="row.result" :style="outcomeLine">
            <Icon
              :name="row.result.ok ? 'check' : 'x'"
              :size="12"
              :style="{ color: row.result.ok ? 'var(--status-done-fg)' : 'var(--status-failed-fg)' }"
            />
            <span>{{ row.result.summary }}</span>
          </div>
          <div v-else-if="row.live" :style="{ ...outcomeLine, color: 'var(--attn-live)' }">
            <span :style="dotStyle" />
            <span>running</span>
          </div>
          <div v-else :style="{ ...outcomeLine, color: 'var(--text-muted)' }">
            <span>no result</span>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
