<script setup>
/* The Agents tab: which CLI coding agent the app starts, the languages it works
   in, and what is left of its subscription.

   The picker and the languages under it are real and take effect on the next
   session started — the id travels to `terminal_create`, and Rust resolves it
   (`agents::resolve`); the languages travel by a different road and never cross
   the IPC as arguments at all, since `terminal::service` reads them from the
   file itself when it builds the session, which is what keeps a person's
   session and a run's batch from disagreeing about them. The commit language
   has a second reader beside a session: `vcs_suggest_message` reads the same
   field for the Git panel's "suggest a message" button, so the two cannot
   disagree either.

   The languages sit inside one `SettingsGroup` and the Agent row above stays
   outside it — the shape the General tab already draws, and there is no second
   group over that one row: a caption over a single row is a caption for its own
   sake.

   Report language is the one row here with a condition on it, and the condition
   belongs to another tab: with **Show run report** off on General, the row is
   drawn `disabled` and its description says so. Disabled rather than absent, for
   the answer `GeneralSettings.vue` already carries in its Launch at login row —
   the one in the Startup group, drawn disabled with `autostartDescription`
   naming the reason — that a control which refuses a press without saying why is
   worse than one that is not there. A row vanishing from this tab because of a
   switch on another is the second half of it: a change nobody sees happen. The
   stored value is untouched while the row is disabled, so turning the switch
   back on brings the choice back rather than `en`.

   The block under them was three dashes and a sentence saying nothing was read
   here yet, which was honest and is no longer necessary: `agent_usage` asks the
   harness the same question the run gate asks before every batch. It is still
   presentational, like every component on this tab — the window does the
   asking, so this renders in `?view=gallery` with nothing behind it — and every
   sentence in it belongs to `usage.js`, since a `.vue` file is the one thing no
   test here can reach.

   Plan and Status are gone rather than kept as dashes: `/usage` reports two
   percentages and two reset times and says nothing at all about a tariff, so
   those two rows could only ever have stayed empty. */
import { computed } from 'vue'
import Button from '../core/Button.vue'
import Dropdown from '../core/Dropdown.vue'
import SettingsGroup from './SettingsGroup.vue'
import SettingsRow from './SettingsRow.vue'
import { agentOf, offersRefresh, usageLines, usageNote } from './usage.js'

const props = defineProps({
  agent: { type: String, default: 'claude' },
  /* The language the agent talks to the person in, the language the prose of a
     bd issue it writes is in, the language a git commit message it writes is
     in, and the language the prose of a run's report is in. BCP-47 ids,
     validated in Rust against `agents::LANGUAGES`; `en` here mirrors that
     table's default, and for the commit language that default is today's
     behaviour to the letter. */
  agentLanguage: { type: String, default: 'en' },
  taskLanguage: { type: String, default: 'en' },
  commitLanguage: { type: String, default: 'en' },
  reportLanguage: { type: String, default: 'en' },
  /* Whether **Show run report** is on, which lives on the General tab and is
     `view.notificationShowReport` in `SettingsWindow.vue`. Read and never
     emitted back: this tab does not own it, it only draws the Report language
     row disabled while it is off — hence the name here drops the section prefix
     rather than the field, the way `GitSettings.vue` takes `gitAutoFetch` as
     `autoFetch`, so that one grep still finds both ends of the pair. Default
     `true`, the shipped position, so the row is usable in the moment before the
     first answer arrives rather than greying itself out and back. */
  showReport: { type: Boolean, default: true },
  /* `agent_usage`'s answer whole, in Rust's own shape, or `null` before there
     has been one. */
  usage: { type: Object, default: null },
  /* A probe is out. It holds the button, since a second press would start a
     second minute-long process against the same harness. */
  busy: { type: Boolean, default: false },
  /* What the last read was refused with, in one readable line. Not the same
     thing as an allowance that could not be read — that is an answer and has
     its own sentence; this is the channel failing. */
  error: { type: String, default: null }
})

const emit = defineEmits([
  'update:agent',
  'update:agentLanguage',
  'update:taskLanguage',
  'update:commitLanguage',
  'update:reportLanguage',
  'refresh'
])

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
   default: "Chinese (Simplified)" is the longest label any of the lists holds,
   and `Dropdown` ellipsises a label that does not fit its field rather than
   growing it. In `ch` like the rest of this window, so the column grows with
   the app-wide font size instead of clipping at the top of the range. */
const CONTROL_WIDTH = '30ch'

/* What the block below is headed, and it names **whoever answered the probe**
   rather than whoever is showing in the picker above. The two can differ:
   `agents::pick` substitutes the first installed profile for a configured one
   that is not on `PATH`, so a heading taken from the dropdown could say Claude
   Code over Codex's allowance. An id nobody ships (a hand-edited file) is named
   as it stands rather than dressed up as one of ours.

   With nobody to name — nothing read yet, or no agent installed at all — the
   heading is the bare word. Borrowing the selected agent's label for that
   moment would be the app claiming a reading it has not got. */
const heading = computed(() => {
  const id = agentOf(props.usage)
  if (!id) return 'Subscription'
  return `${AGENTS.find((agent) => agent.value === id)?.label ?? id} subscription`
})

/* One line under the label, and which one depends on whether the report is ever
   put in front of anybody. The shape `autostartDescription` in
   `GeneralSettings.vue` already has, and no pure module of its own: `usage.js`
   exists because its sentences are a rule with cases, and this is one boolean
   and two strings.

   The Off sentence names the reason and the tab it is on, and it is careful
   about what it does not claim: the document is still written, since
   `runs::service::finish` renders it whatever the switch says and a lead writes
   its batch file either way. What the switch stops is the delivery, so the
   sentence says reports are not shown — not that none is written. */
const reportDescription = computed(() =>
  props.showReport
    ? 'What a run writes about itself is written in this — what each task did, and any note on the batch. Field names, paths and identifiers stay as they are, and so do the report\'s own labels.'
    : 'Reports are not shown, so there is nothing here to show you. Turn on Show run report on the General tab to choose a language.'
)

const lines = computed(() => usageLines(props.usage))
const note = computed(() => usageNote(props.usage, props.busy, props.error))
const refreshable = computed(() => offersRefresh(props.usage))

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
/* The heading and the button share a line: the button is about this block and
   nothing else on the tab, and a row of its own under the rows it refreshes
   would read as an action on the whole screen. */
const headerStyle = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 'var(--space-4)'
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
const errorStyle = {
  color: 'var(--status-failed-fg)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
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

    <!-- The rows that answer the same question about different writing. The
         Agent row above is outside the group deliberately: a second group over
         one row would be a caption for its own sake, and the General tab does
         not do that either. -->
    <SettingsGroup label="Languages">
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

      <SettingsRow
        label="Commit language"
        description="What an agent writes in a git commit message is written in this — both the suggested message in the Git panel and the commits it makes during a run. What sits in front of the colon is left as it is."
        :control-width="CONTROL_WIDTH"
      >
        <Dropdown
          :model-value="props.commitLanguage"
          :options="LANGUAGES"
          @update:model-value="emit('update:commitLanguage', $event)"
        />
      </SettingsRow>

      <!-- Last in the group, and the only row here whose control can be shut:
           its description carries the reason, which is the whole point of
           drawing it rather than removing it. -->
      <SettingsRow
        label="Report language"
        :description="reportDescription"
        :control-width="CONTROL_WIDTH"
      >
        <Dropdown
          :model-value="props.reportLanguage"
          :options="LANGUAGES"
          :disabled="!props.showReport"
          @update:model-value="emit('update:reportLanguage', $event)"
        />
      </SettingsRow>
    </SettingsGroup>

    <div :style="blockStyle">
      <div :style="headerStyle">
        <span :style="headingStyle">{{ heading }}</span>
        <Button
          v-if="refreshable"
          size="sm"
          variant="ghost"
          icon="refresh-cw"
          :disabled="props.busy"
          @click="emit('refresh')"
        >
          Refresh
        </Button>
      </div>
      <div v-for="line in lines" :key="line.name" :style="factStyle">
        <span :style="nameStyle">{{ line.name }}</span>
        <span :style="valueStyle">{{ line.value }}</span>
      </div>
      <span v-if="note" :style="noteStyle">{{ note }}</span>
      <span v-if="props.error" :style="errorStyle">{{ props.error }}</span>
    </div>
  </div>
</template>
