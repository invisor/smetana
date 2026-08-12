<script setup>
/* A run's report, drawn as the document it is.

   `srcdoc` and not a `file://` URL. The alternative is Tauri's asset protocol,
   which costs a new capability scope and a new ACL entry for what is at bottom
   showing a local text file the app itself wrote a moment ago; `srcdoc` needs
   neither. The HTML arrives as a string through the `files_read` this app
   already has, so the report inherits `tabs.js`'s `loading`, `error` and buffer
   handling with nothing added — including the 2 MiB ceiling in
   `files::model::MAX_FILE_BYTES`, which a text report will not approach.

   The `sandbox` attribute is empty, which is every restriction on at once: no
   scripts, no same-origin, no forms, no navigation. `report.rs` writes no
   script and reaches no network, and a test there pins both — this attribute is
   what makes that a guarantee rather than a habit, for a document that has been
   sitting on somebody's disk since last night and can be hand-edited between
   then and now.

   The document carries its own CSS and follows neither this app's theme nor its
   density, deliberately: it has to be readable in a browser with nothing of ours
   loaded, which is exactly why `report.rs` is a documented exception to the
   token rule. There are no tokens inside the frame to reach for and none is
   offered. */

defineProps({
  html: { type: String, default: '' }
})

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
    <iframe :srcdoc="html" sandbox="" title="Run report" :style="frameStyle" />
  </div>
</template>
