<script setup>
/* The projects this window is working with: one is active, the rest are one
   click away. A folder with no bd tracker still belongs here — it is a
   project you added, and the mark says what it is missing, quietly.

   No header of its own: the enclosing Panel already shows "Projects" and
   carries the "+" in its actions slot, so a second copy here would print the
   same word twice in a row. */
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Tooltip from '../core/Tooltip.vue'
import { useInteractive } from '../core/interactive.js'

const props = defineProps({
  projects: { type: Array, default: () => [] },
  activePath: { type: String, default: null }
})

const emit = defineEmits(['select', 'remove'])

/* Hover has to be per row, and useInteractive tracks one control — so the
   list keeps the hovered path itself and asks useInteractive for nothing.
   Press is not tracked here: a row is not a button, it is a place. */
const hovered = ref(null)

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
    boxShadow: active ? 'inset var(--border-w-strong) 0 0 0 var(--text-primary)' : 'none',
    color: active ? 'var(--text-primary)' : 'var(--text-secondary)',
    font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
    cursor: 'default',
    transition: 'var(--transition-control)'
  }
}

const nameStyle = { flex: 1, minWidth: 0, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }

/* A context menu here would get clipped by the list's own scroll container
   (overflow-y in listStyle) no matter which way it opened, and moving it
   outside the list would mean measuring the DOM for the sake of one single
   item — so removal is a button, not a menu. The button's box is always in
   the layout (visibility, not v-if/display), so revealing it on hover or
   active never shifts the row's own content. */
const removeButtonStyle = (project) => ({
  visibility: hovered.value === project.path || project.path === props.activePath ? 'visible' : 'hidden'
})

const empty = computed(() => props.projects.length === 0)
</script>

<template>
  <div :style="{ display: 'flex', flexDirection: 'column', minWidth: 0 }">
    <div v-if="empty" :style="{ padding: 'var(--space-5)', fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }">
      No projects yet.
    </div>

    <div v-else :style="listStyle">
      <div
        v-for="p in projects"
        :key="p.path"
        :style="rowStyle(p)"
        :title="p.path"
        @click="emit('select', p.path)"
        @mouseenter="hovered = p.path"
        @mouseleave="hovered = null"
      >
        <span :style="nameStyle">{{ p.name }}</span>
        <Tooltip v-if="!p.tracked" label="No bd tracker here" side="right">
          <Icon name="triangle-alert" :size="12" :style="{ color: 'var(--text-muted)' }" />
        </Tooltip>
        <IconButton
          icon="x"
          label="Remove from list"
          size="sm"
          :style="removeButtonStyle(p)"
          @click.stop="emit('remove', p.path)"
        />
      </div>
    </div>
  </div>
</template>
