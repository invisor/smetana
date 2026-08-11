<script setup>
/* The projects this window is working with: one is active, the rest are one
   click away. A folder with no bd tracker still belongs here — it is a
   project you added, and the mark says what it is missing, quietly.

   Three marks can meet on one row, and none of them is told apart from the
   others by colour: the missing tracker is a lone muted triangle beside the
   name, with nothing on the row that fixes it; the missing run configuration is
   a red triangle bonded to the gear that opens the setup it is asking for; and
   a run configuration that cannot be parsed is a red page-with-a-cross,
   standing alone. The last needs its own glyph rather than a third triangle
   precisely because it stands alone — beside the tracker's lone triangle, the
   two would differ in nothing but hue.

   That it stands alone is still deliberate: a file that exists and cannot be
   parsed must not be answered by a button that starts an agent writing over it,
   and the row has nowhere to name the section that failed anyway — so the mark
   reports the state and offers nothing. The route out is the right-click menu,
   whose setup item is live over a damaged file: the dialog it opens is what
   explains what the agent is about to do, which is exactly the room the row
   does not have. It used to be a button in the run dialog, and that was one
   route too many for a project already carrying three actions of its own.

   No header of its own: the enclosing Panel already shows "Projects" and
   carries the "+" in its actions slot, so a second copy here would print the
   same word twice in a row. */
import { computed, nextTick, onBeforeUnmount, ref } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Tooltip from '../core/Tooltip.vue'
import ContextMenu from '../overlays/ContextMenu.vue'
import { projectMenuItems } from './projectMenu.js'

const props = defineProps({
  projects: { type: Array, default: () => [] },
  activePath: { type: String, default: null },
  /* Starting an agent is an action on a project, so it is offered on the
     project's own row — and only on the active one: a session belongs to the
     directory the window is pointed at, and the list below shows that
     project's sessions only. Off by default, since the row means the same
     thing whether or not anything can be started from it. */
  canAddAgent: { type: Boolean, default: false },
  /* Only ever about the active row, for the same reason canAddAgent is: the
     configuration is read on switching projects, and probing every row would
     be a command per project for a mark nobody is looking at. */
  needsSetup: { type: Boolean, default: false },
  /* The other half of the same question, and never true at the same time as
     `needsSetup` — a file is missing or it is damaged, and the back end reports
     one state, not two. Kept as a separate prop rather than folded into a
     three-valued one because the two draw differently and only one of them
     offers a button. */
  configBroken: { type: Boolean, default: false },
  /* The third of that same measured triple: a file that reads. Nothing on the
     row draws it — a project that is set up has nothing to report — and the
     menu needs it to choose between "Set up" and "Set up again". Taken as its
     own prop rather than inferred from the other two being false, which is a
     fourth state and means "not measured yet", not "configured". */
  configured: { type: Boolean, default: false }
})

/* `setup` carries whether a file is already there beside the path, because the
   dialog on the other end chooses its words from it. The gear passes false
   without asking: it is drawn only while `needsSetup`, which is that same
   question already answered. */
const emit = defineEmits(['select', 'remove', 'add-agent', 'setup'])

/* Hover has to be per row, and useInteractive tracks one control — so the
   list keeps the hovered path itself and asks useInteractive for nothing.
   Press is not tracked here: a row is not a button, it is a place. */
const hovered = ref(null)

/* Which row's menu is open, by path, and where the pointer opened it. The row
   under an open menu counts as hovered: the panel is teleported to the body, so
   the pointer moving into it fires `mouseleave` on the row, and a row that lost
   its highlight would leave the menu naming nothing. */
const menuFor = ref(null)
const marked = (project) => hovered.value === project.path || menuFor.value === project.path

/* Five rows and then it scrolls: the file tree under it must not be pushed
   off the bottom of the panel by a long list. */
const listStyle = {
  position: 'relative',
  flex: '0 0 auto',
  maxHeight: 'calc(5 * var(--row-h))',
  overflowY: 'auto',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}

const rowStyle = (project) => {
  const active = project.path === props.activePath
  return {
    position: 'relative',
    display: 'flex',
    alignItems: 'center',
    gap: 'var(--space-3)',
    height: 'var(--row-h)',
    padding: '0 var(--space-3) 0 var(--space-5)',
    background: active
      ? 'var(--surface-raised)'
      : marked(project)
        ? 'var(--surface-hover)'
        : 'transparent',
    boxShadow: active ? 'inset var(--border-w-strong) 0 0 0 var(--text-primary)' : 'none',
    color: active ? 'var(--text-primary)' : 'var(--text-secondary)',
    font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
    cursor: 'default',
    transition: 'var(--transition-control)'
  }
}

const nameStyle = { flex: 1, minWidth: 0, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }

/* Neither mark names a side, deliberately: this list scrolls, and a tooltip
   that had to open inside it was cut off by its edges whichever way it went.
   Tooltip now teleports its panel out of the document flow and chooses a side
   against the window, so the right answer here is to ask for nothing and let
   it decide — a row near the top of the panel gets its hint below, everywhere
   else above. */
const setupMarkStyle = { display: 'inline-flex', alignItems: 'center', gap: 'var(--space-2)' }

/* The buttons stay, menu or no menu. A right-click is a gesture people have to
   know about; the `x` is one click away from wherever the pointer already is,
   and this is a list people prune often. The button's box is always in the
   layout (visibility, not v-if/display), so revealing it on hover or active
   never shifts the row's own content. */
const removeButtonStyle = (project) => ({
  visibility: marked(project) || project.path === props.activePath ? 'visible' : 'hidden'
})

const empty = computed(() => props.projects.length === 0)

/* Ctrl+click is the secondary click on macOS, and WebKit — which is the engine
   this app actually runs in — dispatches a `click` for it as well as a
   `contextmenu`, where Chromium suppresses the click. Without this guard the
   menu opens on a row and the active project switches under it a moment later:
   the panel was drawn greyed for a project the window was not pointed at and
   silently turns live, which is precisely the surprise the design refused when
   it rejected a secondary click that changes what the window is pointed at.

   What is read is the gesture rather than whether a menu is open, because the
   second would be reading an event ordering that differs between the two
   engines — and Ctrl+click means nothing else here: there is no multi-select in
   this list for it to be taken away from.

   It cannot be reproduced in `?view=gallery`: that is Chromium, which sends no
   click at all. Only `npm run tauri dev` on macOS shows it. */
const onRowClick = (project, event) => {
  if (event.ctrlKey) return
  emit('select', project.path)
}

/* The menu. It could not have been drawn here at all until `ContextMenu`'s
   panel started leaving the document: an absolutely positioned descendant is
   clipped by any ancestor with `overflow`, which is `listStyle` above and
   `Panel`'s own scroll container after it. Teleported to the body and fixed in
   window coordinates, neither can reach it.

   Placement is repeated from `MenuButton.vue` rather than shared with it, for
   the reason recorded in that file's own header — and it is not the same
   routine anyway: `MenuButton` anchors to a trigger's rect, this one to the
   point the pointer was at, which is the only thing a secondary click gives. */
const box = ref(null)
/* Where the panel goes, in window coordinates. Null until measured — the one
   state it must not be seen in, since it would be sitting in the corner. */
const at = ref(null)
const point = ref({ x: 0, y: 0 })
const cursor = ref(-1)
/* Where the keyboard was when the menu opened, so Esc can hand it back. There
   is no trigger to return it to the way `MenuButton` has one: the row is a
   place, not a control, and it takes no focus. */
const returnTo = ref(null)

/* The distance from the pointer, and the closest the panel may come to the
   window's edge. Operands in arithmetic against getBoundingClientRect rather
   than values handed to the browser, which is why they are numbers here and not
   token references — the same note MenuButton, Dropdown and Tooltip carry. */
const GAP = 4
const EDGE = 8

/* Wide enough for the longest row it can hold, "New agent — switch to this
   project first", since `ContextMenu` clips a label rather than wrapping it.
   `ContextMenu` spends 48px of that on chrome before the label — 2×`--border-w`,
   2×`--space-2` of panel padding, 2×`--space-4` of row padding, the 14px icon
   column and one `--space-4` gap — so this leaves the sentence 212px. Not
   measured in a browser the way `TaskCard`'s `MENU_W` was, but scaled off that
   measurement: 76 characters came to 315px there, and 40 characters of the same
   `--text-sm` sans come to about 170. It carries the same trade with it — a
   number in px does not follow the app-wide font size, so the sentence is
   ellipsised somewhere above the middle of that range rather than at the top of
   it, and widening the panel to cover the top would hang 400px of menu off a
   252px panel for everybody. */
const MENU_W = 260

const items = computed(() =>
  projectMenuItems({
    active: menuFor.value !== null && menuFor.value === props.activePath,
    configured: props.configured,
    configBroken: props.configBroken,
    canAddAgent: props.canAddAgent
  })
)

/* Separators and greyed rows are drawn but never walked to. */
const walkable = computed(() =>
  items.value.map((it, i) => (it.type || it.disabled ? -1 : i)).filter((i) => i >= 0)
)

function place() {
  const rect = box.value?.getBoundingClientRect()
  if (!rect) return
  const { x, y } = point.value
  const room = window.innerHeight - y - GAP - EDGE
  /* Flipped above the pointer when the panel does not fit below it and there is
     more room above — the bottom row of a five-row list is exactly that case.
     The clamp below still applies afterwards, for a window too short for either
     side to hold the whole panel. */
  const above = rect.height > room && y - GAP - EDGE > room
  const top = above ? y - GAP - rect.height : y + GAP
  at.value = {
    left: Math.max(EDGE, Math.min(x + GAP, window.innerWidth - rect.width - EDGE)),
    top: Math.max(EDGE, Math.min(top, window.innerHeight - rect.height - EDGE))
  }
}

const openMenu = async (project, event) => {
  point.value = { x: event.clientX, y: event.clientY }
  menuFor.value = project.path
  cursor.value = -1
  at.value = null
  returnTo.value = document.activeElement
  document.addEventListener('pointerdown', onDocumentPointerdown, true)
  // Capture, so a scroll inside any ancestor is seen, not only the window's.
  window.addEventListener('scroll', onScroll, true)
  window.addEventListener('resize', place)
  await nextTick()
  place()
  /* A second tick before the focus, and it is not spare. `place` only writes
     the position; the panel is `visibility: hidden` until that write has been
     rendered, and an element that is not visible refuses focus — silently, and
     the whole keyboard with it. */
  await nextTick()
  box.value?.focus()
}

const hide = () => {
  if (menuFor.value === null) return
  /* Read before anything is torn down, because closing is what makes the answer
     unavailable: the panel leaves the document and `document.activeElement`
     falls back to `<body>`, at which point there is no telling whether the
     keyboard had been in here at all. */
  const held = box.value?.contains(document.activeElement)
  menuFor.value = null
  at.value = null
  cursor.value = -1
  document.removeEventListener('pointerdown', onDocumentPointerdown, true)
  window.removeEventListener('scroll', onScroll, true)
  window.removeEventListener('resize', place)
  /* Only when focus was inside, and that guard is not decoration: `hide` is
     also the unmount handler and the outside-click handler. On unmount there is
     nothing left to hand anything back to, and on an outside click the browser
     is a moment away from focusing whatever was clicked — moving it here first
     would be a flicker at best and a theft at worst. */
  const back = returnTo.value
  returnTo.value = null
  if (held && back?.isConnected) back.focus?.()
}

/* The whole list can leave under an open menu — the left panel collapsing to
   its rail, or the view being torn down — and the two document-level listeners
   would outlive it, since they are the document's and not this component's.
   Removing a *row* is not the case here, whatever the same guard in
   `MenuButton` is for: one menu lives per card there, so a deletion unmounts
   it, while one `ProjectList` holds every row and outlives any one of them. */
onBeforeUnmount(hide)

/* Scrolling closes rather than re-places, which is where this parts company
   with `MenuButton`. Its panel hangs off a trigger that moves with the scroll,
   so following it keeps the two together; this one hangs off a point the
   pointer was at, and the row that point named slides out from under it. A
   panel left behind would be offering three verbs about a project nothing on
   screen still connects it to. */
const onScroll = () => hide()

/* Pointerdown rather than click: a press that starts inside the panel and ends
   outside it would otherwise close the menu out from under the pointer. The
   panel is teleported to the body, so it is not inside this component's own
   tree and has to be checked on its own. */
const onDocumentPointerdown = (event) => {
  if (!box.value?.contains(event.target)) hide()
}

const pick = (item) => {
  if (!item || item.disabled || item.type) return
  const path = menuFor.value
  hide()
  if (item.kind === 'setup') emit('setup', path, item.existing)
  else if (item.kind === 'add-agent') emit('add-agent', path)
  else if (item.kind === 'remove') emit('remove', path)
}

/* One step along the list of walkable indices, wrapping at both ends. From
   nowhere (-1) a step down lands on the first row and a step up on the last,
   which is what makes the first arrow key after opening do the obvious thing. */
const step = (list, current, delta) => {
  if (!list.length) return -1
  const pos = list.indexOf(current)
  return list[(pos + delta + (pos < 0 ? (delta > 0 ? 0 : 1) : 0) + list.length) % list.length]
}

const onKeydown = (event) => {
  if (event.key === 'Escape') {
    event.preventDefault()
    hide()
  } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    cursor.value = step(walkable.value, cursor.value, event.key === 'ArrowDown' ? 1 : -1)
  } else if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    pick(items.value[cursor.value])
  }
}

const panelStyle = computed(() => ({
  position: 'fixed',
  top: `${at.value?.top ?? 0}px`,
  left: `${at.value?.left ?? 0}px`,
  // Hidden until measured: one frame of it in the window's corner reads as a
  // flash, and it has to be in the document at its natural height to be
  // measured at all.
  visibility: at.value ? 'visible' : 'hidden',
  zIndex: 'var(--z-popover)',
  outline: 'none'
}))
</script>

<template>
  <div :style="{ display: 'flex', flexDirection: 'column', minWidth: 0 }">
    <div v-if="empty" :style="{ padding: 'var(--space-5)', fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }">
      No projects yet.
    </div>

    <div v-else :style="listStyle">
      <!-- `.prevent` stops the browser's own menu: two menus over one row, one
           of them offering Reload, is worse than either alone. The row is not
           selected on the way — a secondary click that changed which project
           the window is pointed at would be an edit nobody asked for, which is
           what `onRowClick` guards where the secondary click is Ctrl+click. -->
      <div
        v-for="p in projects"
        :key="p.path"
        :style="rowStyle(p)"
        @click="onRowClick(p, $event)"
        @contextmenu.prevent="openMenu(p, $event)"
        @mouseenter="hovered = p.path"
        @mouseleave="hovered = null"
      >
        <!-- The path hangs on the name, not on the row. It used to be the row's
             native `title`, which every glyph inside then had to suppress with
             an empty one of its own; a `Tooltip` on the row cannot be suppressed
             that way at all, because `mouseleave` does not fire on the way to a
             child — hovering a glyph would draw the glyph's panel and leave the
             row's open beside it. The name is also the element that is
             ellipsised, so revealing the whole path there is where a person
             looks for it. -->
        <Tooltip :label="p.path" :style="{ flex: 1, minWidth: 0 }">
          <span :style="nameStyle">{{ p.name }}</span>
        </Tooltip>
        <Tooltip v-if="!p.tracked" label="No bd tracker here">
          <Icon name="triangle-alert" :size="12" :style="{ color: 'var(--text-muted)' }" />
        </Tooltip>
        <!-- The mark and the button that clears it are one hover target, tied
             together by a gap narrower than the row's own: the triangle is what
             a person sees without touching anything, the gear is what they
             press. Red is the loudest colour the system has, and it is only
             ever spent once here — the mark is drawn for the active project
             alone, the same reason the gear is. -->
        <!-- The tooltip wraps the glyph alone, not the pair. A panel centred
             over both would hang off to one side of whichever of them the
             pointer is actually on, and the two have different things to say:
             the mark reports the state, the button offers the action, and the
             button already carries its own label. -->
        <span v-if="needsSetup && p.path === activePath" :style="setupMarkStyle">
          <Tooltip label="Not set up for runs">
            <Icon name="triangle-alert" :size="12" :style="{ color: 'var(--status-failed-fg)' }" />
          </Tooltip>
          <IconButton
            icon="settings-2"
            label="Set up for runs"
            size="sm"
            @click.stop="emit('setup', p.path, false)"
          />
        </span>
        <!-- No gear beside it: see the note at the top of this file. The mark is
             what a person sees without pressing anything, and the right-click
             menu is where the setup is offered over a file nobody can read. -->
        <Tooltip v-if="configBroken && p.path === activePath" label="Run configuration cannot be read">
          <Icon name="file-x" :size="12" :style="{ color: 'var(--status-failed-fg)' }" />
        </Tooltip>
        <!-- Before the remove button, not after it: removal keeps the row's
             last position wherever it appears, so a click aimed at it never
             lands on something that moved in. -->
        <IconButton
          v-if="canAddAgent && p.path === activePath"
          icon="plus"
          label="New agent"
          size="sm"
          @click.stop="emit('add-agent', p.path)"
        />
        <IconButton
          icon="x"
          label="Remove from list"
          size="sm"
          :style="removeButtonStyle(p)"
          @click.stop="emit('remove', p.path)"
        />
      </div>
    </div>

    <Teleport to="body">
      <!-- The keydown sits on this wrapper rather than on `ContextMenu`: this
           is the element that takes the focus, and a listener on a component
           tag would have to fall through as an attribute to reach a node at
           all. -->
      <div
        v-if="menuFor !== null"
        ref="box"
        :style="panelStyle"
        tabindex="-1"
        @keydown="onKeydown"
      >
        <ContextMenu
          :items="items"
          :cursor="cursor"
          :width="MENU_W"
          @select="pick"
          @hover="cursor = $event"
        />
      </div>
    </Teleport>
  </div>
</template>
