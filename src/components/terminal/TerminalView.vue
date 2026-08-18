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
import { terminalFont, terminalTheme } from './theme.js'
import {
  attach,
  detach,
  isShellSession,
  isStarting,
  resize,
  send,
  subscribeOutput,
  terminalState
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
  padding: 'var(--space-3)'
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

/* Fitting the terminal to its pane has nothing to do with whether a session
   is attached — an empty terminal still has to fill the space it is given.
   Only the worker side of it, telling the PTY its new size, needs a session
   to send that to. */
function applySize() {
  if (!fit || !term) return
  fit.fit()
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
    if (meta?.reset) term.reset()
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

  if (props.sessionId && !isStarting(props.sessionId)) {
    attached = props.sessionId
    attach(attached).then(applySize)
  }
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
    attach(id).then(applySize)
  }
)

onBeforeUnmount(() => {
  unsubscribe?.()
  observer?.disconnect()
  sizes?.disconnect()
  /* Detach exactly the session this view attached to: the store's pointer
     may have already moved to another agent by now, and a nameless detach
     would silence the wrong one. */
  detach(attached)
  attached = null
  term?.dispose()
  /* An attach in flight still resolves into applySize after this. Forgetting
     both here is what makes that harmless by construction, rather than by the
     fit addon happening to bail on a detached element. */
  term = null
  fit = null
})
</script>

<template>
  <div :style="wrapStyle">
    <div ref="host" :style="style" />
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
