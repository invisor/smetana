<script setup>
/* The compare window: what one branch differs from the branch this repository
   is on by, as a real OS window of its own rather than a modal — so it can be
   dragged out beside the board it is about and left there while somebody reads
   both. It is the same bundle under `?view=compare`, the fourth branch in
   `App.vue`, which is also what makes this screen checkable in `npm run dev`
   with no Tauri behind it.

   `SettingsWindow.vue` is the model, and the two are alike in everything that
   is about being a second window: it paints its own document root, since a
   webview has a root of its own and the app window's attributes reach nothing
   here; it takes the query string's two overrides as props, so `App.vue` stays
   the one place that reads a URL; and an already-open window is focused rather
   than rebuilt, so the pair it is aimed at arrives twice — off the URL when it
   is built, and over `compare:show` when it is not.

   Where the two differ is what they own. The settings window holds no store
   because the app window is the only writer of `settings.json`; this window
   holds `stores/compare.js` entirely, because nothing here writes anything at
   all. Every read goes by the two shas `vcs_compare` resolved, never by the
   branch name — the store's header says why, and the short shas in the diff's
   own captions are that rule on screen.

   Freshness is window focus and the mode switch, which is the answer the Git
   panel, the file tree and the branch in the scope bar all give: a git call per
   change would be a process per change. It matters more here than in any of
   them — an agent committing into this very tree is the ordinary case in this
   app — which is exactly why the endpoints are read once and held. */
import { computed, onMounted, onUnmounted, reactive, ref, watchEffect } from 'vue'
import CompareList from '../components/git/CompareList.vue'
import DiffView from '../components/files/editor/DiffView.vue'
import EmptyState from '../components/core/EmptyState.vue'
import Icon from '../components/core/Icon.vue'
import { EDITOR_FONT_DEFAULT, UI_FONT_DEFAULT, effectiveTheme } from '../appearance.js'
import { paintRoot, usePrefersDark } from './useAppearance.js'
import { readSharedSettings, watchSharedSettings } from '../stores/settings.js'
import {
  aim,
  compareState,
  refresh,
  select,
  setMode,
  watchCompareTarget
} from '../stores/compare.js'
import { fileErrorText } from '../stores/files.js'

const props = defineProps({
  /* The query string's two overrides, passed down rather than read here so that
     `App.vue` stays the one place that knows about them. They win over what the
     app window says, for this run only and never written back — the same
     precedence `DesktopApp` and `SettingsWindow` give them, and the only way
     this window's own chrome can be looked at in the other theme and in
     compact. */
  themeOverride: { type: String, default: null },
  densityOverride: { type: String, default: null },
  /* Which repository and which branch, off `?repo=` and `?branch=` — the pair
     `compare_window_open` percent-encoded into the URL it built. Read in
     `App.vue` with the rest of the query string, for the reason above. A window
     that is already open is re-aimed by the event instead, since it never sees
     a new URL. */
  repo: { type: String, default: null },
  branch: { type: String, default: null }
})

/* How this window is painted, in the shape the two windows speak in. The
   defaults are the shipped ones, so it paints itself correctly in the moment
   before the first answer arrives rather than flashing a light theme at
   somebody working in the dark one — `SettingsWindow` records the same
   reasoning at greater length, since it has a switch for every one of them and
   this window has none. */
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

let stopWatching = null
let stopTarget = null

/* Whatever the window is asked to look at, from either door. A pair with a hole
   in it is ignored rather than aimed at: the store would clear the list and ask
   nothing, which is a window that went blank for no stated reason. */
const aimAt = (repo, branch) => {
  if (repo && branch) aim(repo, branch)
}

onMounted(async () => {
  try {
    stopWatching = await watchSharedSettings((state) => adopt(state, true))
  } catch (err) {
    console.warn('[compare-window] no app window to follow:', err)
  }
  try {
    stopTarget = await watchCompareTarget(aimAt)
  } catch (err) {
    console.warn('[compare-window] no app window to hear a new comparison from:', err)
  }
  try {
    const stored = await readSharedSettings()
    if (!heard.value) adopt(stored, false)
  } catch (err) {
    console.warn('[compare-window] the settings could not be read:', err)
  }
  aimAt(props.repo, props.branch)
  window.addEventListener('focus', refresh)
})

onUnmounted(() => {
  stopWatching?.()
  stopTarget?.()
  window.removeEventListener('focus', refresh)
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

/* An object name is unreadable at full length and identifies nothing more at
   seven characters than git itself prints. The same seven the diff's captions
   carry, since they name the same two commits. */
const SHORT = 7
const shortLeft = computed(() => compareState.left.slice(0, SHORT))
const shortRight = computed(() => compareState.right.slice(0, SHORT))

/* Which branch this window is on, from the store rather than from the prop: a
   window that has been re-aimed is looking at the pair the event named, and the
   URL it was built with is the past. */
const branchName = computed(() => compareState.branch ?? props.branch ?? '')

/* The comparison's own refusal, as against the one file's — two states, in two
   places, because they are two different sentences.

   git's stderr is a diagnostic addressed to whoever can act on it, so it goes
   on the mono line under a title this app can stand behind; the two refusals
   this command raises itself are whole sentences written for the person, and
   repeating a title over one of them would be the panel saying it twice. */
const refusal = computed(() => {
  const error = compareState.error
  if (!error) return null
  return error.kind === 'noSuchBranch' || error.kind === 'unrelated'
    ? { title: error.message, detail: null }
    : { title: 'These two branches could not be compared.', detail: error.message }
})

/* Whether this window has a pair to have compared at all.

   `aimAt` above ignores a pair with a hole in it rather than aiming at half of
   one, so a window opened as a bare `?view=compare` — which is how the dev
   server reaches this screen — asks git nothing: the list is empty, nothing is
   loading and nothing was refused. That is precisely the state the list reads
   as "these two branches are identical", so without this the one screen this
   project checks by eye would open on a claim about a comparison it never
   made. The window is what knows whether it was ever aimed; the list is handed
   an answer and should not have to guess whether there was a question. */
const aimed = computed(() => Boolean(compareState.repo && compareState.branch))

/* The refusal of the one file, in words, through the editor's own table: a file
   refused as binary in a tab has to be refused in the same words here, and the
   kinds are the same on both sides of the wire — `VcsError::kind` carries
   `FilesError`'s strings deliberately. */
const fileNotice = computed(() =>
  compareState.fileError ? fileErrorText(compareState.fileError) : ''
)

const rootStyle = {
  display: 'flex',
  flexDirection: 'column',
  height: '100vh',
  background: 'var(--canvas)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  fontSize: 'var(--text-md)',
  overflow: 'hidden'
}

/* What is being compared with what, once and at the top. The window's own title
   bar says only that this is a comparison — the operating system draws it, and
   it cannot carry a sha — so this line is the whole of what names the two
   endpoints, and the diff's captions below repeat the shas over the columns
   they are about. */
const headerStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  flex: '0 0 auto',
  height: 'var(--scope-bar-h)',
  padding: '0 var(--space-5)',
  background: 'var(--surface)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)'
}

/* The one word of prose on the line, and it is what stops the sha beside it
   from being an unlabelled seven characters: in the default mode the left-hand
   side is where the two branches parted, and in the direct one it is the
   current branch's tip. "Base" is true of both, and naming the current branch
   there would not be — this window compares against `HEAD`, which has no name
   to invent at a detached checkout. */
const captionStyle = {
  flex: 'none',
  color: 'var(--text-muted)',
  fontFamily: 'var(--font-sans)'
}
const arrowStyle = { flex: 'none', color: 'var(--text-muted)' }
const nameStyle = {
  flex: '0 1 auto',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
/* The two object names are muted and the branch name is not: the name is what
   a person came here about, and the shas are what says which two commits they
   are actually reading — the second fact, in the second tone. */
const shaStyle = { ...nameStyle, flex: 'none', color: 'var(--text-muted)' }

const bodyStyle = { display: 'flex', flex: 1, minHeight: 0 }

/* The file list at a panel's width and fixed there, because what is beside it
   is a diff: two columns of code take every pixel they are given, and a list of
   paths does not.

   The **wider** of the two panel widths, which is the one thing here that is
   not simply the app's left column copied over. This list carries the mode
   switch above it, and "From where they diverged" is a clause rather than a
   word: at `--panel-left-w` the pair did not fit and left the panel sideways.
   The right panel's width is the system's next size up, so the measure is still
   the design system's rather than one invented here. */
const listColumnStyle = {
  display: 'flex',
  flex: 'none',
  width: 'var(--panel-right-w)',
  minHeight: 0,
  background: 'var(--surface)',
  borderRight: 'var(--border-w) solid var(--border-subtle)'
}

/* `minWidth: 0` beside `minHeight: 0`, and both are load-bearing: a flex item
   defaults to `min-width: auto` and refuses to shrink below its own content, so
   a wide file would push the two panes out over the list beside them. */
const paneStyle = {
  display: 'flex',
  flex: 1,
  minWidth: 0,
  minHeight: 0,
  overflow: 'hidden'
}
</script>

<template>
  <div :style="rootStyle">
    <div :style="headerStyle">
      <Icon name="git-compare" :size="13" :style="{ flex: 'none', color: 'var(--text-muted)' }" />
      <span :style="captionStyle">Base</span>
      <span :style="shaStyle">{{ shortLeft }}</span>
      <span :style="arrowStyle">→</span>
      <span :style="nameStyle" :title="branchName">{{ branchName }}</span>
      <span :style="shaStyle">{{ shortRight }}</span>
    </div>
    <div :style="bodyStyle">
      <div :style="listColumnStyle">
        <CompareList
          :style="{ flex: 1, minWidth: 0 }"
          :files="compareState.files"
          :selected="compareState.selected"
          :mode="compareState.mode"
          :settled="aimed && !compareState.loading && !refusal"
          @select="select"
          @update:mode="setMode"
        />
      </div>
      <div :style="paneStyle">
        <!-- The comparison could not be made at all, which is a different
             emptiness from a pair of identical branches and says so in its own
             words. The switch beside it stays live on purpose: unrelated
             histories refuse the diverged reading and answer the direct one,
             so the way out of this refusal is one press away. -->
        <EmptyState
          v-if="refusal"
          tone="error"
          :title="refusal.title"
          :style="{ flex: 1, minWidth: 0 }"
        >
          <template v-if="refusal.detail" #detail>{{ refusal.detail }}</template>
        </EmptyState>
        <!-- Two of the four names differ across this seam. `missingAtHead`
             keeps the name it has always had on the component — "the left side
             has no such file" — and is fed the store's `missingLeft`; renaming
             it would touch the diff tab for nothing. The captions are the two
             shas, which is what the panes actually hold: "HEAD" and "Working
             tree" would both be wrong here. -->
        <DiffView
          v-else-if="compareState.selected"
          :path="compareState.selected ?? ''"
          :head="compareState.head"
          :work="compareState.work"
          :missing-at-head="compareState.missingLeft"
          :left-caption="shortLeft"
          :right-caption="shortRight"
          :notice="fileNotice"
        />
        <!-- Nothing picked yet, which is how the window opens. It says what to
             do rather than apologising for being empty. -->
        <EmptyState
          v-else
          icon="git-compare"
          title="Pick a file to see how the two sides differ."
          :style="{ flex: 1, minWidth: 0 }"
        />
      </div>
    </div>
  </div>
</template>
