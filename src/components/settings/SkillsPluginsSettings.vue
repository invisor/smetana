<script setup>
/* The Skills & Plugins tab: what is **installed** on this machine around the
   agent, rather than how the agent talks. That is the whole of the line between
   this tab and Agents next to it — Agents is the harness, the languages, the
   standing instruction and what is left of a subscription, all of them about
   what an agent says and what it spends; this one is about somebody else's
   software sitting on the machine, what state it is in and what would be typed
   to change that. Caveman is the first of those and today the only one, which
   is why the tab holds one group.

   It sits immediately after Agents rather than at the end, because the two are
   read together and because the line before Storage — the tabs that are
   settings, and the one that is not — is not this tab's to cross.

   The label keeps its capital P. Sentence case is this product's rule
   everywhere, so Skills & plugins was the form the rule asks for; the capital
   was chosen deliberately over it and is not a slip to be corrected here.

   The group draws one line about how caveman stands on this machine and up to
   two rows under it, and every word of the first is `caveman.js`'s — the `.vue`
   file is the one thing no test here can reach, so the whole of that rule lives
   outside it. Nothing on this tab is a setting except the level: the state is
   `caveman_state`'s reading, asked afresh every time the tab is opened, since
   the machine's own files are the truth and a copy of ours would disagree with
   the disk the first time somebody ran `caveman enable` outside this app.

   The level here is the one for **all projects**, and it is the only one this
   window has. A project's own override is a row in the project settings window,
   off the project tile's right-click menu: this window is about the machine and
   has no project of its own, so a per-project row here could only ever mean
   "whichever project the app window happens to have open". The field on disk did
   not move — `project.caveman` in `settings.json` — only the control did, to
   `components/run/ProjectSettingsModal.vue`.

   The Install button installs nothing, and that is the design rather than a
   limitation. It opens a terminal in the active project and **types** the
   command without a newline: the person reads it, presses Enter themselves, and
   watches somebody else's installer rewrite their own `~/.claude/settings.json`
   with their own eyes. This component only says which command; the terminal is
   the app window's to open (`stores/app.js`, `typeCavemanInstall` in
   `views/DesktopApp.vue`), which is why the press leaves here as an event like
   every other choice on this screen. With no project open there is nothing to
   open a terminal in, so the button is drawn `disabled` and the row's
   description names the reason — the Launch at login shape on the General tab.

   Presentational like every other tab in this directory: the window does the
   asking and this renders in `?view=gallery` with nothing behind it. */
import { computed } from 'vue'
import Button from '../core/Button.vue'
import Dropdown from '../core/Dropdown.vue'
import SettingsGroup from './SettingsGroup.vue'
import SettingsRow from './SettingsRow.vue'
import {
  installCommand,
  installDescription,
  levelOptions,
  offersInstall,
  stateFacts,
  stateSentence
} from './caveman.js'

const props = defineProps({
  /* `caveman_state`'s answer whole, in Rust's own shape (`src-tauri/src/
     caveman.rs`), or `null` before there has been one. Not a setting and
     nothing about it reaches `settings.json`: the machine's own files are the
     truth, so the window asks afresh every time this tab is opened. */
  caveman: { type: Object, default: null },
  /* How compressed an agent's answers are, everywhere. The shipped `off`, which
     is the fourth copy of it — the other three are `CavemanSettings::default()`
     in Rust, `defaults()` in `stores/settings.js` and `view` in
     `SettingsWindow.vue` — and they have to agree, or this tab draws a level
     the app is not using for the moment before the first answer arrives. A
     project's own override is not this window's; see the header. */
  cavemanLevel: { type: String, default: 'off' },
  /* Whether the app window has a project open, which is the whole of what the
     Install button needs to know: the command is typed into a terminal, and a
     terminal is opened somewhere. `false` by default, so the button opens
     `disabled` and settles into being live — the other way round it would offer
     a press for the length of a round trip and then take it away, which is the
     shape `autostartSupported` on the General tab already refuses. */
  projectOpen: { type: Boolean, default: false }
})

const emit = defineEmits([
  'update:cavemanLevel',
  /* The Install press, carrying nothing: what would be typed is this
     component's to say and the window's to send on, and the terminal is the app
     window's to open. */
  'install'
])

/* The one control column this tab's rows share, the same measure the Agents tab
   next door uses so the two read as one window rather than as two layouts. In
   `ch` like the rest of this window, so the column grows with the app-wide font
   size instead of clipping at the top of the range. */
const CONTROL_WIDTH = '30ch'

/* The ladder, built once: the list does not change, and it lives in
   `caveman.js` for the reason every rule in this directory does. */
const cavemanLevels = levelOptions()

/* What this machine's four files came to, in one sentence and up to a handful
   of facts. The facts are the journal's, and the group draws them only where
   they are true right now — `caveman.js` holds that rule, not this file. */
const cavemanSentence = computed(() => stateSentence(props.caveman))
const cavemanFacts = computed(() => stateFacts(props.caveman))
const cavemanCommand = computed(() => installCommand(props.caveman))
const cavemanInstallable = computed(() => offersInstall(props.caveman))
const cavemanInstallDescription = computed(() =>
  installDescription(props.caveman, props.projectOpen)
)

/* The line about this machine, drawn where a row would be and with a row's own
   rule under it, so the group reads as one stack rather than as a paragraph
   that wandered into it. It is deliberately not a `SettingsRow`: there is
   nothing to change on this line, and a row with an empty control column reads
   as a control that failed to draw. */
const cavemanStatusStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  padding: 'var(--space-4) 0',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const cavemanSentenceStyle = {
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
/* This and the two under it stand here and again in `AgentSettings.vue`, where
   the subscription block draws the same name-and-value shape. The copy is
   deliberate: a few lines of style literal are not a rule, and a module for
   them would be a home for something neither file could then read in place. */
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
/* The value beside it, said of something long. A path and a command are
   identifiers, so they are mono and muted like every other identifier in this
   window; what is added is the pair of properties that keep a home directory or
   three chained commands inside the panel — `anywhere` lets the line break
   where it must, and a `minWidth` of 0 is what lets a flex item narrower than
   its content do so at all. */
const cavemanValueStyle = {
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-mono)',
  minWidth: 0,
  overflowWrap: 'anywhere'
}
</script>

<template>
  <div>
    <SettingsGroup label="Caveman">
      <!-- What the machine's own four files came to. Every word of it is
           `caveman.js`'s, including the line for a state this build has never
           heard of and the one for a reading that has not arrived. -->
      <div :style="cavemanStatusStyle">
        <span :style="cavemanSentenceStyle">{{ cavemanSentence }}</span>
        <!-- The pack version, what it was applied to, and every file caveman
             rewrote. That last list is not decoration: another installer edited
             two of this person's own configuration files, and this is the only
             screen that says which. -->
        <div v-for="(fact, index) in cavemanFacts" :key="index" :style="factStyle">
          <span :style="nameStyle">{{ fact.name }}</span>
          <span :style="cavemanValueStyle">{{ fact.value }}</span>
        </div>
        <!-- Drawn beside the button rather than only inside the terminal, so it
             can be read before it is typed — and so somebody with no project
             open, whose button is dead, can still see what to run themselves. -->
        <div v-if="cavemanCommand" :style="factStyle">
          <span :style="nameStyle">Command</span>
          <span :style="cavemanValueStyle">{{ cavemanCommand }}</span>
        </div>
      </div>

      <!-- Only where there is something to install or to wire in. In the other
           two states the press would do nothing, and a button that does nothing
           is worse than none. -->
      <SettingsRow
        v-if="cavemanInstallable"
        label="Install"
        :description="cavemanInstallDescription"
        :control-width="CONTROL_WIDTH"
      >
        <Button
          variant="secondary"
          icon="terminal"
          :disabled="!props.projectOpen"
          @click="emit('install')"
        >
          Install
        </Button>
      </SettingsRow>

      <!-- The level for every project, and the only one this window draws. A
           project wanting its own says so in its own window, off the project
           tile's right-click menu — the header carries why the pair no longer
           stands side by side. -->
      <SettingsRow
        label="All projects"
        description="How compressed an agent's answers are, everywhere. Off is what this app has always said about caveman: nothing at all. A project can take a level of its own in its own settings window."
        :control-width="CONTROL_WIDTH"
      >
        <Dropdown
          :model-value="props.cavemanLevel"
          :options="cavemanLevels"
          @update:model-value="emit('update:cavemanLevel', $event)"
        />
      </SettingsRow>
    </SettingsGroup>
  </div>
</template>
