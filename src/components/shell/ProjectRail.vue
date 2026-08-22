<script setup>
/* The projects this window is working with, one 28px tile each, in a 44px strip
   down the far left.

   What a project row used to carry beside its name — set up for runs, new
   agent, remove from list — is in the tile's own menu now: a tile this size has
   room for a monogram and a dot, and nothing else. Nothing is lost and nothing
   is invented; the items are `projectMenu.js`'s, the same ones the deleted
   `ProjectList` opened, and the only change is that the menu is the single door
   to the three of them.

   One `PointerMenu` for the whole rail rather than one per tile, the way
   `ProjectList` had one for the whole list: its panel is teleported to the body
   and only one can be open at a time, so a copy per tile would be a copy per
   tile of everything it holds. */
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import Tooltip from '../core/Tooltip.vue'
import PointerMenu from '../overlays/PointerMenu.vue'
import ProjectTile from './ProjectTile.vue'
import { useInteractive } from '../core/interactive.js'
import { projectMenuItems } from './projectMenu.js'
import { stateLabel } from './projectState.js'
import { PROJECT_RAIL } from '../../views/panelWidths.js'

const props = defineProps({
  /* `projectRows` from stores/projects.js: `{path, name, tracked}`. */
  projects: { type: Array, default: () => [] },
  activePath: { type: String, default: null },
  /* path → `{state, live, loud}`, from `projectStates` in stores/terminals.js.
     A path missing from it is idle, which is what every project reads as in a
     window that has just opened. */
  states: { type: Object, default: () => ({}) },
  /* path → branch name. Only the active project's head has been read, so this
     holds one entry in the app; the tooltip drops the empty segment itself. */
  branches: { type: Object, default: () => ({}) },
  /* The three the menu is built from. Measured for the active project alone —
     probing every project would be a command apiece for a mark nobody reads —
     which `projectMenuItems` already knows and words its items around. */
  canAddAgent: { type: Boolean, default: false },
  configured: { type: Boolean, default: false },
  configBroken: { type: Boolean, default: false }
})

const emit = defineEmits(['select', 'remove', 'add-agent', 'setup', 'add-project'])

/* 28px, the tile's own size: the add button is a place for a project standing
   in a column of projects, so it is the same box. */
const TILE = 28

const railStyle = {
  width: `${PROJECT_RAIL}px`,
  flex: '0 0 auto',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: 'var(--space-3)',
  padding: 'var(--space-4) 0',
  overflowY: 'auto',
  background: 'var(--surface-sunken)',
  borderRight: 'var(--border-w) solid var(--border)'
}

const { hover: addHover, handlers: addHandlers } = useInteractive()

const addStyle = computed(() => ({
  width: `${TILE}px`,
  height: `${TILE}px`,
  flex: '0 0 auto',
  padding: 0,
  display: 'grid',
  placeItems: 'center',
  borderRadius: 'var(--radius-3)',
  /* Dashed, and the one dashed border in the app: it is a place for a project
     rather than a project, and it stands in a column of solid tiles where the
     difference has to be legible without colour. */
  border: 'var(--border-w) dashed var(--border)',
  background: addHover.value ? 'var(--surface-hover)' : 'transparent',
  color: addHover.value ? 'var(--text-primary)' : 'var(--text-muted)',
  cursor: 'default',
  transition: 'var(--transition-control)'
}))

/* Which tile the open menu is about, by path. Cleared on close, and read while
   the menu is up so `projectMenuItems` refuses the two project-scoped verbs —
   and captions the refusal — for the tile they were opened on rather than for
   the active one. */
const menu = ref(null)
const menuFor = ref(null)

/* `ProjectList`'s number, kept, though what it now has to clear is not a row.
   The refusal moved off the two labels and into a caption above them
   (`projectMenu.js`), so the verbs are three short words each and the widest
   thing in the panel is "Switch to this project first" — and a caption is the
   one kind of row `ContextMenu` **wraps** rather than clipping, which is why
   the ceiling is left with room over it rather than trimmed to the new rows. */
const MENU_W = 260

const items = computed(() =>
  projectMenuItems({
    active: menuFor.value !== null && menuFor.value === props.activePath,
    configured: props.configured,
    configBroken: props.configBroken,
    canAddAgent: props.canAddAgent
  })
)

const openMenu = (project, event) => {
  menuFor.value = project.path
  menu.value?.open(event, project.path)
}

const pick = (item, path) => {
  if (item.kind === 'setup') emit('setup', path, item.existing)
  else if (item.kind === 'add-agent') emit('add-agent', path)
  else if (item.kind === 'remove') emit('remove', path)
}
</script>

<template>
  <div :style="railStyle">
    <ProjectTile
      v-for="p in projects"
      :key="p.path"
      :project="p"
      :active="p.path === activePath"
      :state="states[p.path]?.state ?? 'idle'"
      :state-label="stateLabel(states[p.path])"
      :branch="branches[p.path] ?? ''"
      @select="emit('select', $event)"
      @menu="openMenu"
    />
    <Tooltip label="Add project" side="right" :style="{ flex: '0 0 auto' }">
      <button type="button" :style="addStyle" v-bind="addHandlers" @click="emit('add-project')">
        <Icon name="plus" :size="14" />
      </button>
    </Tooltip>
    <PointerMenu ref="menu" :items="items" :width="MENU_W" @select="pick" @close="menuFor = null" />
  </div>
</template>
