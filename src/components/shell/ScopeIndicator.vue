<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import WindowControls from './WindowControls.vue'
import { CHROME_BUTTONS, CHROME_NONE, CHROME_STATES } from './windowChrome.js'

/* The scope bar answers "where am I working" before anything else on screen,
   and that one question is now the whole of it: repo / worktree @ branch, the
   search, the bell and the window's own buttons.

   What this project is *doing* used to be here as well — the headline, the two
   live counters and a segment per run — and it is `shell/StatusFooter.vue`'s
   now. This bar is the window's title bar, and a title bar that was also the
   status bar spent the eye's first stop on numbers that change every few
   seconds. */
const props = defineProps({
  repo: { type: String, required: true },
  worktree: { type: String, default: '' },
  branch: { type: String, default: '' },
  notifications: { type: Number, default: 0 },
  /* Which chrome the window around this bar has, from
     `shell/windowChrome.js`. `none` is the default and is what a browser gets:
     the gallery and the dev server draw this bar with no window behind it at
     all, and a default of anything else would put a gap in it there. */
  windowChrome: {
    type: String,
    default: CHROME_NONE,
    validator: (value) => CHROME_STATES.includes(value)
  },
  /* Only ever read while `windowChrome` is `buttons`: which of the two the
     middle button is. */
  maximized: { type: Boolean, default: false }
})

defineEmits(['notifications', 'settings', 'minimize', 'toggle-maximize', 'close'])

/* Still a plain object, and deliberately: the left inset varies, but it varies
   through a token the document root redefines, not through a prop. A computed
   here would claim a reactive dependency this object does not have. */
const barStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  height: 'var(--scope-bar-h)',
  flex: '0 0 auto',
  /* The inset clears macOS's traffic lights, which are drawn over this bar
     rather than beside it. `--title-bar-inset` is 0 in every other chrome, so
     this is the one expression rather than a branch. */
  padding: '0 var(--space-5) 0 calc(var(--space-5) + var(--title-bar-inset))',
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

const scopeName = computed(() => props.worktree || props.branch)

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
  <!-- The drag region is unconditional. Nothing in a browser listens for the
       attribute, and Tauri starts a drag only from the element that actually
       carries it, so the buttons and the search field below — which do not —
       keep working. -->
  <div :style="barStyle" data-tauri-drag-region>
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

    <span :style="{ flex: 1 }" data-tauri-drag-region />

    <!-- The search field. A slot rather than props: this bar knows about a
         repository and a branch, and giving it the tracker as well would make
         the one component on screen that is deliberately ignorant of what it is
         describing know the most of anything. -->
    <slot name="search" />

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

    <!-- Only where the system has stopped drawing them. On macOS the real
         traffic lights are over the other end of this bar and nothing belongs
         here; in a browser there is no window to command at all. -->
    <WindowControls
      v-if="windowChrome === CHROME_BUTTONS"
      :maximized="maximized"
      @minimize="$emit('minimize')"
      @toggle-maximize="$emit('toggle-maximize')"
      @close="$emit('close')"
    />
  </div>
</template>
