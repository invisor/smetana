<script setup>
/* Host for xterm.js. One Terminal instance per view, not per session: on
   switching agents, reset() and a fresh fill from the ring. The cost is
   that returning to an agent lands at the end of its output, not wherever
   it was scrolled to. An instance per session would fix that, but that is
   editor/states.js territory, and building it before the lack is shown to
   matter is premature. */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
// xterm ships its own stylesheet, and without it the terminal does not
// render at all — this import is part of the same sanctioned exception
// theme.js explains, not an oversight of the "no CSS" rule.
import '@xterm/xterm/css/xterm.css'
import EmptyState from '../core/EmptyState.vue'
import { dropText } from './dropPaths.js'
import { terminalFont, terminalTheme } from './theme.js'
import {
  attach,
  detach,
  isShellSession,
  isStarting,
  resize,
  send,
  subscribeOutput,
  terminalState,
  watchSessionDrops
} from '../../stores/terminals.js'

/* Which session this pane shows, handed in rather than read from the store.

   `terminalState.activeId` used to be both the answer to this and the answer to
   "which agent has the person selected", and the two came apart the moment a
   session could be something other than an agent: a shell drawn in its own
   centre tab would have moved the highlight in the agents panel onto a row that
   does not exist there, and taken the Agent tab off whatever it was showing.
   So the field keeps the one meaning it is named for, the Agent tab passes it
   in, and a terminal tab passes its own shell.

   `null` is a pane with nothing behind it — the Agent tab with no agent picked
   — and it draws the empty state rather than nothing at all. */
const props = defineProps({
  sessionId: { type: [String, Number], default: null }
})

const host = ref(null)
let term = null
let fit = null
let unsubscribe = null
let observer = null
let sizes = null
/* The session this view is attached to right now — the one it detaches on
   unmount too. Deliberately not reactive: nothing displays it. */
let attached = null

/* Whether a file is over this pane right now, and how many came with it. The
   count is read off the enter event, which is the only one carrying the paths;
   until one arrives it is zero and the caption speaks in the singular. */
const dropping = ref(false)
const dropCount = ref(0)
let stopDrops = null

/* `minWidth: 0` on both, and it is load-bearing in a way `minHeight: 0` next to
   it is not. A flex item defaults to `min-width: auto`, which means it refuses
   to shrink below its own content — and this item's content is xterm.js, whose
   width comes from the cols it was last fitted to. Narrow the column and the
   pane cannot follow: it stays as wide as the terminal used to be and hangs
   over the task panel, painted on top of it because this wrapper is positioned
   and that column is not.

   It looked animated because it converged: ResizeObserver → fit() → new cols →
   xterm re-renders a frame later → the floor drops a little → the observer
   fires again. fit() was measuring a pane sized by its own last answer instead
   of by the column. `KanbanBoard` and `FileEditor` never showed this because
   `overflow: auto`/`hidden` zeroes that automatic minimum for them already. */
const wrapStyle = { position: 'relative', flex: 1, minWidth: 0, minHeight: 0, display: 'flex' }
const style = {
  flex: 1,
  minWidth: 0,
  minHeight: 0,
  background: 'var(--editor-bg)',
  padding: 'var(--space-3)',
  /* Where the rounding remainder goes, and it goes up. A terminal is a whole
     number of cells tall and the pane it is given almost never is, so there is
     always up to one cell of space left over; `.xterm` is the host's one child
     and is exactly `rows * cell` tall, so a column packed to the end spends
     that leftover above the first row, against the tab bar, and leaves the
     distance from the last row to the bottom of the dark ground at exactly the
     padding on every window height and on every machine. Left at the default it
     sat under the last row instead — next to the status footer and directly
     under the agent's own prompt line, which is the one place on this pane a
     person is looking at. */
  display: 'flex',
  flexDirection: 'column',
  justifyContent: 'flex-end'
}

/* With no session attached the pane was simply black, which reads as a
   terminal that has not printed anything yet rather than as one with nothing
   behind it. The explanation is drawn over the host, not in place of it: the
   host has to keep its real geometry, because that is what fit() measures and
   what the first resize() sends to the PTY — a hidden host would size the
   session to nothing and the first frame would arrive wrapped wrong. */
const overlayStyle = {
  position: 'absolute',
  inset: 0,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  background: 'var(--editor-bg)'
}

/* The response while a file is over the pane: a frame around the terminal and
   one line of caption over it. Without it the gesture is invisible and cannot be
   told apart from the broken state it replaces, where a drop did nothing and
   said nothing about it.

   `pointerEvents: 'none'` is load-bearing rather than tidy. The aim is a hit
   test — `document.elementFromPoint` at the drag's own position, asked whether
   it landed inside the xterm host — and this element sits over that host as a
   sibling. Taking pointer events would make it the answer to that question, the
   next event of the same drag would read the pane as not ours, and the response
   would switch itself off the moment it appeared. */
const dropStyle = {
  position: 'absolute',
  inset: 'var(--space-3)',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-3)',
  background: 'var(--overlay-scrim)',
  pointerEvents: 'none'
}
const dropLabelStyle = {
  padding: 'var(--space-3) var(--space-5)',
  background: 'var(--surface-overlay)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-pill)',
  boxShadow: 'var(--shadow-overlay)',
  color: 'var(--text-primary)',
  fontSize: 'var(--text-ui-size)',
  lineHeight: 'var(--leading-normal)'
}

/* The selection names an agent that is still being spawned. It is neither idle
   — somebody has just asked for this and is watching it — nor attachable, so it
   gets its own word rather than either of the two the empty state already has:
   "No agent selected" under a row a person picked themselves reads as the app
   having lost it. */
const starting = computed(() => isStarting(props.sessionId))
const idle = computed(() => !props.sessionId)
/* Agents, not sessions: this empty state belongs to the Agent tab, which is the
   only pane that can be handed nothing, and a project whose shells are open is
   still a project with no agent in it. */
const noSessions = computed(() => !terminalState.sessions.some((s) => !isShellSession(s)))

/* A pane with a session behind it that the worker has already answered for.
   Nothing is promised in any other state: `send` drops what is written to a
   session still coming up, so a frame offering to take a path there would be
   offering something this app cannot do. */
const live = computed(() => !!props.sessionId && !isStarting(props.sessionId))

/* Whose drop this is. The point arrives in CSS pixels from the top left of the
   viewport (`watchSessionDrops` does that conversion), so the browser can be
   asked outright what is drawn there, and the pane takes only what lands inside
   its own xterm host.

   Which pane of this window, and nothing more. The argument with the new task
   dialog is not this test's any more — that dialog is a window of its own and
   Tauri delivers a drop only to the window it landed on — but an overlay added
   inside *this* window would be separated by this same property rather than by
   a list of exceptions, which is why the test stays a hit test. */
function insideHost(x, y) {
  if (!host.value) return false
  const el = document.elementFromPoint(x, y)
  return !!el && host.value.contains(el)
}

const dropCaption = computed(() =>
  dropCount.value > 1 ? `Drop to insert ${dropCount.value} file paths` : 'Drop to insert the file path'
)

/* How many rows the pane can really draw, and why this is not the fit addon's
   own answer.

   `FitAddon.proposeDimensions` measures `getComputedStyle(host).height` and then
   subtracts the padding of the `.xterm` element inside it, which has none. Under
   this project's global `box-sizing: border-box` that computed height is the
   **border box** — the host's own `--space-3` padding is counted inside it — so
   the addon divides a height two paddings larger than the terminal has to draw
   in, and answers with up to one row more than fits. Measured against
   @xterm/addon-fit 0.11.0 in Chromium 136 with a 13px cell: a 400px pane was
   proposed 30 rows where 29 fit, and the extra row was drawn through the bottom
   padding and out of the host's box, over whatever sits below it.

   That overflow, rather than a rounding remainder left empty, is what made the
   gap under the last line a fact about the machine: as the pane's height crosses
   a multiple of the cell the distance from the last row to the bottom of the
   dark ground walks the whole cycle from minus a padding to plus one, and where
   a person's window happened to land decided which end of it they saw.

   So the row count is taken from the host's **content** box — `clientHeight`
   less its two paddings, both read back as resolved lengths rather than named
   here — and the cell height from what xterm has actually drawn: `.xterm-screen`
   is sized `cols * cell` by `rows * cell`, so dividing it by `term.rows` is the
   same number `_core` holds, without reaching into `_core` for it.

   The width is deliberately left exactly as it was. `proposeDimensions` measures
   it the same wrong way and hands back one or two columns too many, but the
   column count is what the agent's own output is wrapped at, so correcting it
   silently alongside this would change what a running session looks like — a
   separate defect with its own decision behind it.

   Nothing here writes a size onto the host, and that is what keeps the
   `ResizeObserver` below convergent: the host stays `flex: 1`, its box is a
   function of the centre column alone, and a fit can therefore never be measured
   against its own last answer — the loop the `minWidth: 0` note above describes.
   Setting a height on the host, or a padding, would close exactly that circle. */
function fittedRows() {
  const screen = term.element?.querySelector('.xterm-screen')
  if (!screen || !host.value) return 0
  const box = getComputedStyle(host.value)
  const inner = host.value.clientHeight - parseFloat(box.paddingTop) - parseFloat(box.paddingBottom)
  const cell = screen.getBoundingClientRect().height / term.rows
  // A pane that is not laid out yet, or one whose terminal has not drawn a
  // frame, answers nothing rather than a row count divided by zero.
  if (!(cell > 0) || !(inner > 0)) return 0
  return Math.max(1, Math.floor(inner / cell))
}

/* Fitting the terminal to its pane has nothing to do with whether a session
   is attached — an empty terminal still has to fill the space it is given.
   Only the worker side of it, telling the PTY its new size, needs a session
   to send that to. */
function applySize() {
  if (!fit || !term) return
  const cols = fit.proposeDimensions()?.cols
  const rows = fittedRows()
  /* `term.resize` rather than `fit.fit()`, since the rows are this file's now,
     and guarded the way the addon guards its own: a resize with nothing to
     change still reflows the buffer, and this runs on every observed layout
     change. What the addon does either side of that call and this does not is
     one `_renderService.clear()` in front of it, which for the DOM renderer this
     app uses empties the row elements — and the resize repaints every one of
     them through `_fullRefresh` regardless, so what is dropped is a repaint of a
     repaint. A canvas or WebGL renderer would want it back with the addon. */
  if (rows && Number.isFinite(cols) && (cols !== term.cols || rows !== term.rows)) {
    term.resize(cols, rows)
  }
  if (props.sessionId) resize(props.sessionId, term.cols, term.rows)
}

onMounted(() => {
  term = new Terminal({ ...terminalFont(), theme: terminalTheme(), scrollback: 5000, allowProposedApi: true })
  fit = new FitAddon()
  term.loadAddon(fit)
  term.open(host.value)

  term.onData((data) => {
    if (props.sessionId) send(props.sessionId, data)
  })

  unsubscribe = subscribeOutput((bytes, meta) => {
    // An attach started before unmount can still answer after it: the
    // subscription is dropped there, but a chunk already in flight through
    // the store must not reach a disposed terminal.
    if (!term) return
    /* `meta.reset` marks a whole ring snapshot rather than a chunk of new
       output — the screen is cleared and refilled from the beginning — so the
       view belongs at the end of the buffer once that fill is in.

       In `write`'s own callback and not after the call: `write` only queues the
       bytes, the parser runs later, so scrolling on the line below would scroll
       a buffer the snapshot has not reached yet. `term` is re-read there rather
       than closed over, because the view can be unmounted between the two. */
    if (meta?.reset) {
      term.reset()
      term.write(bytes, () => term?.scrollToBottom())
      return
    }
    term.write(bytes)
  })

  /* A data-theme change on the root does not repaint the terminal by
     itself — its colours were already handed over as resolved strings.
     Recompute and reassign.

     `data-ui-font` is the same problem one field over: the app-wide font size
     rewrites the type scale on the root, and the terminal's size came off that
     scale as a number (`--text-xs`, read once). The attribute carries no value
     anybody reads — it exists so that a font change is an attribute change here
     too, and the re-read below then picks up the new size. `applySize` is what
     turns it into rows and columns the PTY agrees with. */
  observer = new MutationObserver(() => {
    // Disconnected on unmount, so this should never run after one; the guard is
    // here so that "nothing reaches this view once `term` is forgotten" holds
    // of every callback in this file rather than of three out of four.
    if (!term) return
    term.options.theme = terminalTheme()
    Object.assign(term.options, terminalFont())
    applySize()
  })
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-theme', 'data-density', 'data-ui-font']
  })

  sizes = new ResizeObserver(applySize)
  sizes.observe(host.value)

  /* Fitted here, in front of the attach below rather than after it, and the
     thing to have straight first is that the order this replaces was a race and
     not a fixed wrong size. `sizes.observe` two lines up delivers an initial
     callback of its own — `observe()` always does — and that callback is this
     same `applySize`. It lands at the next rendering opportunity, while
     `attach`'s snapshot lands after an IPC round trip, so which of the two came
     first was a matter of timing: where the frame boundary won, the ring was
     parsed at the pane's real size; where the round trip won, it was parsed at
     the Terminal's own default 80×24, which this pane is much taller than. This
     call is what takes the timing out of it, and it is also why the observer's
     initial callback is no reason to delete the line: that callback is now a fit
     with nothing to change, where before it was a repair.

     What such a repair does is worth writing down, because it is narrower
     than it looks: growing the row count pulls rows out of the scrollback above
     the cursor while there are any, and appends blank ones below once there are
     none. Measured against @xterm/xterm 6.0.0, a short ring comes out of both
     orders identically — so the empty tail under the output is not the
     terminal's own arithmetic alone, and fitting first is the floor rather than
     the whole of the answer.

     The other half is `applySize`'s second job, telling the PTY its size, and
     it is a smaller claim than it looks. `ring.rs` is cumulative — a snapshot is
     everything since the session started, trimmed only on overflow — so a screen
     some program drew for the worker's fixed 120×30 is in that snapshot
     whichever order these two calls go out in, and nothing here can take bytes
     back out of a ring. The old order asked for the redraw as well, in
     `.then(applySize)`, one round trip later. What changes is the gap: the
     resize is dispatched in front of the read now rather than behind it.
     Dispatched, not arriving — neither `invoke` is awaited, each command reaches
     the worker's queue through a task of its own, and nothing here depends on
     them landing in that order. */
  applySize()

  if (props.sessionId && !isStarting(props.sessionId)) {
    attached = props.sessionId
    attach(attached)
  }

  /* A drop never reaches the webview — Tauri reports it against the window
     instead — so the pane cannot listen for one itself and the store hands it
     over. What goes in is the path and nothing else: pressing Return stays with
     the person, because a path is nearly always part of a sentence rather than
     the whole of one, and a message sent on somebody's behalf cannot be taken
     back out of an agent. */
  stopDrops = watchSessionDrops({
    over: ({ x, y, paths }) => {
      if (paths) dropCount.value = paths.length
      dropping.value = live.value && insideHost(x, y)
    },
    leave: () => {
      dropping.value = false
    },
    drop: ({ x, y, paths }) => {
      dropping.value = false
      if (!live.value || !insideHost(x, y)) return
      const text = dropText(paths)
      if (text) send(props.sessionId, text)
    }
  })
})

/* Switched to a different agent — the new ring's snapshot arrives with
   meta.reset, and the subscriber above clears the screen before writing it.

   Losing the session while the view stays mounted (switching to a project
   with no sessions, removing the last agent) is the same seam and needs the
   same work: without it the previous session's frame keeps sitting on screen
   while the worker still calls it active and encodes its bytes every tick for
   a listener that drops every one — and typing goes nowhere, because onData
   guards on the prop. Clearing the selection is not this view's to do, though:
   it belongs to the store. */
watch(
  () => props.sessionId,
  (id) => {
    /* An agent still being spawned goes through this same seam: there is
       nothing to attach to yet, and leaving the previous session attached
       would keep the worker encoding its bytes every tick for a screen that is
       about to be replaced — and would show that agent's output under the new
       one's name in the meantime. The watcher fires again the moment the
       selection turns into a real id. */
    if (!id || isStarting(id)) {
      detach(attached)
      attached = null
      term?.reset()
      return
    }
    attached = id
    /* Fitted first here too, and here it is the smaller of the two halves
       onMounted sets out. The pane's geometry has not moved since the last fit,
       so the fit finds nothing to change; what is left is `applySize`'s other
       job, and a session being switched to may still be sitting on the worker's
       fixed 120×30, never having had a view of its own. Its ring already holds
       whatever it drew at that size and this cannot change that — the snapshot
       is cumulative — so what the order buys is only that the resize is
       dispatched before the read rather than a round trip after it. */
    applySize()
    attach(id)
  }
)

onBeforeUnmount(() => {
  unsubscribe?.()
  stopDrops?.()
  observer?.disconnect()
  sizes?.disconnect()
  /* Detach exactly the session this view attached to: the store's pointer
     may have already moved to another agent by now, and a nameless detach
     would silence the wrong one. */
  detach(attached)
  attached = null
  term?.dispose()
  /* An attach in flight can still answer after this, and so can a write already
     queued in the parser. The subscription is dropped and the observers are
     disconnected above, so in the ordinary case nothing reaches this view at
     all; forgetting the pair here is what makes the leftovers harmless by
     construction rather than by the order of those three lines. Every callback
     back into this view returns on a null `term` — the output subscriber, the
     snapshot's scroll, `applySize`, the theme observer — rather than the fit
     addon happening to bail on a detached element. */
  term = null
  fit = null
})
</script>

<template>
  <div :style="wrapStyle">
    <div ref="host" :style="style" />
    <div v-if="dropping" :style="dropStyle">
      <span :style="dropLabelStyle">{{ dropCaption }}</span>
    </div>
    <div v-if="starting" :style="overlayStyle">
      <EmptyState icon="bot" title="Starting the agent" description="Its output appears here in a moment." />
    </div>
    <div v-else-if="idle" :style="overlayStyle">
      <EmptyState
        icon="bot"
        :title="noSessions ? 'No agents in this project' : 'No agent selected'"
        :description="noSessions
          ? 'Start one with + on the project row, in the Agents tab of the left panel.'
          : 'Pick an agent in the Agents tab of the left panel.'"
      />
    </div>
  </div>
</template>
