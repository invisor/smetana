<script setup>
import { computed } from 'vue'
import FileTreeRow from './FileTreeRow.vue'

const props = defineProps({
  nodes: { type: Array, default: () => [] },
  selectedPath: { type: String, default: undefined },
  expanded: { type: Object, default: () => ({}) }
})

defineEmits(['toggle', 'select'])

/* Flattened to a single list so the tree can be virtualised later without
   restructuring the markup. */
const rows = computed(() => {
  const out = []
  const walk = (list, depth) => {
    for (const n of list) {
      const open = !!props.expanded[n.path]
      out.push({
        path: n.path,
        name: n.name,
        depth,
        kind: n.kind || 'file',
        expanded: open,
        selected: n.path === props.selectedPath,
        git: n.git,
        readOnly: !!n.readOnly
      })
      if (n.kind === 'dir' && open && n.children) walk(n.children, depth + 1)
    }
  }
  walk(props.nodes, 0)
  return out
})
</script>

<template>
  <div role="tree" :style="{ display: 'flex', flexDirection: 'column', color: 'var(--text-primary)' }">
    <FileTreeRow
      v-for="r in rows"
      :key="r.path"
      v-bind="r"
      @toggle="$emit('toggle', r.path)"
      @select="$emit('select', r.path)"
    />
  </div>
</template>
