<script setup>
/* The Agents tab: which CLI coding agent the app starts, the language it talks
   to the person in, the language it writes a task in, and what is known about
   its subscription.

   The first three are real and take effect on the next session started — the id
   travels to `terminal_create`, and Rust resolves it (`agents::resolve`); the
   two languages travel by a different road and never cross the IPC as
   arguments at all, since `terminal::service` reads them from the file itself
   when it builds the session, which is what keeps a person's session and a
   run's batch from disagreeing about them. The
   second half is a placeholder with nothing behind it, and it says so in as many
   words: a block of invented numbers under a real setting would claim the app
   knows something it does not, which is the same mistake the fixture log pane in
   the right column was removed for. */
import { computed } from 'vue'
import Dropdown from '../core/Dropdown.vue'
import SettingsRow from './SettingsRow.vue'

const props = defineProps({
  agent: { type: String, default: 'claude' },
  /* The language the agent talks to the person in, and the language the prose
     of a bd issue it writes is in. BCP-47 ids, validated in Rust against
     `agents::LANGUAGES`; `en` here mirrors that table's default. */
  agentLanguage: { type: String, default: 'en' },
  taskLanguage: { type: String, default: 'en' }
})

const emit = defineEmits(['update:agent', 'update:agentLanguage', 'update:taskLanguage'])

/* The one place in the front end that names an agent. The ids are `agents::IDS`
   in `src-tauri/src/agents/mod.rs`, which is where the truth lives — Rust
   validates `agent` against that list on the way to the file and drops anything
   else. So this list is a set of labels for ids Rust already knows, and an id
   added there and not here is simply not offered, while one added here and not
   there is picked, dropped on save and back to Claude Code after a restart.

   Codex is offered and not selectable, and the restriction is this row and
   nothing else: Rust still knows `codex`, `agents/codex.rs` is complete, and a
   `settings.json` that already holds it goes on starting Codex sessions. That
   is accepted rather than overlooked — the working code is not worth breaking
   for a temporary limit, and lifting the limit is deleting the two fields
   below. It is shown rather than dropped from the list because a person should
   be able to see that the app knows the agent and has not switched it on yet;
   a list of one says nothing at all. */
const AGENTS = [
  { value: 'claude', label: 'Claude Code' },
  { value: 'codex', label: 'Codex', disabled: true, note: 'Not supported yet' }
]

/* The same doubling one row down, accepted for the same reason. The ids are
   `agents::LANGUAGES` in `src-tauri/src/agents/mod.rs`, which carries the
   English name beside each because that name is what goes into the prompt —
   these are labels for ids Rust already knows and validates, so drift costs a
   stale label rather than a lost setting. The order is the table's, and the
   table's order is not alphabetical either: it is roughly how many people write
   in each, with English and Russian first because they are the two this was
   built for. */
const LANGUAGES = [
  { value: 'en', label: 'English' },
  { value: 'ru', label: 'Russian' },
  { value: 'zh-Hans', label: 'Chinese (Simplified)' },
  { value: 'es', label: 'Spanish' },
  { value: 'hi', label: 'Hindi' },
  { value: 'pt', label: 'Portuguese' },
  { value: 'fr', label: 'French' },
  { value: 'de', label: 'German' },
  { value: 'ja', label: 'Japanese' },
  { value: 'ko', label: 'Korean' },
  { value: 'it', label: 'Italian' },
  { value: 'tr', label: 'Turkish' }
]

/* Every row on this tab shares one control column, wider than the shipped
   default: "Chinese (Simplified)" is the longest label either list holds, and
   `Dropdown` ellipsises a label that does not fit its field rather than growing
   it. In `ch` like the rest of this window, so the column grows with the
   app-wide font size instead of clipping at the top of the range. */
const CONTROL_WIDTH = '30ch'

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
      :control-width="CONTROL_WIDTH"
    >
      <Dropdown
        :model-value="props.agent"
        :options="AGENTS"
        @update:model-value="emit('update:agent', $event)"
      />
    </SettingsRow>

    <SettingsRow
      label="Conversation language"
      description="What an agent says to you is written in this. It reaches the next session started, not the ones already running."
      :control-width="CONTROL_WIDTH"
    >
      <Dropdown
        :model-value="props.agentLanguage"
        :options="LANGUAGES"
        @update:model-value="emit('update:agentLanguage', $event)"
      />
    </SettingsRow>

    <SettingsRow
      label="Task language"
      description="What an agent writes into a task is written in this — the title, the description, the criteria. Section headings stay English, and so do specifications and plans."
      :control-width="CONTROL_WIDTH"
    >
      <Dropdown
        :model-value="props.taskLanguage"
        :options="LANGUAGES"
        @update:model-value="emit('update:taskLanguage', $event)"
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
