<script setup>
/* The five rows of the Models group — Default, Tasks, Code, Run lead, Branch
   review — out of `AgentSettings.vue` and into a component of their own,
   because a second caller wants exactly this shape: the Project settings
   dialog's own Agents group (`ProjectSettingsModal.vue`,
   `.claude/rules/runs.md`) draws the identical five rows over a project's own
   table rather than the root's. Two copies of ten dropdowns and their
   inheritance rule would drift the first time one of them changed, which is
   the whole reason every rule in this tree is pulled out of the component
   that draws it.

   This component draws the rows and nothing around them — no caption, no
   group, no "Codex models could not be refreshed" line. Those stay with
   whichever caller wraps it, since a settings-window tab and a project
   dialog's group want different headings over the identical rows.

   `agent`, `model` and `agentRoles` are a table in the shape
   `components/settings/agentRoles.js`'s `pairOf` already reads — the root's
   own three fields, or a project's own whole block, and the caller decides
   which. `disabled` greys every dropdown at once, for the one caller that
   needs it: the project dialog's rows hold still while a save is in flight,
   the way every other field in that dialog does. */
import { computed } from 'vue'
import Dropdown from '../core/Dropdown.vue'
import SettingsRow from './SettingsRow.vue'
import {
  chooseModel,
  chooseProvider,
  modelOptions,
  pairOf,
  providerOptions,
  ROLE_ROWS,
  unavailableModelOption
} from './agentRoles.js'
import { agents } from '../../stores/agents.js'

const props = defineProps({
  agent: { type: String, default: 'claude' },
  model: { type: String, default: '' },
  agentRoles: {
    type: Object,
    default: () => ({
      tasks: { agent: '', model: '' },
      code: { agent: '', model: '' },
      runLead: { agent: '', model: '' },
      reviewBranch: { agent: '', model: '' }
    })
  },
  disabled: { type: Boolean, default: false }
})

const emit = defineEmits([
  /* One event for all ten dropdowns, carrying `{ role, pair }` — the role's
     key, or `null` for the Default row, and the whole pair. See
     `AgentSettings.vue`'s own header, unchanged by the move, for why this is
     one event rather than a pair per row. */
  'update:agentRole'
])

/* The five rows, each already knowing which pair it stands for and what its
   two lists hold. Computed rather than worked out in the template: the model
   list depends on the harness the row resolves to, which is the root's for a
   row that has chosen none, and a template expression repeating that would be
   the place the two halves come apart. */
const modelRows = computed(() =>
  ROLE_ROWS.map((row) => {
    const pair = pairOf(row.role, props.agentRoles, props.agent, props.model)
    return {
      ...row,
      pair,
      providers: providerOptions(row.role, agents.value),
      models: unavailableModelOption(modelOptions(row.role, agents.value, pair.agent, pair.inherited), pair.model)
    }
  })
)

/* A row of this group asks for two fields side by side, so it asks for twice
   the column a single-field row takes plus the gap between them. In `ch` for
   the reason `AgentSettings.vue`'s own `CONTROL_WIDTH` is: the column grows
   with the app-wide font size instead of clipping at the top of the range. */
const PAIR_WIDTH = '38ch'
const pairStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  width: '100%'
}
/* Each half takes half of whatever the row got, and neither refuses to
   shrink: `GPT-5.6-Terra` and `Same as default` are the longest labels either
   list holds, and `Dropdown` ellipsises a label that does not fit rather than
   growing its field. */
const halfStyle = { flex: '1 1 0', minWidth: 0 }
</script>

<template>
  <SettingsRow
    v-for="row in modelRows"
    :key="row.label"
    :label="row.label"
    :description="row.description"
    :control-width="PAIR_WIDTH"
  >
    <div :style="pairStyle">
      <div :style="halfStyle">
        <Dropdown
          :model-value="row.pair.inherited ? '' : row.pair.agent"
          :options="row.providers"
          :disabled="props.disabled"
          @update:model-value="emit('update:agentRole', chooseProvider(row.role, $event))"
        />
      </div>
      <div :style="halfStyle">
        <Dropdown
          :model-value="row.pair.model"
          :options="row.models"
          :disabled="props.disabled"
          @update:model-value="
            emit('update:agentRole', chooseModel(row.role, $event, props.agentRoles, props.agent))
          "
        />
      </div>
    </div>
  </SettingsRow>
</template>
