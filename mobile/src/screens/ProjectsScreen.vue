<script setup>
/* The projects open on the desktop, with the one it has in front marked. Choosing another one
   switches the desktop's too - the epic decided the phone follows the desktop's single active
   project instead of keeping one of its own - and the line at the top says so, because a tap
   here is felt at the workstation. */
import { computed } from 'vue'
import Icon from '@src/components/core/Icon.vue'
import EmptyState from '@src/components/core/EmptyState.vue'
import { settings } from '@src/stores/settings.js'
import { trackerState } from '@src/stores/tracker.js'
import { basename } from '@src/paths.js'
import { projectRows } from '../desktopSettings.js'
import { chooseProject, workspace } from '../workspace.js'

const emit = defineEmits(['switched'])

const rows = computed(() => projectRows(settings.openProjects, settings.activeProject, basename))

async function choose(row) {
  if (row.active) {
    emit('switched')
    return
  }
  if (await chooseProject(row.path)) emit('switched')
}

const note = {
  margin: 0,
  padding: 'var(--space-4) var(--space-5)',
  background: 'var(--surface-sunken)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-secondary)'
}
const failure = {
  ...note,
  background: 'transparent',
  color: 'var(--status-failed-fg)'
}
const row = (active, busy) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  width: '100%',
  minHeight: 'var(--control-h-lg)',
  padding: 'var(--space-4) var(--space-5)',
  border: 0,
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  background: active ? 'var(--surface-selected)' : 'transparent',
  color: 'var(--text-primary)',
  textAlign: 'left',
  opacity: busy ? 'var(--attn-quiet-opacity)' : 1,
  cursor: 'pointer'
})
const names = { flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 'var(--space-2)' }
const nameStyle = { font: 'var(--weight-medium) var(--text-sm)/1 var(--font-mono)' }
const pathStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  direction: 'rtl',
  textAlign: 'left'
}
const mark = {
  flex: '0 0 auto',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-secondary)'
}
</script>

<template>
  <div>
    <p :style="note">Choosing a project here switches the project on the desktop too.</p>
    <p v-if="trackerState.lastError && !workspace.switching" :style="failure">
      The project could not be switched. {{ trackerState.lastError.description }}
    </p>
    <EmptyState
      v-if="!rows.length"
      icon="folder"
      title="No project is open"
      description="Open a project on the desktop and it appears here."
    />
    <button
      v-for="item in rows"
      :key="item.path"
      type="button"
      :disabled="!!workspace.switching"
      :style="row(item.active, workspace.switching && workspace.switching !== item.path)"
      @click="choose(item)"
    >
      <span :style="names">
        <span :style="nameStyle">{{ item.name }}</span>
        <span :style="pathStyle"><bdo dir="ltr">{{ item.path }}</bdo></span>
      </span>
      <Icon v-if="workspace.switching === item.path" name="loader-circle" :size="14" />
      <span v-else-if="item.active" :style="mark">On desktop</span>
    </button>
  </div>
</template>
