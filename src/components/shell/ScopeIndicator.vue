<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Tooltip from '../core/Tooltip.vue'

/* The scope bar answers "where am I working" before anything else on screen:
   repo / worktree @ branch, then the two live counters. */
const props = defineProps({
  repo: { type: String, required: true },
  worktree: { type: String, default: '' },
  branch: { type: String, default: '' },
  /* `null` and deliberately not `0`, the way `git/SectionHeader.vue` declares
     its own count: a working tree that could not be read has an unknown number
     of uncommitted files, which is the opposite fact to a clean one, and
     `stores/vcs.js` says so by handing over `null`. Both come out as no icon
     and no number here — the difference is that nothing in this component ever
     claims a repository is tidy on the strength of not knowing. */
  dirtyCount: { type: Number, default: null },
  /* Zero, because this one is never unknown: the store counts sessions and
     start tickets it is already holding, and there is no read behind it that
     could fail. */
  agentsActive: { type: Number, default: 0 },
  notifications: { type: Number, default: 0 },
  /* What this project is doing right now, from components/shell/headline.js.
     Empty is the ordinary case and draws nothing at all — most of the time
     nothing is happening, and a bar reserving room for the sentence would be a
     bar with a hole in it. */
  headline: { type: String, default: '' },
  /* The design system's attention vocabulary, so an agent waiting on somebody
     reads loud here as it does on a badge. `quiet` is the default. */
  headlineLevel: { type: String, default: 'quiet' }
})

defineEmits(['notifications', 'settings'])

const barStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  height: 'var(--scope-bar-h)',
  flex: '0 0 auto',
  padding: '0 var(--space-5)',
  background: 'var(--scope-bar)',
  borderBottom: 'var(--border-w) solid var(--border)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}

const segStyle = (strong) => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  minWidth: 0,
  color: strong ? 'var(--text-primary)' : 'var(--text-secondary)',
  fontWeight: strong ? 'var(--weight-medium)' : 'var(--weight-regular)'
})
const truncate = { whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }
const counter = (color) => ({ display: 'inline-flex', alignItems: 'center', gap: 'var(--space-2)', color })

const scopeName = computed(() => props.worktree || props.branch)

/* The one segment in this bar that is a sentence rather than a name, which is
   why it is the one that gives way when the window is narrow: the shrink factor
   in `flex` is far above the 1 every other item in the row has, so at 900px the
   repo and the branch keep their letters and this ellipsises instead.

   Loud takes the attention colour and the glyph below with it. Colour alone is
   what `status/status.js` refuses everywhere else, and a headline saying
   somebody is waited on is exactly the case that rule is written for. */
const headlineStyle = computed(() => ({
  ...truncate,
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  flex: '0 100 auto',
  minWidth: 0,
  color: props.headlineLevel === 'loud' ? 'var(--attn-loud)' : 'var(--text-muted)'
}))

/* The counters' tooltips. Both were glued together with a plural noun and
   said "1 uncommitted files" for the commonest case there is, which nobody saw
   while the numbers were a fixture holding 3 and 2 — the bell below had the
   same fault and was fixed for the same reason. Neither label is ever drawn
   over a count of zero: the counter itself is hidden then, so there is no third
   case to word. */
const dirtyLabel = computed(() =>
  props.dirtyCount === 1 ? '1 uncommitted file' : `${props.dirtyCount} uncommitted files`
)
const agentsLabel = computed(() =>
  props.agentsActive === 1 ? '1 agent running' : `${props.agentsActive} agents running`
)

/* The bell's name, and the tooltip over it. It used to say "unread", which is a
   ledger this app deliberately does not keep — a notification here is derived
   from what is true right now and goes when that stops being true, so there is
   nothing for "read" to mean. It also said "1 notifications". */
const notificationsLabel = computed(() => {
  if (props.notifications <= 0) return 'Notifications'
  return props.notifications === 1 ? '1 notification' : `${props.notifications} notifications`
})
/* The count over the bell. `pointerEvents: 'none'` is not decoration: the badge
   is drawn over the top right of a 24px button, which is where a pointer aiming
   at the middle of the glyph lands, so without it the badge swallows the press
   and the bell does nothing — silently, since the badge is `aria-hidden` and has
   no handler of its own to fail. It cost nothing while nobody listened for the
   click; the moment something did, it was the whole feature. */
const badgeStyle = {
  pointerEvents: 'none',
  position: 'absolute', top: '1px', right: '1px', minWidth: '12px', height: '12px', padding: '0 3px',
  display: 'flex', alignItems: 'center', justifyContent: 'center',
  background: 'var(--attn-loud)', color: 'var(--attn-loud-contrast)',
  border: 'var(--border-w) solid var(--scope-bar)', borderRadius: 'var(--radius-pill)',
  font: 'var(--weight-semibold) var(--text-2xs)/1 var(--font-mono)'
}
</script>

<template>
  <div :style="barStyle">
    <span :style="segStyle(true)">
      <Icon name="folder-git-2" :size="12" :style="{ color: 'var(--text-muted)' }" />
      <span :style="truncate">{{ repo }}</span>
    </span>
    <span :style="{ color: 'var(--border-strong)' }">/</span>
    <span :style="segStyle(true)">
      <Icon name="git-branch" :size="12" :style="{ color: 'var(--text-muted)' }" />
      <span :style="truncate">{{ scopeName }}</span>
    </span>
    <span v-if="branch && worktree" :style="{ color: 'var(--text-muted)' }">@{{ branch }}</span>

    <!-- What is going on in this project, when anything is. No `v-else`, no
         placeholder and no reserved width: an empty headline is the common
         case and the bar simply closes up around it. -->
    <span v-if="headline" :style="headlineStyle">
      <Icon v-if="headlineLevel === 'loud'" name="triangle-alert" :size="12" />
      <span :style="truncate">{{ headline }}</span>
    </span>

    <Tooltip v-if="dirtyCount > 0" :label="dirtyLabel">
      <span :style="counter('var(--git-modified)')">
        <Icon name="file-pen" :size="12" />{{ dirtyCount }}
      </span>
    </Tooltip>
    <Tooltip v-if="agentsActive > 0" :label="agentsLabel">
      <span :style="counter('var(--attn-live)')">
        <Icon name="bot" :size="12" />{{ agentsActive }}
      </span>
    </Tooltip>

    <!-- Whatever else belongs to the scope right now — today the run. Beside
         the counters rather than out at the right, because it is about this
         project and the buttons over there are about the window. -->
    <slot name="status" />

    <span :style="{ flex: 1 }" />

    <span :style="{ position: 'relative', display: 'inline-flex' }">
      <IconButton
        icon="bell"
        size="sm"
        :label="notificationsLabel"
        @click="$emit('notifications')"
      />
      <span v-if="notifications > 0" aria-hidden="true" :style="badgeStyle">{{ notifications }}</span>
    </span>
    <IconButton icon="settings" size="sm" label="Settings" @click="$emit('settings')" />
  </div>
</template>
