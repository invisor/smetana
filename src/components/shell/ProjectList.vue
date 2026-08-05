<script setup>
/* The projects this window is working with: one is active, the rest are one
   click away. A folder with no bd tracker still belongs here — it is a
   project you added, and the mark says what it is missing, quietly.

   Two rows can carry a warning triangle at once, and they are told apart by
   what they sit next to rather than by colour alone: the missing tracker is a
   lone muted glyph beside the name, with nothing on the row that fixes it,
   while the missing run configuration is a red glyph bonded to the gear that
   opens the setup it is asking for.

   No header of its own: the enclosing Panel already shows "Projects" and
   carries the "+" in its actions slot, so a second copy here would print the
   same word twice in a row. */
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Tooltip from '../core/Tooltip.vue'

const props = defineProps({
  projects: { type: Array, default: () => [] },
  activePath: { type: String, default: null },
  /* Starting an agent is an action on a project, so it is offered on the
     project's own row — and only on the active one: a session belongs to the
     directory the window is pointed at, and the list below shows that
     project's sessions only. Off by default, since the row means the same
     thing whether or not anything can be started from it. */
  canAddAgent: { type: Boolean, default: false },
  /* Only ever about the active row, for the same reason canAddAgent is: the
     configuration is read on switching projects, and probing every row would
     be a command per project for a mark nobody is looking at. */
  needsSetup: { type: Boolean, default: false }
})

const emit = defineEmits(['select', 'remove', 'add-agent', 'setup'])

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

/* Neither mark names a side, deliberately: this list scrolls, and a tooltip
   that had to open inside it was cut off by its edges whichever way it went.
   Tooltip now teleports its panel out of the document flow and chooses a side
   against the window, so the right answer here is to ask for nothing and let
   it decide — a row near the top of the panel gets its hint below, everywhere
   else above. */
const setupMarkStyle = { display: 'inline-flex', alignItems: 'center', gap: 'var(--space-2)' }

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
        <!-- The empty title is not decoration: `title` is inherited, and the
             row carries the project's full path in one. Hovering a glyph that
             explains itself would otherwise draw that path as well, in a
             second panel the app does not control the placement of. An empty
             title is the standard way to say "nothing to advise here" and
             stops the lookup before it reaches the row. -->
        <Tooltip v-if="!p.tracked" label="No bd tracker here" title="">
          <Icon name="triangle-alert" :size="12" :style="{ color: 'var(--text-muted)' }" />
        </Tooltip>
        <!-- The mark and the button that clears it are one hover target, tied
             together by a gap narrower than the row's own: the triangle is what
             a person sees without touching anything, the gear is what they
             press. Red is the loudest colour the system has, and it is only
             ever spent once here — the mark is drawn for the active project
             alone, the same reason the gear is. -->
        <!-- The tooltip wraps the glyph alone, not the pair. A panel centred
             over both would hang off to one side of whichever of them the
             pointer is actually on, and the two have different things to say:
             the mark reports the state, the button offers the action, and the
             button already carries its own label. -->
        <span v-if="needsSetup && p.path === activePath" :style="setupMarkStyle">
          <Tooltip label="Not set up for runs" title="">
            <Icon name="triangle-alert" :size="12" :style="{ color: 'var(--status-failed-fg)' }" />
          </Tooltip>
          <IconButton icon="settings-2" label="Set up for runs" size="sm" @click.stop="emit('setup', p.path)" />
        </span>
        <!-- Before the remove button, not after it: removal keeps the row's
             last position wherever it appears, so a click aimed at it never
             lands on something that moved in. -->
        <IconButton
          v-if="canAddAgent && p.path === activePath"
          icon="plus"
          label="New agent"
          size="sm"
          @click.stop="emit('add-agent', p.path)"
        />
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
