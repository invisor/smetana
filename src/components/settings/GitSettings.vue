<script setup>
/* The Git tab: everything the app does to a person's repositories without
   asking each time. Two switches, and they are not the same kind of act — one
   opens a socket, the other deletes a directory — which is what makes a tab of
   their own worth having over a pair of rows filed under General.

   Neither is a fact about one repository, which is why both live at the root of
   `settings.json` rather than under a project: a metered connection and a habit
   about finished work are facts about a person and a machine.

   Presentational, like every component here: it is handed the values and emits
   what a person picked. The window is what talks to the main window and to the
   file, so this renders in `?view=gallery` with nothing behind it. */
import Switch from '../core/Switch.vue'
import SettingsRow from './SettingsRow.vue'

/* Both default on, mirroring `GitSettings::default()` in Rust,
   `defaults().git` in `stores/settings.js` and `view` in `SettingsWindow.vue` —
   a component that defaulted the other way would draw the opposite of what the
   app is doing for the moment before the first value arrives. */
const props = defineProps({
  autoFetch: { type: Boolean, default: true },
  removeWorktrees: { type: Boolean, default: true }
})

const emit = defineEmits(['update:autoFetch', 'update:removeWorktrees'])
</script>

<template>
  <div>
    <!-- Off is for a metered connection, a VPN that is not always up, or a key
         with a passphrase that would fail on every sweep. The sentence says
         what it does not do as well: nothing about this changes a file. -->
    <SettingsRow
      label="Fetch from remotes automatically"
      description="Checks the Git panel's branches for new commits when the window comes back into focus, at most every few minutes. Nothing is merged."
    >
      <Switch
        :model-value="props.autoFetch"
        @update:model-value="emit('update:autoFetch', $event)"
      />
    </SettingsRow>
    <!-- The app itself never runs `git worktree`: this switch is a line in the
         run's prompt, and the agent leading the run is what acts on it. The
         description names the cost of turning it off, since nothing here
         counts what is on the disk or can tell anybody later. -->
    <SettingsRow
      label="Remove a task's worktree after it merges"
      description="Off keeps every merged task's worktree on disk until you remove it by hand."
    >
      <Switch
        :model-value="props.removeWorktrees"
        @update:model-value="emit('update:removeWorktrees', $event)"
      />
    </SettingsRow>
  </div>
</template>
