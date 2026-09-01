<script setup>
/* The image window: one picture attached to a task nobody has filed yet, shown
   whole in an OS window of its own — so it can be dragged out beside the dialog
   it came from, made as large as the screen, and left there. It is the same
   bundle under `?view=image`, the sixth branch in `App.vue`, which is also what
   makes this screen checkable in `npm run dev` with no Tauri behind it.

   It exists because the picture used to be an overlay inside the new-task
   dialog, and that dialog became a window of its own 440 points wide that
   cannot be resized: "the picture, larger" came out no larger than the dialog
   it was opened from (smetana-msxp). `ImageViewer.vue` carries the rest of that
   history, and it is this window's whole body.

   `CompareWindow.vue` is the model, and the two are alike in everything that is
   about being a second window: it paints its own document root, since a webview
   has a root of its own and the app window's attributes reach nothing here; it
   takes the query string's two overrides as props, so `App.vue` stays the one
   place that reads a URL; and an already-open window is focused rather than
   rebuilt, so the picture arrives twice — off the URL when the window is built,
   and over `image:show` when it is not.

   **What travels is the path, and the bytes stay where they are.** An
   attachment's `url` is a `data:` URL of up to 8 MiB of base64: it fits in no
   URL, and sending it over the event channel would be eleven megabytes per
   click. This window reads the file itself with `readAttachment`, which is one
   command — `attachment_reopen`, already confined to the store by
   `cleanup::in_store` — and keeps nothing. It is the one reader of
   `stores/attachments.js` that holds no list and hears no drop, and that is
   what leaves the rule in `.claude/rules/attachments.md` intact: the list still
   belongs to the New task window alone, and a command is not a subscription. */
import { onMounted, onUnmounted, reactive, ref, watchEffect } from 'vue'
import EmptyState from '../components/core/EmptyState.vue'
import ImageViewer from '../components/overlays/ImageViewer.vue'
import { EDITOR_FONT_DEFAULT, UI_FONT_DEFAULT, effectiveTheme } from '../appearance.js'
import { paintRoot, usePrefersDark } from './useAppearance.js'
import { readSharedSettings, watchSharedSettings } from '../stores/settings.js'
import { closeWindow, watchImageShow } from '../stores/app.js'
import { readAttachment } from '../stores/attachments.js'

const props = defineProps({
  /* The query string's two overrides, passed down rather than read here so that
     `App.vue` stays the one place that knows about them. They win over what the
     app window says, for this run only and never written back — the same
     precedence the other windows give them, and the only way this window can be
     looked at in the other theme and in compact. */
  themeOverride: { type: String, default: null },
  densityOverride: { type: String, default: null },
  /* Which picture, off `?path=` and `?name=` — the pair `image_window_open`
     percent-encoded into the URL it built. Read in `App.vue` with the rest of
     the query string, for the reason above. A window that is already open never
     sees a new URL and is re-aimed by the event instead. */
  path: { type: String, default: null },
  name: { type: String, default: '' }
})

/* How this window is painted, in the shape the app's windows speak in. The
   defaults are the shipped ones, so it paints itself correctly in the moment
   before the first answer arrives rather than flashing a light theme at
   somebody working in the dark one — `CompareWindow` records the same reasoning
   beside the same four fields. */
const view = reactive({
  theme: 'dark',
  density: 'comfortable',
  uiFontSize: UI_FONT_DEFAULT,
  editorFontSize: EDITOR_FONT_DEFAULT
})

/* Whether the app window has spoken. The disk read below is the fall-back for
   the moment before it does — and for `npm run dev`, where there is no app
   window at all — so an answer from the file that lands second must not
   overwrite the newer truth. */
const heard = ref(false)

const adopt = (state, fromApp) => {
  if (!state) return
  if (fromApp) heard.value = true
  for (const field of Object.keys(view)) {
    if (field in state && state[field] != null) view[field] = state[field]
  }
}

/* The picture on screen, and what is being said instead when there is none.

   `reading` is its own state rather than an absent picture, and it is what
   stops this window opening on a refusal it has not had yet: the read is a
   round trip, and drawing the empty state in the meantime would announce that
   somebody's screenshot was gone every single time one is opened.

   `label` is kept beside the picture rather than taken from the props, because
   a window that has been re-aimed is showing what the event named and the URL
   it was built with is the past — the same reason `CompareWindow` reads its
   branch off its store. It is also what the empty state says: a file that could
   not be read has no record to take a name from, so the name has to be the one
   that was asked for. */
const picture = ref(null)
const label = ref(props.name ?? '')
const reading = ref(Boolean(props.path))

/* The guard is `seq` and it plays the part `compareSeq` plays in
   `stores/compare.js`: two reads can be in flight with no ordering guarantee,
   and without it the last *response* would win rather than the last *call*.
   Here that lands as one picture drawn under another one's name — click
   thumbnail A, click B while A is still being read, and a slow A resolving last
   would paint A under the caption and the frame title B. The same defect
   `loadDiff`, `terminals.js` and `git.js` all guard against. */
let showSeq = 0

/* Whichever picture this window is aimed at, from either door. A path that is
   not there at all is the bare `?view=image` of the dev server and of nothing
   else: it says so rather than reading nothing and calling it a missing file. */
async function show(path, name) {
  const seq = ++showSeq
  label.value = name ?? ''
  if (!path) {
    /* No guard on this branch and none wanted: nothing has been awaited yet, so
       this call is still the newest by construction. */
    picture.value = null
    reading.value = false
    return
  }
  reading.value = true
  try {
    const attachment = await readAttachment(path)
    if (seq !== showSeq) return
    picture.value = attachment
    /* The record's own name wins over the one on the URL: it is what the store
       actually holds, and the caption under the picture has to be the file. */
    label.value = attachment.name || label.value
  } catch (err) {
    if (seq !== showSeq) return
    /* An ordinary outcome, not a fault: the Storage tab's button sweeps files
       no open task refers to, and a draft can still be naming one of them. It
       is said in the empty state below, with the name of what is missing —
       which is why this is `debug` and not `error`. A red line in the console
       for a state the window is already explaining would be the app reporting a
       bug it does not have. */
    console.debug('[image-window] that picture is not in the store:', err)
    picture.value = null
  } finally {
    /* Behind the guard, unlike the two above it only in what it costs to get
       wrong: a stale read clearing this would put the window back on its empty
       state while the newer picture is still on its way. */
    if (seq === showSeq) reading.value = false
  }
}

let stopWatching = null
let stopShow = null

onMounted(async () => {
  try {
    stopWatching = await watchSharedSettings((state) => adopt(state, true))
  } catch (err) {
    console.warn('[image-window] no app window to follow:', err)
  }
  try {
    stopShow = await watchImageShow(show)
  } catch (err) {
    console.warn('[image-window] no app window to hear a new picture from:', err)
  }
  /* The URL's picture goes in here, immediately after the subscription and
     **before** the settings are read. That order is the whole of it: the read
     below is a real IPC round trip in the app, and an `image:show` arriving
     inside it would be answered first and then overwritten by the URL — the
     window ending up on the picture it was built for rather than the one it was
     just re-aimed at. The URL is the oldest thing this window knows, so it has
     to be spent before anything can be awaited on top of it. Not awaited
     itself, for the same reason: the settings are about how this window is
     painted and have nothing to do with which picture is in it. */
  show(props.path, props.name)
  try {
    const stored = await readSharedSettings()
    if (!heard.value) adopt(stored, false)
  } catch (err) {
    console.warn('[image-window] the settings could not be read:', err)
  }
})

onUnmounted(() => {
  stopWatching?.()
  stopShow?.()
  document.removeEventListener('keydown', onKeydown)
})

/* This window paints itself: it is a separate webview with its own document
   root, so the app window's attributes reach nothing here. `system` is resolved
   against the machine and follows it live, which is the whole reason
   `usePrefersDark` is a listener rather than a reading. */
const prefersDark = usePrefersDark()
watchEffect(() => {
  paintRoot(document.documentElement, {
    theme: props.themeOverride ?? effectiveTheme(view.theme, prefersDark.value),
    density: props.densityOverride ?? view.density,
    uiFontSize: view.uiFontSize,
    editorFontSize: view.editorFontSize
  })
})

/* Esc, and the frame's own button, come to the same thing. `closeWindow` is
   `close()` rather than `destroy()`, and it needs `core:window:allow-close`
   granted to this window's label in `capabilities/default.json` — a label not
   listed there reaches no core plugin at all and this would log one line and do
   nothing. */
const close = () => closeWindow()

/* Esc closes this window whatever it happens to be showing, and the guard is
   what keeps that one behaviour in one place at a time. `ImageViewer` carries a
   listener of its own and emits `close` while a picture is up; this one is for
   the two states where that component is not drawn at all — the swept file and
   the bare `?view=image` — where otherwise a window with no picture in it would
   be the one window in the app that ignores the key. */
const onKeydown = (event) => {
  if (event.key !== 'Escape' || picture.value) return
  event.preventDefault()
  close()
}

/* Subscribed at setup rather than on mount, and taken off in `onUnmounted`
   above: this window's first paint may already be the empty state, and a key
   pressed before the first frame should still close it. */
document.addEventListener('keydown', onKeydown)

/* `position: relative` is the one line here that another window would not need:
   `ImageViewer` is `position: absolute; inset: 0`, which is what keeps it
   inside its frame in the gallery, so this root has to be the positioned
   ancestor it measures itself against. Without it the picture would be laid out
   against the initial containing block, which happens to be the same rectangle
   in this window and would stop being one the moment anything else was drawn
   here. */
const rootStyle = {
  position: 'relative',
  height: '100vh',
  background: 'var(--canvas)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  fontSize: 'var(--text-md)',
  overflow: 'hidden'
}

const emptyStyle = {
  display: 'flex',
  height: '100%',
  alignItems: 'center',
  justifyContent: 'center',
  padding: 'var(--space-6)'
}
</script>

<template>
  <div :style="rootStyle">
    <!-- The picture, and the whole of what this window is for. -->
    <ImageViewer v-if="picture" :url="picture.url" :name="label" @close="close" />
    <!-- Nothing drawn while the file is being read: a round trip is a few
         milliseconds, and an empty state in that window would say a picture was
         gone every time one is opened. -->
    <div v-else-if="!reading" :style="emptyStyle">
      <!-- The file is not in the store any more, which is an ordinary state
           rather than a fault: the Storage tab's button sweeps what no open
           task refers to, and a draft can still be naming one of them. The name
           goes in the `detail` slot because that is where this system puts an
           identifier — and it is the only thing left that says which picture
           this window was about. -->
      <EmptyState
        v-if="label"
        icon="image-off"
        title="This picture is not in the store any more."
        description="It was cleared from the Storage tab, or the file was moved. The task can still be filed without it."
      >
        <template #detail>{{ label }}</template>
      </EmptyState>
      <!-- A bare `?view=image` with no picture named at all: the dev server's
           way in, and nothing the app can produce. It says that rather than
           reporting a file that was never asked for as missing. -->
      <EmptyState
        v-else
        icon="image-off"
        title="No picture was named."
        description="Click a thumbnail in the Images section of the New task window to open one here."
      />
    </div>
  </div>
</template>
