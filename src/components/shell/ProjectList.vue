<script setup>
/* The projects this window is working with: one is active, the rest are one
   click away. A folder with no bd tracker still belongs here — it is a
   project you added, and the mark says what it is missing, quietly. */
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Tooltip from '../core/Tooltip.vue'
import ContextMenu from '../overlays/ContextMenu.vue'
import { useInteractive } from '../core/interactive.js'

const props = defineProps({
  projects: { type: Array, default: () => [] },
  activePath: { type: String, default: null }
})

const emit = defineEmits(['select', 'add', 'remove'])

/* Hover has to be per row, and useInteractive tracks one control — so the
   list keeps the hovered path itself and asks useInteractive for nothing.
   Press is not tracked here: a row is not a button, it is a place. */
const hovered = ref(null)
const menuFor = ref(null)

const MENU = [{ label: 'Remove from list' }]

const openMenu = (path) => {
  menuFor.value = menuFor.value === path ? null : path
}
const onMenuSelect = (path) => {
  menuFor.value = null
  emit('remove', path)
}

/* A left click picks the project — and if some other row's menu was left
   open, it should not go on hanging over a row that is no longer the one
   the user is looking at. */
const selectProject = (path) => {
  menuFor.value = null
  emit('select', path)
}

/* 10px uppercase mono: a label, not a sentence — the same header the panel
   used for the worktree line. */
const headerStyle = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 'var(--space-3)',
  height: 'var(--tab-h)',
  flex: '0 0 auto',
  padding: '0 var(--space-3) 0 var(--space-5)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}

/* Five rows and then it scrolls: the file tree under it must not be pushed
   off the bottom of the panel by a long list. */
const listStyle = {
  position: 'relative',
  flex: '0 0 auto',
  maxHeight: 'calc(5 * var(--row-h))',
  overflowY: 'auto',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}

const rowStyle = (project) => {
  const active = project.path === props.activePath
  return {
    position: 'relative',
    display: 'flex',
    alignItems: 'center',
    gap: 'var(--space-3)',
    height: 'var(--row-h)',
    padding: '0 var(--space-3) 0 var(--space-5)',
    background: active
      ? 'var(--surface-raised)'
      : hovered.value === project.path
        ? 'var(--surface-hover)'
        : 'transparent',
    boxShadow: active ? 'inset 2px 0 0 0 var(--text-primary)' : 'none',
    color: active ? 'var(--text-primary)' : 'var(--text-secondary)',
    font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
    cursor: 'default',
    transition: 'var(--transition-control)'
  }
}

const nameStyle = { flex: 1, minWidth: 0, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }

/* The menu hangs off its own row — top: 100% needs nothing measured, and the
   row is the positioning context (rowStyle sets position: relative). --z-overlay
   does not exist as a token; --z-dropdown is what a popped-up menu over other
   content uses (see shape.css). */
const menuStyle = {
  position: 'absolute',
  top: '100%',
  right: 0,
  zIndex: 'var(--z-dropdown)'
}

const empty = computed(() => props.projects.length === 0)
</script>

<template>
  <div :style="{ display: 'flex', flexDirection: 'column', minWidth: 0 }">
    <div :style="headerStyle">
      <span>Projects</span>
      <IconButton icon="plus" label="Add project" size="sm" @click="emit('add')" />
    </div>

    <div v-if="empty" :style="{ padding: 'var(--space-5)', fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }">
      No projects yet.
    </div>

    <div v-else :style="listStyle">
      <div
        v-for="p in projects"
        :key="p.path"
        :style="rowStyle(p)"
        :title="p.path"
        @click="selectProject(p.path)"
        @contextmenu.prevent="openMenu(p.path)"
        @mouseenter="hovered = p.path"
        @mouseleave="hovered = null"
      >
        <span :style="nameStyle">{{ p.name }}</span>
        <Tooltip v-if="!p.tracked" label="No bd tracker here" side="right">
          <Icon name="triangle-alert" :size="12" :style="{ color: 'var(--text-muted)' }" />
        </Tooltip>

        <div v-if="menuFor === p.path" :style="menuStyle">
          <ContextMenu :items="MENU" :width="180" @select="onMenuSelect(p.path)" />
        </div>
      </div>
    </div>
  </div>
</template>
