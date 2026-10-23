<script setup>
/* One thing the agent did: the tool's name, the one line of what it was doing,
   and how it ended.

   Exported from `src/components/index.js` as **`ConversationToolCall`**, the
   one export in that file whose name is not its file's: `agent/ToolCall.vue`
   holds the short one, and the reason the two keep separate names is written
   out beside the export rather than repeated here.

   One row and one line. The detail is `agents::claude::tool_detail`'s single
   line — a command, a path, a pattern — and it is clipped here rather than
   wrapped, because a journal of tool calls is read by scanning down the names.
   `PermissionRequest.vue` is the one place that wraps instead, and the
   difference is the point: there, the exact command is what is being agreed to.

   The result is `null` while the call is still open, and `{ ok, summary, denial }`
   once it is not. **The outcome pair — the tick or the cross with the summary
   beside it — is drawn only with a result behind it**, never as a guess: a tick
   that appeared before the tool had finished would be the panel claiming
   something it does not know. What the open case gets instead is the live glyph
   `StatusBadge` gives `running`, which is a different statement from an
   outcome: a row with no mark at either end reads as a call that was made and
   then nothing, while the panel does in fact know that this one is still open.

   The leading glyph asks a different question from the one `core/icons.js`
   answers, whenever the detail names a file: `catppuccinIcon.js` draws what
   kind of file it is, and reading `documentTheme` here is what repaints it when
   the theme flips, since nothing inside a `data:` URL can be reached by the
   stylesheet. Everything else gets the wrench — a tool doing something that is
   not about one file. What counts as naming a file is the narrow reading
   deliberately: one unbroken token with a separator in it and an extension on
   its last segment. `rm -rf /tmp/x` holds a path and is a command; `--workspace`
   is neither; and the cost of guessing wrong either way is one glyph, never a
   wrong answer about what happened.

   **A `denial` draws a card under the row**, and only under it: the call was
   refused by Claude Code's auto-mode classifier, nobody is held waiting (the
   agent has already gone on and written its prose), and so it is deliberately
   not loud. It uses the failed idiom the row already has, never `needs-you`,
   whose fill is budgeted at one or two a screen for things that hold a
   session open. The two buttons exist only while `blockOpen` and the session
   takes messages; they emit and `ConversationView` sends, since this file
   knows no store. A closed block keeps the card without the buttons so what
   happened stays readable. */
import { computed } from 'vue'
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'
import { denialReason } from './autoModeBlock.js'
import { fileIconUrl } from '../../catppuccinIcon.js'
import { documentTheme } from '../../documentTheme.js'

const props = defineProps({
  name: { type: String, required: true },
  detail: { type: String, default: '' },
  /* `null` while the call is still running, `{ ok, summary, denial }` once it is not. */
  result: { type: Object, default: null },
  /* True while an auto-mode block has had no reply from the person yet. */
  blockOpen: { type: Boolean, default: false },
  canMessage: { type: Boolean, default: false }
})

const emit = defineEmits(['allow-block', 'refuse-block'])

const denial = computed(() => props.result?.denial ?? null)
const reason = computed(() => denialReason(denial.value))
const showButtons = computed(() => props.blockOpen && props.canMessage)

const FILE_PATH = /^\S*[/\\][^\s/\\]*\.[^\s/\\]+$/

const isPath = computed(() => FILE_PATH.test(props.detail))
const iconUrl = computed(() => (isPath.value ? fileIconUrl(props.detail, documentTheme.value) : ''))

const running = computed(() => props.result == null)
const ok = computed(() => props.result?.ok === true)

/* No horizontal inset of its own: this row is a direct child of the journal's
   `.sm-prose` (`ConversationView.vue`), which now spends `--panel-pad` once
   for the whole column. A second copy here lined up with the prose only by
   coincidence before that root existed and doubled the inset once it did. */
const row = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  minHeight: 'var(--row-h)',
  padding: 0,
  color: 'var(--text-primary)'
}

const nameStyle = {
  flex: '0 0 auto',
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)'
}

const detailStyle = {
  flex: 1,
  minWidth: 0,
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)'
}

/* The live glyph turns, the way `StatusBadge` turns `running`'s. */
const spin = { animation: 'sm-spin 1.6s linear infinite', color: 'var(--attn-live)' }

/* `0 1 auto` rather than `0 0 auto`, and it is the pair with the detail's
   `flex: 1` above that makes the row fit at every width: the detail has a basis
   of zero and gives way first, and a summary longer than the row is left has
   somewhere to shrink to instead of pushing the row open. */
const outcome = computed(() => ({
  flex: '0 1 auto',
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  minWidth: 0,
  overflow: 'hidden',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)',
  color: ok.value ? 'var(--text-muted)' : 'var(--status-failed-fg)'
}))

const outcomeGlyph = computed(() => ({
  color: ok.value ? 'var(--status-done-fg)' : 'var(--status-failed-fg)'
}))

const column = { display: 'flex', flexDirection: 'column', gap: 'var(--space-3)', minWidth: 0 }

const card = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  minWidth: 0,
  padding: 'var(--space-4)',
  background: 'var(--surface-raised)',
  border: 'var(--border-w) solid var(--status-failed-fg)',
  borderRadius: 'var(--radius-4)'
}

const heading = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  font: 'var(--weight-medium) var(--text-sm)/1.2 var(--font-sans)',
  color: 'var(--text-primary)'
}

/* `anywhere` and `pre-wrap` together: a 140-character command with no break
   in it and a reason of several lines both stay inside the column. */
const reasonStyle = {
  minWidth: 0,
  margin: 0,
  whiteSpace: 'pre-wrap',
  overflowWrap: 'anywhere',
  font: 'var(--weight-regular) var(--text-sm)/1.4 var(--font-sans)',
  color: 'var(--text-secondary)'
}

const buttons = { display: 'flex', flexWrap: 'wrap', gap: 'var(--space-3)' }

const summaryStyle = { minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }
</script>

<template>
  <div :style="column">
  <div :style="row">
    <!-- 15px and not the row's usual 13, for the reason `FileTreeRow` gives:
         this set draws on a 16 grid with padding of its own, where a lucide
         glyph fills its box. `alt` is empty — the path is right beside it. -->
    <img v-if="isPath" :src="iconUrl" alt="" width="15" height="15" :style="{ display: 'block', flex: '0 0 auto' }" />
    <Icon v-else name="wrench" :size="13" :style="{ color: 'var(--text-muted)' }" />

    <span :style="nameStyle">{{ name }}</span>
    <span v-if="detail" :style="detailStyle">{{ detail }}</span>
    <span v-else :style="{ flex: 1 }" />

    <Icon v-if="running" name="loader-circle" :size="11" :style="spin" />
    <span v-else :style="outcome">
      <Icon :name="ok ? 'check' : 'x'" :size="11" :stroke-width="2.25" :style="outcomeGlyph" />
      <span v-if="result.summary" :style="summaryStyle">{{ result.summary }}</span>
    </span>
  </div>
  <div v-if="denial" :style="card" data-auto-mode-block>
    <div :style="heading">
      <Icon name="x" :size="13" :stroke-width="2.25" :style="{ color: 'var(--status-failed-fg)' }" />
      <span>Blocked by auto mode</span>
    </div>
    <p :style="reasonStyle">{{ reason }}</p>
    <div v-if="showButtons" :style="buttons">
      <Button variant="primary" size="sm" @click="emit('allow-block')">Allow</Button>
      <Button variant="secondary" size="sm" @click="emit('refuse-block')">Refuse</Button>
    </div>
  </div>
  </div>
</template>
