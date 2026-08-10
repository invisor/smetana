<script setup>
/* The Agents tab: which CLI coding agent the app starts, and what is known
   about its subscription.

   The first half is real and takes effect on the next session started — the id
   travels to `terminal_create`, and Rust resolves it (`agents::resolve`). The
   second half is a placeholder with nothing behind it, and it says so in as many
   words: a block of invented numbers under a real setting would claim the app
   knows something it does not, which is the same mistake the fixture log pane in
   the right column was removed for. */
import { computed } from 'vue'
import Dropdown from '../core/Dropdown.vue'
import SettingsRow from './SettingsRow.vue'

const props = defineProps({
  agent: { type: String, default: 'claude' }
})

const emit = defineEmits(['update:agent'])

/* The one place in the front end that names an agent. The ids are `agents::IDS`
   in `src-tauri/src/agents/mod.rs`, which is where the truth lives — Rust
   validates `agent` against that list on the way to the file and drops anything
   else. So this list is a set of labels for ids Rust already knows, and an id
   added there and not here is simply not offered, while one added here and not
   there is picked, dropped on save and back to Claude Code after a restart. */
const AGENTS = [
  { value: 'claude', label: 'Claude Code' },
  { value: 'codex', label: 'Codex' }
]

/* The agent whose subscription the block below is about — by its own label,
   because "your subscription" beside a picker showing something else reads as
   the app having switched already. An id nobody ships (a hand-edited file) is
   named as it stands rather than dressed up as one of ours. */
const agentLabel = computed(
  () => AGENTS.find((agent) => agent.value === props.agent)?.label ?? props.agent
)

const blockStyle = {
  marginTop: 'var(--space-5)',
  padding: 'var(--space-4)',
  background: 'var(--surface-sunken)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)',
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)'
}
const headingStyle = {
  color: 'var(--text-primary)',
  font: 'var(--weight-medium) var(--text-ui-size)/var(--leading-snug) var(--font-sans)'
}
const factStyle = { display: 'flex', alignItems: 'baseline', gap: 'var(--space-4)' }
/* A `ch` measure, not pixels: this is a column of words, and it is the first
   thing that would clip when the app-wide font size grows. `ch` is the width of
   a "0" in the font actually in use, so the column grows with the text inside
   it — the same reasoning as the prose width on the About tab. */
const nameStyle = {
  flex: '0 0 13ch',
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
const valueStyle = {
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-mono)'
}
const noteStyle = {
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}

/* Placeholder rows, not readings. The dash is the same "there is nothing to
   show" the scope bar draws for a missing branch. */
const FACTS = [
  { name: 'Plan', value: '—' },
  { name: 'Status', value: '—' },
  { name: 'Allowance', value: '—' }
]
</script>

<template>
  <div>
    <SettingsRow
      label="Agent"
      description="Which CLI agent new sessions start. Sessions already running keep the one they started with."
    >
      <Dropdown
        :model-value="props.agent"
        :options="AGENTS"
        @update:model-value="emit('update:agent', $event)"
      />
    </SettingsRow>

    <div :style="blockStyle">
      <span :style="headingStyle">{{ agentLabel }} subscription</span>
      <div v-for="fact in FACTS" :key="fact.name" :style="factStyle">
        <span :style="nameStyle">{{ fact.name }}</span>
        <span :style="valueStyle">{{ fact.value }}</span>
      </div>
      <span :style="noteStyle">
        Nothing is read from the agent here yet. A run asks it what is left of the allowance before
        each batch; this block will show the same answer once it is wired to it.
      </span>
    </div>
  </div>
</template>
