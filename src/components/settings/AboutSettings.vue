<script setup>
/* The About tab: what this app is, which build it is, and where the source
   lives. Everything on it is static except the version, which is handed in —
   asking Tauri for it is the window's business, and a component that did it
   itself would be the first one in `src/components/` to know Tauri exists. */
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import { useInteractive } from '../core/interactive.js'

const props = defineProps({
  /* `null` where there is nobody to ask — a browser. Drawn as a dash rather
     than as a guess: a wrong version in a bug report is worse than none. */
  version: { type: String, default: null },
  repository: { type: String, default: 'https://github.com/invisor/smetana' }
})

const emit = defineEmits(['open'])

const { hover, handlers } = useInteractive()

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
const linkStyle = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-ui-size)/1 var(--font-mono)',
  textDecoration: hover.value ? 'underline' : 'none',
  cursor: 'default'
}))
</script>

<template>
  <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-4)', paddingTop: 'var(--space-4)' }">
    <div :style="{ display: 'flex', alignItems: 'baseline', gap: 'var(--space-3)' }">
      <span :style="nameStyle">Smetana</span>
      <span :style="versionStyle">{{ props.version ? `v${props.version}` : '—' }}</span>
    </div>

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
      <span>{{ props.repository }}</span>
    </a>
  </div>
</template>
