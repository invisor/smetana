<script setup>
/* Host for xterm.js. One Terminal instance per view, not per session: on
   switching agents, reset() and a fresh fill from the ring. The cost is
   that returning to an agent lands at the end of its output, not wherever
   it was scrolled to. An instance per session would fix that, but that is
   editor/states.js territory, and building it before the lack is shown to
   matter is premature. */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
// xterm ships its own stylesheet, and without it the terminal does not
// render at all — this import is part of the same sanctioned exception
// theme.js explains, not an oversight of the "no CSS" rule.
import '@xterm/xterm/css/xterm.css'
import { terminalFont, terminalTheme } from './theme.js'
import { attach, detach, resize, send, subscribeOutput, terminalState } from '../../stores/terminals.js'

const host = ref(null)
let term = null
let fit = null
let unsubscribe = null
let observer = null
let sizes = null
/* The session this view is attached to right now — the one it detaches on
   unmount too. Deliberately not reactive: nothing displays it. */
let attached = null

const style = { flex: 1, minHeight: 0, background: 'var(--editor-bg)', padding: 'var(--space-3)' }

/* Fitting the terminal to its pane has nothing to do with whether a session
   is attached — an empty terminal still has to fill the space it is given.
   Only the worker side of it, telling the PTY its new size, needs a session
   to send that to. */
function applySize() {
  if (!fit || !term) return
  fit.fit()
  if (terminalState.activeId) resize(terminalState.activeId, term.cols, term.rows)
}

onMounted(() => {
  term = new Terminal({ ...terminalFont(), theme: terminalTheme(), scrollback: 5000, allowProposedApi: true })
  fit = new FitAddon()
  term.loadAddon(fit)
  term.open(host.value)

  term.onData((data) => {
    if (terminalState.activeId) send(terminalState.activeId, data)
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
     Recompute and reassign. */
  observer = new MutationObserver(() => {
    term.options.theme = terminalTheme()
    Object.assign(term.options, terminalFont())
    applySize()
  })
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'data-density'] })

  sizes = new ResizeObserver(applySize)
  sizes.observe(host.value)

  if (terminalState.activeId) {
    attached = terminalState.activeId
    attach(attached).then(applySize)
  }
})

/* Switched to a different agent — the new ring's snapshot arrives with
   meta.reset, and the subscriber above clears the screen before writing it.

   Losing the selection while the view stays mounted (switching to a project
   with no sessions, removing the last agent) is the same seam and needs the
   same work: without it the previous session's frame keeps sitting on screen
   while the worker still calls it active and encodes its bytes every tick for
   a listener that drops every one — and typing goes nowhere, because onData
   guards on activeId. Clearing activeId is not this view's to do, though:
   the selection belongs to the store. */
watch(
  () => terminalState.activeId,
  (id) => {
    if (!id) {
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
  <div ref="host" :style="style" />
</template>
