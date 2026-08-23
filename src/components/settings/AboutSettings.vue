<script setup>
/* The About tab: what this app is, which build it is, and where the source
   lives. Everything on it is static except the version, which is handed in —
   asking Tauri for it is the window's business, and a component that did it
   itself would be the first one in `src/components/` to know Tauri exists. */
import { computed } from 'vue'
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'
import SettingsRow from './SettingsRow.vue'
import { useInteractive } from '../core/interactive.js'
/* The whole of what this tab says about updating, out here where a test can
   reach it. This component decides where the row sits and what it is made of,
   and nothing about which of the six states the app is in. */
import { installRefusal, updateAction, updateLine, updatesKnown } from './update.js'
/* The one raster in the interface, and the one place the constraint against
   images is lifted: this is the app icon itself, the same artwork the Dock and
   the installer draw, so anything redrawn from tokens here would be a second
   version of it to keep in step. It is the icon's own body without the 1024
   canvas margin `tauri icon` needs (see `scripts/make-app-icon.py`), because the
   margin is transparent and the layout below sizes the tile itself. */
import appIcon from '../../assets/app-icon.png'

const props = defineProps({
  /* `null` where there is nobody to ask — a browser. Drawn as a dash rather
     than as a guess: a wrong version in a bug report is worse than none. */
  version: { type: String, default: null },
  repository: { type: String, default: 'https://github.com/invisor/smetana' },
  /* `UpdateState` whole, in Rust's own shape, or `null` where there is nobody
     to ask. The window reads it from `stores/updates.js`; this component never
     asks anybody anything, which is what keeps it drawable in the gallery with
     nothing behind it. */
  updateState: { type: Object, default: null },
  /* Why the last press on Install was refused, in one line — the run gate above
     all, which this window cannot see for itself and must not guess at. Held by
     the window rather than by the state, because it is the answer to a press
     and not a state the machine is in: `updates.rs` deliberately leaves the
     machine at `ready` through a refusal, since what was downloaded is still
     downloaded. */
  updateRefusal: { type: [Object, String], default: null }
})

/* `check` and `install` carry nothing: there is one update machine and the
   window knows how to reach it. The verb is the pure rule's (`updateAction`),
   so the label — which is prose — is never what the window switches on. */
const emit = defineEmits(['open', 'check', 'install'])

const showsUpdate = computed(() => updatesKnown(props.updateState))
const updateSentence = computed(() => updateLine(props.updateState))
const updateControl = computed(() => updateAction(props.updateState))
const refusal = computed(() => installRefusal(props.updateRefusal))

const { hover, handlers } = useInteractive()

/* Fixed pixels, and deliberately not scaled by `--ui-scale`: every other glyph
   in the app is a numeric literal on `Icon` for the same reason, and a tile that
   grew while the ~35 icons around it stayed put would be the odd one out. The
   artwork carries its own black ground and its own squircle, so it needs no
   border and no radius from here — that ground is what makes it read on the
   paper of the light theme and on the ink of the dark one alike. */
const iconStyle = {
  display: 'block',
  flexShrink: 0,
  width: '64px',
  height: '64px'
}

const nameStyle = {
  color: 'var(--text-primary)',
  font: 'var(--weight-semibold) var(--text-xl)/var(--leading-tight) var(--font-sans)'
}
const versionStyle = {
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-mono)'
}
const proseStyle = {
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-body-size)/var(--leading-normal) var(--font-sans)',
  maxWidth: '52ch'
}
/* Two rules here are about one defect, and it is worth naming both because
   either one alone leaves half of it on screen.

   `alignSelf` is the first: this anchor is a flex item of the column above it,
   whose `align-items` is the default `stretch`, and an `inline-flex` item is
   blockified to `flex` — so the link was as wide as the whole panel however
   short the address inside it was. `flex-start` gives it back its own width.

   The underline is the second, and it sits on the text rather than on the
   anchor. A decoration set on the box is drawn across the box, so it ran under
   the icon and under the gap between them as well as under the address. Under
   the words is what a link means. */
const linkStyle = {
  display: 'inline-flex',
  alignSelf: 'flex-start',
  alignItems: 'center',
  gap: 'var(--space-2)',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-ui-size)/1 var(--font-mono)',
  textDecoration: 'none',
  cursor: 'default'
}

const addressStyle = computed(() => ({
  textDecoration: hover.value ? 'underline' : 'none'
}))

/* Under the row rather than in its description, and the same shape the Storage
   tab gives its own refusals: a row's description says what a control does,
   while this says what happened when somebody pressed it. */
const refusalStyle = {
  color: 'var(--status-failed-fg)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
</script>

<template>
  <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-4)', paddingTop: 'var(--space-4)' }">
    <div :style="{ display: 'flex', alignItems: 'center', gap: 'var(--space-4)' }">
      <!-- `alt` is empty on purpose: the name is right beside it, and a reader
           that announced the picture would say the word twice. -->
      <img :src="appIcon" alt="" :style="iconStyle" width="64" height="64" />
      <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-1)' }">
        <span :style="nameStyle">Smetana</span>
        <span :style="versionStyle">{{ props.version ? `v${props.version}` : '—' }}</span>
      </div>
    </div>

    <!-- The update row, directly under the version it is about, and drawn only
         where there is somebody to ask: in a browser there is no updater, and a
         row offering a check nothing could carry out would be the same invention
         as a version number made up for the line above. -->
    <template v-if="showsUpdate">
      <SettingsRow label="Updates" :description="updateSentence" control-width="22ch">
        <Button
          v-if="updateControl"
          :variant="updateControl.verb === 'install' ? 'primary' : 'secondary'"
          :disabled="updateControl.disabled"
          @click="emit(updateControl.verb)"
        >
          {{ updateControl.label }}
        </Button>
      </SettingsRow>
      <p v-if="refusal" :style="refusalStyle">{{ refusal }}</p>
    </template>

    <p :style="proseStyle">
      An open-source desktop app for supervising autonomous coding agents: one window over a
      project's bd issue tracker, its worktree files and the agent sessions working in them. It
      notices when an agent is waiting on a person, including one in a tab nobody is looking at.
    </p>

    <p :style="proseStyle">
      Source, issues and releases are on GitHub. Contributions are welcome.
    </p>

    <!-- An anchor, so the address is there to copy and to read before it is
         followed; the navigation itself is refused, because inside this webview
         it would replace the app with a web page and leave no way back. The
         window opens it in the person's own browser instead. -->
    <a
      :href="props.repository"
      :style="linkStyle"
      v-on="handlers"
      @click.prevent="emit('open', props.repository)"
    >
      <Icon name="external-link" :size="13" />
      <span :style="addressStyle">{{ props.repository }}</span>
    </a>
  </div>
</template>
