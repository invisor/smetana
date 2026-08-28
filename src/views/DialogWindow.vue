<script setup>
/* One dialog, in a window of its own.

   A fifth branch in `App.vue` beside the app, the gallery, the settings window
   and the compare window, and built the way those last two are: the same bundle
   under a `?view=` of its own, so there is one front end, one set of tokens and
   one place a component can break — and so this screen stays checkable in
   `npm run dev` with no Tauri behind it (`?view=dialog&kind=new-branch`).

   **This window holds no store.** It is a view of what the app window holds,
   exactly as `SettingsWindow.vue` is: props arrive by event, the guest's emits
   go back by event, and the app window remains the only thing that talks to bd,
   to git or to a run. That is what lets the guest components stay
   presentational and unrewritten — the only change any of them saw is that
   `Modal.vue` now draws two frames instead of one.

   The height is measured here rather than named anywhere. A dialog's height is
   whatever its content comes to, and the content changes — a validation line, a
   progress report — so a number in the registry would be wrong at the first
   field somebody adds, and wrong again under a different `--ui-scale` or in
   compact. The window is built hidden; the first measurement is what puts it on
   screen. Rust does the sizing, because `core:default` grants neither
   `set_size` nor `show` and adding them would publish both to every window in
   the app for the sake of one call. */
import { computed, onMounted, onUnmounted, provide, reactive, ref, shallowRef, watchEffect } from 'vue'
import DeleteSessionModal from '../components/agent/DeleteSessionModal.vue'
import EmptyState from '../components/core/EmptyState.vue'
import DeleteTaskModal from '../components/kanban/DeleteTaskModal.vue'
import NewBranchModal from '../components/git/NewBranchModal.vue'
import NewTaskModal from '../components/kanban/NewTaskModal.vue'
import PromoteColumnModal from '../components/kanban/PromoteColumnModal.vue'
import ReadyTaskModal from '../components/kanban/ReadyTaskModal.vue'
import RunModal from '../components/run/RunModal.vue'
import SetupProjectModal from '../components/run/SetupProjectModal.vue'
import { dialogWidth, isDialogKind } from './dialogRegistry.js'
import { EDITOR_FONT_DEFAULT, UI_FONT_DEFAULT, effectiveTheme } from '../appearance.js'
import { paintRoot, usePrefersDark } from './useAppearance.js'
import { emitDialogResult, sizeDialogWindow, watchDialogProps } from '../stores/app.js'
import { readSharedSettings, watchSharedSettings } from '../stores/settings.js'

const props = defineProps({
  /* Which dialog this window is, off `?kind=` — `dialog_window_open` put it
     there and checked it on the way (`kind_query`). Read in `App.vue` with the
     rest of the query string, so this file never sees a URL.

     Optional rather than required, because a bare `?view=dialog` is reachable
     by hand in the dev server and a missing parameter is an empty window, not a
     warning in a console nobody is looking at. */
  kind: { type: String, default: null },
  /* The query string's two overrides, passed down for the reason the settings
     and compare windows take theirs: they win over what the app window says,
     for this run only and never written back, and without them this window's
     own chrome could not be looked at in compact or in the other theme at
     all. */
  themeOverride: { type: String, default: null },
  densityOverride: { type: String, default: null }
})

/* The one place a dialog kind becomes a component. It is here rather than in
   `dialogRegistry.js` because importing a `.vue` file would pull Vue into a
   module whose whole point is having neither Vue nor a DOM in it — and that
   module is the half a test can reach. */
const COMPONENTS = {
  run: RunModal,
  'new-task': NewTaskModal,
  'new-branch': NewBranchModal,
  'promote-column': PromoteColumnModal,
  'setup-project': SetupProjectModal,
  'delete-task': DeleteTaskModal,
  'ready-task': ReadyTaskModal,
  'delete-session': DeleteSessionModal
}

const component = computed(() => COMPONENTS[props.kind] ?? null)

/* What `Modal.vue` reads to know it is drawing inside a window: no scrim, no
   header of its own, no border, no radius, no shadow — the OS frame carries all
   of it. Provided here and nowhere else, which is what leaves the gallery and
   the app window exactly as they were. */
provide('smDialogWindow', true)

/* The one store a dialog window holds, and it is held for one kind.

   Everything else here is a view of what the app window owns, which is what
   `SettingsWindow.vue` is and what the header above promises. Images are the
   exception the design settled on, and the reason is not tidiness: Tauri
   intercepts a file drop before any webview sees it and reports it against the
   *window* it landed on. A person filing a task drops a screenshot on **this**
   window, so this is the only process that can hear it — a list kept in the app
   window would simply never be told. `stores/attachments.js` carries the whole
   argument in its own header.

   Loaded lazily, and only for `new-task`, which is a statement about ownership
   before it is anything else: this file is the one place in `src/` that reaches
   the list, and the shape says so where a reader is standing.

   What it saves is small and worth knowing exactly, because the obvious guesses
   are all wrong. It saves nothing in the shipped app: `notifications.js` imports
   the same store for `surveyStorage`, so it lands in the one chunk whatever this
   line does — Vite prints a note to that effect on every build — and the
   `import()` compiles to a resolved promise over a namespace the chunk has
   already evaluated. It does not save the subscription either: the call below is
   already under a check on the kind. What it does save is `npm run dev`, where
   modules are unbundled and six dialog windows skip a fetch and an evaluation
   apiece. */
const attachments = shallowRef(null)

async function holdAttachments() {
  const store = await import('../stores/attachments.js')
  attachments.value = store
  /* Always accepting, unlike the app window's own call, which asked whether the
     dialog was open. Here the window *is* the dialog: it exists because
     somebody opened it and it is destroyed when they close it, so there is no
     state in which a drop on it should be refused. The predicate that used to
     keep this store's ears off a drop meant for a terminal is not needed either
     — that subscription is the app window's webview and this is not it. */
  stops.push(store.watchDrops(() => true))
}

/* The three emits a `new-task` guest raises about its images, answered here
   rather than forwarded. They are the other half of the store living in this
   window: sending them to the app window would be asking the one process that
   cannot hear a drop to keep the list that a drop goes into. */
const answerHere = {
  attach: () => attachments.value?.pickImages(),
  files: (files) => attachments.value?.attachFiles(files),
  remove: (path) => attachments.value?.removeAttachment(path)
}

/* The props the app window is feeding this dialog. `open` is forced true: a
   window that exists is open, and that prop only ever meant "is the scrim
   up".

   `title` travels with them because the OS frame draws it and nothing on this
   side knows what this dialog is called. It reaches the guest too, harmlessly:
   a guest that declares no `title` prop passes it through to its `Modal`, which
   is the same string that `Modal` was already being given. */
const incoming = shallowRef({})
const guestProps = computed(() => {
  const announced = { ...incoming.value, open: true }
  /* The images are this window's own state and are laid over the announcement
     rather than taken from it — the app window has none of them to send. It
     announces `busy`, `status`, `parent` and `title` for this kind and nothing
     about pictures, so nothing is being overwritten here. */
  const store = attachments.value
  if (!store) return announced
  return {
    ...announced,
    attachments: store.attachmentsState.items,
    dragging: store.attachmentsState.dragging,
    error: store.attachmentsState.lastError ?? ''
  }
})
const title = computed(() => incoming.value.title ?? 'Smetana')

/* Whether this window has heard what it is drawing yet.

   It gates two things, and the second was learned from a dialog that lost its
   settings. The first is the **first** `sizeDialogWindow`, because that one call
   does three things that cannot be taken back: it sizes the window, it puts it
   over the main window, and it is the only thing that ever shows one. The
   measurement is ready a frame after mount and the props are three IPC hops
   behind it, so without this the window was placed at the height of a dialog
   with no content in it and captioned `Smetana` — the fallback above — and by
   the time the real title arrived the window was visible and its one placement
   was spent.

   The second is **mounting the guest at all**, and the reason is that a guest
   reads some of its props exactly once. `RunModal` fills its mode, its floor,
   its parallelism and its two switches from `remembered` and from the project's
   config in a watcher on `open`, which fires once, on mount — every one of them
   is a choice a person made last time, and the branch field is the only one with
   a late-fill watcher behind it. Mounted before the announcement, the window
   opened on this component's own fallbacks and quietly threw away the whole
   remembered run. Nothing draws until there is something to draw with, which is
   free here: the window is hidden until the measurement below anyway, and the
   measurement is gated on this same flag.

   The timeout is the other half, and it is what keeps the failure visible: a
   dialog nobody announces anything for is still a window somebody opened, and
   one that never appeared at all would be the same silence as the missing
   component below. It shows late and wrong rather than not at all. */
const told = ref(false)
const FIRST_PAINT_WAIT = 250
const stopWaiting = setTimeout(() => {
  told.value = true
}, FIRST_PAINT_WAIT)

/* Every emit the eight guests have between them that crosses back to the app
   window, forwarded by name. A list rather than a wildcard because listeners
   need names, and because a name that is not here is a message that would
   silently go nowhere. A name here that a guest does not declare costs nothing:
   it falls through as an inert listener for a DOM event that never fires. */
const EMITS = ['close', 'confirm', 'create', 'submit', 'resolve', 'rescope']

/* And the three that deliberately do not travel: `new-task`'s images, answered
   in this window by the store above. They are a separate list rather than a
   branch inside the loop below so that the division is a thing somebody reads
   rather than a condition they have to work out — the app window has no handler
   for any of them, and one of these names appearing in `EMITS` would be a
   button that draws normally and does nothing at all when pressed.

   **For that one kind and no other.** These names belong to the images and to
   nothing else in this host, but nothing reserves them: a later guest raising
   an `attach` of its own would have it swallowed here as an inert no-op instead
   of crossing to the app window — the same silent button, one list over. `kind`
   comes off the URL and never changes for the life of a window, so the question
   is settled once rather than watched. */
const HOSTED_EMITS = props.kind === 'new-task' ? ['attach', 'files', 'remove'] : []

const on = (name) => `on${name[0].toUpperCase()}${name.slice(1)}`
const listeners = {
  ...Object.fromEntries(
    EMITS.map((name) => [on(name), (payload) => emitDialogResult(props.kind, name, payload)])
  ),
  ...Object.fromEntries(HOSTED_EMITS.map((name) => [on(name), answerHere[name]]))
}

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

/* The measurement. A `ResizeObserver` rather than a watcher on the props: what
   decides the height is the rendered result, and only the box knows it — a
   hint wrapping onto a second line changes nothing any prop could be watched
   for.

   Deliberately no `min-height` on the box below. A root as tall as the viewport
   would measure the window rather than the content, and the window could then
   grow and never shrink again. */
const root = ref(null)
const measured = ref(0)
let observer = null

/* How much of the window reaches the page, which is the second number the
   sizing call carries and the whole of smetana-s30.

   A window is told a height and hands the page less than that: a title bar's
   worth on macOS, a title bar and borders elsewhere, nothing at all where the
   two agree. Neither end can work the difference out on its own — Rust knows
   what it set, this side knows what arrived — so this number is sent beside the
   height and Rust subtracts. `window::height_to_set` holds the argument, the
   measurements, and why the difference is not `outer_size - inner_size`.

   Read again on every resize, and that is what makes the arithmetic
   self-correcting rather than a single shot that has to be right first time. A
   size that lands wrong is a resize, and the resize is what sends the corrected
   number; a size that lands right recomputes to itself, so `set_size` is handed
   the size the window already has, no resize follows and it stops there. */
const viewport = ref(window.innerHeight)
const readViewport = () => {
  viewport.value = window.innerHeight
}
window.addEventListener('resize', readViewport)

/* Height, viewport and title go over together, and a change to any of them
   sends all three: the title is what the frame draws, and it arrives by event
   like everything else, so a dialog whose name changed with its height
   unchanged would otherwise keep the frame it had. Nothing is sent before the
   first measurement — there is no height to send, and this call is also what
   shows the window. */
watchEffect(() => {
  if (told.value && measured.value > 0) {
    sizeDialogWindow(props.kind, measured.value, viewport.value, title.value)
  }
})

const stops = []

onMounted(async () => {
  if (root.value) {
    observer = new ResizeObserver(([entry]) => {
      measured.value = Math.ceil(entry.contentRect.height)
    })
    observer.observe(root.value)
  }
  /* Before the props are asked for, so a file dropped in the first moments of
     this window has somewhere to go. It answers nothing about what is drawn, so
     it is started rather than awaited: the subscription below must not wait
     behind a dynamic import. */
  if (props.kind === 'new-task') {
    holdAttachments().catch((err) => {
      console.warn('[dialog-window] the attachment store did not load:', err)
    })
  }
  /* A kind this build has never heard of gets no subscription at all: there is
     nothing to draw and nothing to ask about. What is on screen in that case is
     the template's own note at the foot of this file. */
  if (isDialogKind(props.kind)) {
    try {
      stops.push(
        await watchDialogProps(props.kind, (next) => {
          incoming.value = next ?? {}
          told.value = true
        })
      )
    } catch (err) {
      console.warn('[dialog-window] no app window to be told what to draw by:', err)
    }
  }
  try {
    stops.push(await watchSharedSettings((state) => adopt(state, true)))
  } catch (err) {
    console.warn('[dialog-window] no app window to follow:', err)
  }
  try {
    const stored = await readSharedSettings()
    if (!heard.value) adopt(stored, false)
  } catch (err) {
    console.warn('[dialog-window] the settings could not be read:', err)
  }
})

onUnmounted(() => {
  clearTimeout(stopWaiting)
  window.removeEventListener('resize', readViewport)
  observer?.disconnect()
  for (const stop of stops) stop()
})

/* The width is the registry's rather than the window's, and that is what makes
   this screen checkable in a browser: in the window the two are the same number,
   and in `npm run dev` this is what keeps the dialog the width it is in the app
   instead of stretching across the tab. */
const rootStyle = computed(() => ({
  width: `${dialogWidth(props.kind)}px`,
  maxWidth: '100%',
  background: 'var(--surface-overlay)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  fontSize: 'var(--text-body-size)'
}))
</script>

<template>
  <div ref="root" :style="rootStyle">
    <component
      :is="component"
      v-if="component && told"
      v-bind="{ ...guestProps, ...listeners }"
    />
    <!-- A kind with no component behind it. Drawn rather than left blank
         because the first measurement is what shows this window at all: an
         empty root measures nothing, nothing is sent, and the result is a
         window that exists, holds the label and can never be seen or closed —
         so pressing the menu item looks like nothing happening, and pressing it
         again focuses a window nobody can find. This is the one failure of the
         mechanism with no symptom of its own, and this is its symptom. -->
    <EmptyState
      v-else-if="!component"
      icon="triangle-alert"
      tone="error"
      title="This dialog has nothing to draw"
      description="No component is registered for this dialog kind."
    >
      <!-- The kind itself goes in the `detail` slot rather than into the
           sentence above: it is an identifier, and identifiers are drawn in
           mono. That slot is what every other empty state in this tree puts its
           diagnostic in. -->
      <template #detail>{{ kind ?? '(no kind given)' }}</template>
    </EmptyState>
  </div>
</template>
