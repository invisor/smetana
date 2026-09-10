<script setup>
/* An html document, drawn as the document it is.

   It began as a run's report and is now every `.html` in the project — see
   `reportTab.js` for why the folder stopped being the rule. Nothing in this file
   changed for that, and the paragraph on the sandbox below is why: it was
   already written for a file that had been sitting on somebody's disk since last
   night, and the widening only makes it load-bearing where it had been prudent.

   `srcdoc` and not a `file://` URL. The alternative is Tauri's asset protocol,
   which costs a new capability scope and a new ACL entry for what is at bottom
   showing a local text file the app itself wrote a moment ago; `srcdoc` needs
   neither. The HTML arrives as a string through the `files_read` this app
   already has, so the report inherits `tabs.js`'s `loading`, `error` and buffer
   handling with nothing added — including the 2 MiB ceiling in
   `files::model::MAX_FILE_BYTES`, which a text report will not approach.

   **It takes two things to shut this frame, and the sandbox is only one of
   them.** `sandbox=""` is every restriction that attribute has: no scripts, no
   same-origin, no forms, no navigation — and it stops a `<meta http-equiv=
   refresh>` firing at all, which was measured rather than assumed. What it has
   never stopped is the document *loading* things. Driven against a local server
   logging every request, a `srcdoc` frame with an empty sandbox fetched a
   `<link rel=stylesheet>`, an `<img>` and an `@font-face` — all three, from the
   person's own machine. This comment used to claim otherwise, in bold, which is
   the worst kind of wrong a header can be.

   The other half is `reportTheme.js`'s content policy, written into the string
   the frame is built from because the empty sandbox is exactly what puts the
   frame's DOM out of reach. `default-src 'none'` with inline styles and
   `data:` images allowed, and the same probe afterwards logged nothing at all.
   Nothing was inherited to lean on instead: `csp` in `tauri.conf.json` is
   `null`.

   That mattered little while the only writers were this app and its own agent.
   It matters now: one click on any `.html` in the tree renders a file from a
   repository somebody is supervising rather than one they wrote, and every URL
   in it would otherwise be a beacon fired at an address its author chose.
   Scripts still do not run and the origin is still opaque, so this was never
   code execution — but a beacon is precisely what this feature's design
   forbids. Rendering "properly, like a browser" — scripts, external
   stylesheets, fonts, images — was considered and turned down for the same
   reason: a different feature with a different threat model, not this one grown
   a little.

   A document that wants a stylesheet, a font or a picture therefore gets none,
   and that is visible rather than hidden — a page written against a CDN
   stylesheet draws as unstyled text here. That is the accepted outcome: the
   document this frame exists for carries its own CSS, because it has to be
   readable in a browser with nothing of ours loaded, which is exactly why
   `report.rs` is a documented exception to the token rule. That document was
   checked against the policy rather than reasoned about — the same report
   rendered with and without it is pixel-identical, since it reaches nowhere and
   its whole appearance is one inline `<style>`. There are still no tokens inside
   the frame to reach for, and none is offered.

   **It does follow this app's palette, though, and the empty sandbox is what
   decides how.** Nothing on this side can reach the frame's own DOM to set an
   attribute on it, so the choice has to be in the string the frame is built from:
   `reportTheme.js` names the theme on the document's root element on its way into
   `srcdoc`, and the palettes it selects between are the document's own. Nothing on
   disk is touched. A theme change therefore rebuilds `srcdoc` and the frame
   reloads, which is a blink on a switch somebody throws by hand and the price of
   keeping the sandbox shut.

   Density is deliberately not carried across, and neither is the app-wide font
   size: those are about fitting rows into panels and about this app's chrome,
   while a document has a measure of its own. */

import { computed } from 'vue'

import { documentFor } from './reportTheme.js'

const props = defineProps({
  /* The buffer's current text and never the bytes on disk, which is what makes
     the source mode's unsaved edits visible the moment somebody switches back:
     `srcdoc` is rebuilt from this string, so there is nothing to save and
     nothing to refresh. */
  html: { type: String, default: '' },
  /* Already resolved to one of the two painted themes: `system` is `App.vue`'s to
     answer, since it is the machine's answer rather than a stored one. A value
     this component does not recognise reaches `themed`, which declines it and
     leaves the document reading `prefers-color-scheme`. */
  theme: { type: String, default: 'dark' }
})

/* Computed, which is the whole of "the tab repaints on a theme change": the frame
   is built from this string, so a new theme is a new document with no reopening
   and nothing to remember.

   One call and not two: `documentFor` is the theme stamp and the content policy
   composed in `reportTheme.js`, where a test can reach the pair. Composing them
   here would put half of "themed, and cannot reach the network" in a file no
   runner in this repository opens. */
const page = computed(() => documentFor(props.html, props.theme))

/* `minWidth: 0` beside `minHeight: 0`, and it is the load-bearing one. A flex
   item defaults to `min-width: auto` and so refuses to shrink below its own
   content — see `TerminalView.vue`, where the same omission left the pane
   hanging over the task panel as the centre column narrowed. The document
   inside is laid out by the frame's own viewport, so without this the frame
   keeps whatever width it was last given and the column cannot squeeze it.

   The ground is a token because the frame is transparent until the document
   paints: an empty `html` — a buffer still loading, or one that failed to read
   — would otherwise show whatever sits behind the centre column. */
const hostStyle = {
  flex: 1,
  minWidth: 0,
  minHeight: 0,
  display: 'flex',
  overflow: 'hidden',
  background: 'var(--canvas)'
}

const frameStyle = {
  flex: 1,
  minWidth: 0,
  minHeight: 0,
  border: 'none',
  background: 'transparent'
}
</script>

<template>
  <div :style="hostStyle">
    <iframe :srcdoc="page" sandbox="" title="Document" :style="frameStyle" />
  </div>
</template>
