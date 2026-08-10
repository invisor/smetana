<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import Tooltip from '../core/Tooltip.vue'

const props = defineProps({
  blockedBy: { type: Number, default: 0 },
  blocks: { type: Number, default: 0 },
  /* Which tasks those are, when the caller knows. The chip still draws the
     count — these only name the tasks in the hint, because "1 task blocks this
     one" is precisely what somebody looking at a blocked card already knows.
     Optional, so a caller holding only a number (the gallery, a fixture board)
     keeps working and gets the count phrased instead. */
  blockedByIds: { type: Array, default: () => [] },
  blockingIds: { type: Array, default: () => [] },
  spawnedFrom: { type: String, default: undefined },
  size: { type: String, default: 'md' }
})

/* Ids rather than titles, and one line rather than a list. `Tooltip` takes a
   string and renders it inline, so there are no lines to put them on; and the
   id is the name this app uses for an issue everywhere else — the card's own
   header draws it, and it is what a person types into bd to go and look. Three
   titles in a hint is a paragraph. */
const named = (ids) => ids.join(', ')

const blockedLabel = computed(() =>
  props.blockedByIds.length
    ? `Blocked by ${named(props.blockedByIds)}`
    : `${props.blockedBy} ${props.blockedBy === 1 ? 'task blocks' : 'tasks block'} this one`
)

const blocksLabel = computed(() =>
  props.blockingIds.length
    ? `Blocks ${named(props.blockingIds)}`
    : `Blocks ${props.blocks} downstream ${props.blocks === 1 ? 'task' : 'tasks'}`
)

const sm = computed(() => props.size === 'sm')
const empty = computed(() => props.blockedBy <= 0 && props.blocks <= 0 && !props.spawnedFrom)

const chipStyle = (tone) => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '2px',
  color: tone,
  font: `var(--weight-medium) ${sm.value ? 'var(--text-2xs)' : 'var(--text-xs)'}/1 var(--font-mono)`
})

const spawnStyle = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '2px',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)'
}))

const wrapStyle = { display: 'inline-flex', alignItems: 'center', gap: 'var(--space-4)' }
const glyphSize = computed(() => (sm.value ? 10 : 11))
</script>

<template>
  <span v-if="!empty" :style="wrapStyle">
    <Tooltip v-if="blockedBy > 0" :label="blockedLabel">
      <span :style="chipStyle('var(--status-blocked-fg)')">
        <Icon name="lock" :size="glyphSize" :stroke-width="2" />{{ blockedBy }}
      </span>
    </Tooltip>
    <Tooltip v-if="blocks > 0" :label="blocksLabel">
      <span :style="chipStyle('var(--text-secondary)')">
        <Icon name="git-fork" :size="glyphSize" :stroke-width="2" />{{ blocks }}
      </span>
    </Tooltip>
    <Tooltip v-if="spawnedFrom" :label="`spawned from ${spawnedFrom}`">
      <span :style="spawnStyle">
        <Icon name="corner-down-right" :size="10" :stroke-width="2" />{{ spawnedFrom }}
      </span>
    </Tooltip>
  </span>
</template>
