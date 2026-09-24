<script setup>
/* The project's agent list. Split out of DesktopApp.vue: that file is
   already past nine hundred lines, and a live list with removal would have
   made it unreadable.

   Starting an agent is not here: it belongs to a project, and it is offered
   in the menu on the project's tile in ProjectRail.vue. This component is the
   sessions and nothing else.

   Colour is never the only signal here: needs-you is the status system's
   warning triangle, everything else is the agent's own glyph.

   A row is captioned by the work, not by the process: `claude-7` names a
   process, and five of those in a column said nothing about who was doing
   what. The caption arrives in two pieces because they are set differently —
   `label` is prose and goes in sans, `tasks` are issue ids and go in mono.

   **The order is the person's**, and this component draws it rather than
   deciding it: `components/agent/agentOrder.js` is the whole of the rule, the
   caller keeps it in `settings.json`, and what happens here is a drag and a
   redraw. The same split `TabBar.vue` and `KanbanBoard.vue` keep, for the same
   reason — a list that owned its own order would be a second answer to a
   question `settings.json` already answers.

   The menu is `PointerMenu` on a secondary click, the same panel on the same
   gesture `ProjectRail` and `BranchList` use; its rows are
   `components/agent/agentMenu.js`. There is no native context menu anywhere in
   this app (`src/nativeMenu.js`), so this is the only one a right click can
   open here. Clearing a session is one of its rows and is the only verb here
   that reaches the harness rather than the app: the row carries the session id
   and nothing more, and which words clear a conversation is
   `agents::Profile::clear_command`'s to say.

   Rename is the menu's own first row (smetana-b9se) and is drawn here rather
   than through a dialog: the field replaces the caption **in place**, at the
   row's own height, the way `FileTreeDraftRow.vue` replaces a tree row rather
   than opening over the whole screen — the answer to "which row is this" is
   the row's own position, which a modal in the middle of the panel cannot
   give. It differs from that draft row in one decision, and the difference
   is deliberate: losing the focus **commits** here rather than cancelling.
   The tree's row genuinely stands still under an open draft; this one does
   not — a `session:state` can still repaint the row's own label under the
   field while somebody is typing — but the field itself is untouched by
   that: it is keyed by `agentKey`, and `v-model` owns its value regardless
   of what the row around it does, so a name typed and then clicked away
   from is worth keeping rather than throwing away. `withAgentName` and
   `nameAgentRows` in `agentName.js` are the pure rules this component leans
   on; the write itself is the caller's, through the `rename` event below. */
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import { useInteractive } from '../core/interactive.js'
import PointerMenu from '../overlays/PointerMenu.vue'
import { STATUS_GLYPH, attentionLevel } from '../status/status.js'
import { AGENT_MENU_W, agentMenuItems, closableOthers } from './agentMenu.js'
import { agentKey, isPinned, moveAgent, orderAgents, togglePin } from './agentOrder.js'

const props = defineProps({
  rows: { type: Array, default: () => [] },
  activeId: { type: [Number, String], default: null },
  /* Which agents are kept above the rest, by conversation id — the caller's
     list, straight out of `settings.json`. A prop rather than a flag on each
     row, which is `BranchList`'s shape for its favourites and the same subject:
     what is marked is a fact about the stored list, not about how the panel
     happens to be drawn today. */
  pinned: { type: Array, default: () => [] }
})

/* `reorder` carries the rows in their new order rather than a from/to pair, for
   `TabBar`'s reason: this list does not own the order and has no business
   describing a change to something it does not keep. The rows themselves and
   not their ids, which is where it parts company with that row — two different
   keyings are wanted out of the answer, the conversation ids that reach
   `settings.json` and this window's own keys for the rows that have none, and
   the row is the only place both of them live.

   `pin` carries the whole new list, the way `BranchList` emits its favourites:
   the toggle is a pure rule and belongs beside the others rather than in the
   caller. `remove-others` carries the id of the row the menu was open on and
   nothing more — the same shape `remove` already has, and the caller asks
   `closableOthers` the same question this component just asked it, from the
   rows and the pins it already owns, rather than being handed a list of ids
   to remove one by one. `rename` carries `{ conversation, name }` — the
   conversation id the field was opened on and the text it held when it
   closed, trimmed or not; `withAgentName` on the caller's side is where an
   empty value becomes the removal of a stored name, so this component sends
   exactly what was typed. */
const emit = defineEmits(['select', 'remove', 'reorder', 'pin', 'clear', 'remove-others', 'rename'])

const body = { flex: 1, minHeight: 0, overflow: 'auto' }

/* Hover has to be per row, and useInteractive tracks one control at a time
   — calling it fresh from inside rowStyle would throw hover state away on
   every re-render, since that would build a new pair of refs each time. So
   each row's instance is created once and cached by id, the way a keyed
   ref would be. Press is not tracked: a row is not a button, it is a
   place, the same reasoning ClaimedTasks.vue uses. */
const rowInteractive = new Map()
const interactiveFor = (id) => {
  let entry = rowInteractive.get(id)
  if (!entry) {
    entry = useInteractive()
    rowInteractive.set(id, entry)
  }
  return entry
}

/* Sessions come and go with agents; without this the cache would keep one
   stale entry per agent that ever existed for the life of the component. */
watch(
  () => props.rows.map((row) => row.id),
  (ids) => {
    const live = new Set(ids)
    for (const id of rowInteractive.keys()) {
      if (!live.has(id)) rowInteractive.delete(id)
    }
  }
)

/* The order under the pointer, and only while the pointer holds it. Idle, this
   is null and the panel draws exactly what it was given — the drawn order is
   the caller's, the same way the tab row's is. The draft is applied through
   `orderAgents`, so an agent started mid-drag appears at the end instead of
   vanishing, and so the pinned block stays in front however the draft was
   dragged: a row cannot be dropped above it, and a pinned row cannot be dropped
   out of it. `held` is the key of the row being dragged. */
const draft = ref(null)
const held = ref(null)

const view = computed(() =>
  draft.value ? orderAgents(props.rows, draft.value, props.pinned) : props.rows
)

/* A Crew tree's order and parentage come from the provider contract. It shares
   the panel with ordinary sessions but is not a flat persisted arrangement, so
   a drag must not turn a child into a sibling just because both are visible. */
const movable = computed(
  () => view.value.length > 1 && !props.rows.some((row) => Number.isFinite(Number(row.depth)))
)

const pinnedRow = (row) => isPinned(row, props.pinned)

const bodyRef = ref(null)

/* The page-wide guards a drag needs, the same pair `TabBar.vue` and
   `KanbanBoard.vue` take: without them the pointer sweeping the list selects
   every caption it crosses, and the cursor flickers between grabbing and
   whatever it passes over. */
let guarded = null
const guard = () => {
  if (guarded) return
  guarded = { userSelect: document.body.style.userSelect, cursor: document.body.style.cursor }
  document.body.style.userSelect = 'none'
  document.body.style.cursor = 'grabbing'
}
const unguard = () => {
  if (!guarded) return
  document.body.style.userSelect = guarded.userSelect
  document.body.style.cursor = guarded.cursor
  guarded = null
}

let capture = null
let moved = false
/* The frame between the draft changing and the list being redrawn, during which
   nothing can be measured where it is about to stand: the rows still sit where
   they sat before the swap.

   `TabBar`'s latch has no counterpart here, and the reason is the one
   `KanbanBoard` gives for not needing one either: every row in this list is
   exactly `--row-h` tall, so after a swap the pointer is over the held row
   again by construction and the next move finds it already where it belongs. A
   tab is as wide as its own label, which is the guarantee that file has to buy
   back with a latch. */
let settling = false

/* The rows as elements, in drawn order. Read off the list's own children rather
   than looked up by id — `KanbanBoard.vue`'s `columnAt` does the same, and for
   a stronger reason here: a row's key is a session number, a start ticket or a
   UUID, none of which is a thing to put in a selector. The empty state is the
   only other child this box can hold, and it is drawn only when there are no
   rows at all. */
const cells = () => (bodyRef.value ? [...bodyRef.value.children] : [])

/* Which row the pointer is over, in the drawn list's indices. Everything above
   the first answers 0 and everything below the last answers the last, so a
   pointer that has left the panel vertically still names a row. */
const rowAt = (y) => {
  const boxes = cells()
  for (let i = 0; i < boxes.length; i += 1) {
    if (y < boxes[i].getBoundingClientRect().bottom) return i
  }
  return boxes.length - 1
}

/* One row's vertical extent, or nothing when the list no longer has that index
   — an agent can exit under a drag. */
const cellBox = (at) => {
  const box = cells()[at]
  if (!box) return null
  const { top, bottom } = box.getBoundingClientRect()
  return { top, bottom }
}

/* Escape abandons the drag. It has to be on the window: the pointer is captured
   and focus is wherever it was, so a handler on the list would never see it. */
const onKeydown = (event) => {
  if (event.key !== 'Escape') return
  event.preventDefault()
  end(false)
}

/* A press is not yet a drag, and on this list that distinction is the whole of
   whether a row can still be clicked at all.

   Pointer capture retargets the compatibility mouse events. With the capture
   taken in `pointerdown` the `mouseup` goes to the list, the `click` is then
   dispatched at the nearest common ancestor — the list again — and the row's
   own `@click` never fires: picking an agent stops working altogether.
   `KanbanBoard.vue` captures on the press and is right to, because a column
   header carries no click of its own. A row carries the one gesture this panel
   exists for, so this follows `TabBar.vue` instead.

   So a press only **arms**: it remembers the row and the box it was pressed in,
   and waits. A release inside that box takes no capture at all and stays an
   ordinary click. The drag begins the moment the pointer leaves that box
   vertically.

   The page guards go on here rather than at the drag's start: a press that has
   not moved yet may still become one, and a text selection begun in the
   meantime would outlive it. */
let armed = null

const armedOff = () => {
  window.removeEventListener('pointermove', onArmedMove)
  window.removeEventListener('pointerup', onArmedUp)
  window.removeEventListener('pointercancel', onArmedUp)
  armed = null
}

const onArmedUp = (event) => {
  if (event.pointerId !== armed?.pointerId) return
  armedOff()
  unguard()
}

/* A secondary press opens the menu and takes hold of nothing — `event.button`
   is what tells the two apart, since `contextmenu` arrives after a
   `pointerdown` of its own. The press on the cross, or on any other button a
   row grows, is that button's and not the list's: `Tab.vue` carries the same
   guard, and for its reason — the click would survive a drag that moved
   nothing, but the row would slide about while somebody aimed at a 24px
   target. */
const onPointerdown = (row, event) => {
  if (!movable.value || event.button !== 0) return
  if (event.target.closest('button')) return
  if (draft.value || armed) return
  const key = agentKey(row)
  const box = cellBox(view.value.findIndex((one) => agentKey(one) === key))
  if (!box) return
  armed = { key, pointerId: event.pointerId, box }
  guard()
  /* On the window rather than on the list: until the capture is taken there is
     nothing making the moves arrive at any one element, and a pointer that left
     the panel would leave the press armed with nothing to disarm it. */
  window.addEventListener('pointermove', onArmedMove)
  window.addEventListener('pointerup', onArmedUp)
  window.addEventListener('pointercancel', onArmedUp)
}

const onArmedMove = (event) => {
  if (!armed || event.pointerId !== armed.pointerId) return
  if (event.clientY >= armed.box.top && event.clientY <= armed.box.bottom) return
  const { key, pointerId } = armed
  armedOff()
  begin(key, pointerId)
  onPointermove(event)
}

/* Capture goes on the scrolling list, not on the row that was pressed. It is
   what makes a release outside the window still end the drag — and taking it
   here means every move and release arrives at one element regardless of which
   row the pointer has since crossed into.

   It is also the one call that can throw, on a pointer the engine no longer
   knows. Letting that escape would leave the page unselectable with no drag
   left to release it, so a refused capture costs the drag once the pointer
   leaves the panel, and nothing more. */
const begin = (key, pointerId) => {
  held.value = key
  draft.value = view.value.map(agentKey)
  moved = false
  window.addEventListener('keydown', onKeydown)
  try {
    bodyRef.value.setPointerCapture(pointerId)
    capture = pointerId
  } catch {
    capture = null
  }
}

const sameOrder = (a, b) => a.length === b.length && a.every((key, at) => key === b[at])

/* The order is rebuilt from what is drawn on every move rather than mutated in
   place: `rowAt` answers in the drawn list's indices, and the two would disagree
   the moment an agent started or exited mid-drag.

   **What counts as having moved is what is drawn, and never the draft**, which
   is the one place this parts company with `TabBar.vue` and it is the pinned
   block that forces it. There, a draft that changed always redrew; here
   `orderAgents` has the last word, so dragging a pinned row below the block, or
   an unpinned row above it, produces a new draft on every move that is then
   redrawn exactly as it was. Counting those would have a drag that visibly did
   nothing emit `reorder` with the order already on screen — flipping a project
   from "never arranged" to "arranged", which is not nothing: it is what a later
   `Unpin` reads to find a row's place again. `moveAgent`'s reference-identity
   contract says the same thing one level down, and this is that rule where the
   drawn list and the draft can disagree. */
const onPointermove = (event) => {
  if (!draft.value || settling) return
  const order = view.value.map(agentKey)
  const next = moveAgent(order, order.indexOf(held.value), rowAt(event.clientY))
  if (next === order) return
  draft.value = next
  if (!sameOrder(order, view.value.map(agentKey))) moved = true
  settling = true
  nextTick(() => {
    settling = false
  })
}

function end(commit = true) {
  if (!draft.value) return
  /* Read before the draft is cleared: `view` is computed from it, and this is
     the drawn order the caller is being handed. */
  const ordered = view.value
  const changed = moved
  draft.value = null
  held.value = null
  moved = false
  settling = false
  unguard()
  window.removeEventListener('keydown', onKeydown)
  if (capture != null) {
    /* Releasing a capture the element no longer holds throws in some engines,
       and by here the pointer may already be gone. */
    try {
      bodyRef.value?.releasePointerCapture(capture)
    } catch {
      /* already released — nothing to undo */
    }
    capture = null
  }
  if (commit && changed) emit('reorder', ordered)
}

// A press or a drag that outlives the component would leave the page
// unselectable, and its window listeners attached to nothing.
onBeforeUnmount(() => {
  armedOff()
  unguard()
  window.removeEventListener('keydown', onKeydown)
})

/* The menu, and which row it is open on. The key is kept here because the items
   are built from that row and because the row under an open panel has to keep
   its highlight — the panel is teleported to the body, so the pointer moving
   into it leaves the row. Everything else about it is `PointerMenu`'s. */
const menu = ref(null)
const menuFor = ref(null)

const menuRow = computed(
  () => view.value.find((row) => agentKey(row) === menuFor.value) ?? null
)

const items = computed(() =>
  agentMenuItems({
    pinned: menuRow.value ? pinnedRow(menuRow.value) : false,
    conversation: menuRow.value?.conversation ?? null,
    starting: Boolean(menuRow.value?.starting),
    state: menuRow.value?.state ?? null,
    /* **The row's own answer, not the panel's.** Whether the harness has a line
       that clears a conversation is `Profile::clear_command`'s, by way of
       `agents::catalogue` and `stores/agents.js`, and it used to arrive here as
       one prop for the whole list — which was right only while every session in
       a project ran the same harness. It is a session's fact now: two rows of
       one panel can be on two harnesses, and a single flag was wrong in both
       directions, drawing a Clear row that Rust then refuses and hiding one
       that would have worked.

       Still nothing this list *draws*: it has never named the agent on a row,
       deliberately, and this is read by the menu alone. `false` for a row that
       does not say, which is the same refusal the prop's default was: a row
       promising on a guess would send a clearing line nobody confirmed as the
       first line of somebody's prompt. */
    clearable: Boolean(menuRow.value?.clearable),
    /* How many rows `Close other agents` would actually reach: every drawn
       row but this one, minus the pinned and the still-starting.
       `closableOthers` takes `id` and not `agentKey` — see its own header —
       so the row the menu is open on is excluded by the same field the
       cross itself is pressed with. */
    others: menuRow.value ? closableOthers(view.value, menuRow.value.id, props.pinned).length : 0
  })
)

const openMenu = (row, event) => {
  menuFor.value = agentKey(row)
  menu.value?.open(event, agentKey(row))
}

/* Pairs with `MAX_AGENT_NAME_LEN` in `src-tauri/src/settings/model.rs`,
   written out a second time for the reason every doubled ceiling in this app
   is: Rust validates on save, this is what stops the field accepting more
   than a save would keep. Both sides count **characters** rather than bytes,
   and that still leaves the two not quite the same measure: `maxlength` is
   UTF-16 code units, which equals the character count for ordinary text and
   is *stricter* for anything outside the Basic Multilingual Plane, where one
   character is a surrogate pair and costs two. So the field is never the
   generous half of the pair — at worst it clips a name of rare characters a
   little short of what Rust would still have accepted — and Rust's own
   `chars().count()` stays the real ceiling for everything this field lets
   through. */
const AGENT_NAME_MAX = 120

/* The row whose caption is a field right now, by `agentKey`, and the text in
   it. One at a time, but not by closing the first unsaved: `startRename`
   only ever runs from a menu pick, and picking a second row's Rename first
   takes the pointer through the first field's own `blur`, which commits it
   — so by the time `startRename` overwrites `renaming` for the new row, the
   old one has already been saved rather than discarded. What `startRename`
   itself does is simpler than that account: it overwrites the refs, and the
   field that was standing unmounts as the new one mounts.

   `renameFrom` is what the field opened with, clipped to `AGENT_NAME_MAX`
   the same way the prefill itself is (see `startRename`), and held apart
   from `row.label`: `commitRename` compares the draft against this rather
   than against the row's *current* label, because the row is not otherwise
   static while the field is open — a fresh automatic title can still arrive
   underneath it — and comparing against a title that changed after the
   field was opened would let an untouched Enter save whatever the title
   happened to become in the meantime, which is exactly the freeze the no-op
   check exists to prevent.

   `renameField` is written by the input's own `:ref` function below, and it
   has to be a **function** rather than the plain `ref="renameField"` string
   form: this element sits inside the `v-for`, and the string form marks the
   binding `ref_for` regardless of the `v-if` limiting it to one element at a
   time, which makes Vue's runtime collect it into an array rather than hand
   back the element itself. A function ref is called with the raw element on
   every mount and unmount whichever way it is reached. Inside the function
   the compiler auto-unwraps a bare `renameField` identifier exactly the way
   `v-model` unwraps `renameDraft` two lines down — do **not** write
   `renameField.value = el` in the template: this same auto-unwrap already
   turns that into `renameField.value.value = el` at build time (confirmed by
   compiling this template through `@vue/compiler-sfc` in both its dev and
   its inlined production shape), which throws on the first mount because
   `renameField.value` is still `null`. */
const renaming = ref(null)
const renameFrom = ref('')
const renameDraft = ref('')
const renameField = ref(null)

const startRename = (row) => {
  renaming.value = agentKey(row)
  // By code point (`Array.from`) and not by index, so a surrogate pair is
  // never split — and clipped here rather than left to `maxlength`, which
  // only holds a person's own typing to the cap and does nothing to a value
  // set programmatically: a resumed row's caption is "Resume session:
  // <title>" and a title can run to 120 characters on its own, so the raw
  // label can already be past `AGENT_NAME_MAX` before anybody has touched
  // the field. Trimmed after the clip, and not only inside `commitRename`:
  // a clip can land the cut on a space, and an untrimmed `renameFrom` would
  // then never equal `commitRename`'s own trimmed draft even when nothing
  // was typed, defeating the no-op check on the one row long enough to need
  // clipping at all.
  renameFrom.value = Array.from(row.label ?? '').slice(0, AGENT_NAME_MAX).join('').trim()
  renameDraft.value = renameFrom.value
  nextTick(() => {
    renameField.value?.focus()
    renameField.value?.select()
  })
}

/* Enter and blur commit, Esc cancels. Blur commits where the tree's draft
   cancels, deliberately: the row this field sits in is not otherwise static
   while it is open — a fresh automatic title can still arrive and repaint
   the row underneath it — but nothing about *this field* is torn down by
   that, since the input is keyed by `agentKey` and `v-model` keeps its own
   binding, so what somebody has typed survives it untouched. A name typed
   and then clicked away from is worth keeping rather than throwing away,
   which is the tree's own reason for the opposite rule turned around: nothing
   there is mid-edit until a person deliberately starts one.

   Committing exactly what the field opened with is a no-op and emits
   nothing at all, the draft trimmed before the comparison so trailing
   whitespace typed by accident does not count as a change — against
   `renameFrom`, the clipped and already-trimmed text the field opened on
   (see `startRename`), and never against `row.label` read fresh: the row
   can have a new automatic title by the time Enter is pressed, and
   comparing against that would let an untouched field still commit,
   freezing whatever the title had become in the meantime into a permanent
   manual name the automatic rule could then never move again. An empty
   commit that *is* a change from what was there is still the caller's
   business: `withAgentName` removes the entry and the automatic title (or
   the intent's caption) shows again. */
const commitRename = () => {
  const row = props.rows.find((one) => agentKey(one) === renaming.value)
  if (row) {
    const name = renameDraft.value.trim()
    if (name !== renameFrom.value) emit('rename', { conversation: row.conversation, name: renameDraft.value })
  }
  renaming.value = null
}
const cancelRename = () => {
  renaming.value = null
}

/* IME composition sends its own Enter to confirm a candidate, and the field
   must not read that as "commit the name" while somebody is still choosing
   the characters they meant to type. `event.isComposing` is the documented
   signal, but WebKit fires the confirming Enter's `keydown` *after* its own
   `compositionend`, by which point `isComposing` has already gone back to
   `false` — so that flag alone misses exactly the keystroke it exists to
   catch on that engine. `keyCode === 229` is the older, still-live signal
   for the same event on every engine, kept for no better reason than that
   nothing newer replaced it. */
const onRenameEnter = (event) => {
  if (event.isComposing || event.keyCode === 229) return
  event.preventDefault()
  commitRename()
}

/* The row is handed back with the pick rather than read from `menuFor`, which
   closing has already cleared — see `PointerMenu`'s header. The kinds are
   turned into events by hand for `BranchList`'s reason: the words happen to
   match today, and a rule file free to add a verb must not be able to make this
   component emit something nobody declared. */
const pick = (item, key) => {
  const row = props.rows.find((one) => agentKey(one) === key)
  if (!row) return
  if (item.kind === 'rename') startRename(row)
  else if (item.kind === 'pin') emit('pin', togglePin(props.pinned, row.conversation))
  /* The session and nothing else: what to write is the harness's own word for
     it and this window never learns which harness a session runs, so the caller
     hands the id to the store and Rust composes the line. */
  else if (item.kind === 'clear') emit('clear', row.id)
  else if (item.kind === 'close') emit('remove', row.id)
  else if (item.kind === 'close-others') emit('remove-others', row.id)
}

const rowStyle = (row) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  /* Crew rows carry a backend-owned depth. Ordinary sessions omit it and keep
     the exact flat layout; the indent is presentation only and never changes
     selection, ordering or the stable row key. */
  paddingLeft: `calc(var(--space-5) + ${(Math.max(0, Number(row.depth) || 0)) * 16}px)`,
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)',
  background:
    row.id === props.activeId || agentKey(row) === menuFor.value
      ? 'var(--surface-raised)'
      : interactiveFor(row.id).hover.value
        ? 'var(--surface-hover)'
        : 'transparent',
  cursor: movable.value ? (agentKey(row) === held.value ? 'grabbing' : 'grab') : 'default',
  /* Without this a touch drag scrolls the list instead of moving the row, and
     the pointer capture never sees the moves. */
  touchAction: movable.value ? 'none' : 'auto',
  opacity: attentionLevel(row.state) === 'quiet' ? 'var(--attn-quiet-opacity)' : 1,
  transition: 'var(--transition-control)'
})

/* One mark to a row, and it is the agent's own glyph carrying the state in
   its colour — a dot beside the agent icon said the same thing twice and
   spent two marks of a row's width on one signal.

   needs-you is the exception, and it is what keeps the row honest: it draws
   STATUS_GLYPH's own glyph — the same triangle-with-a-bang the badges use —
   rather than the agent icon in a louder colour, so the loud row is still
   distinguishable by silhouette alone. One status, one picture across the app.

   The mark's box is fixed at the icon's size: the two glyphs differ in
   shape, and a row must not shift by four pixels when an agent starts
   waiting. */
const MARK = 12
const markBox = {
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: `${MARK}px`,
  height: `${MARK}px`,
  flex: 'none'
}

/* One glyph for every agent. Lucide has no brand marks, and telling claude
   from codex on a row was never the question this list has to answer — what
   the agent is doing was. */
const AGENT_ICON = 'bot'

const markColour = (state) => (state === 'running' ? 'var(--attn-live)' : 'var(--text-muted)')

/* minWidth 0 and the ellipsis are what keep a long caption — a run holding
   four issues — from pushing the elapsed time and the remove button off the
   end of the row: a flex item refuses by default to shrink below its own
   content. Baseline rather than centre, because the two halves are set in
   different families and centring would leave them sitting at different
   heights.

   `flex` is a function of the row rather than a constant, and only for the
   one row being renamed: the box otherwise sizes to its own content and the
   sibling spacer eats what is left, the same shrink-to-fit every other row
   keeps, but a field open for editing is worth the whole width the spacer
   would have taken rather than whatever the label's own text happened to
   measure. */
const captionBoxStyle = (row) => ({
  display: 'flex',
  alignItems: 'baseline',
  gap: 'var(--space-2)',
  minWidth: 0,
  flex: renaming.value === agentKey(row) ? '1 1 0%' : 'initial',
  overflow: 'hidden',
  whiteSpace: 'nowrap'
})
const idsStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
}
const labelStyle = { overflow: 'hidden', textOverflow: 'ellipsis' }
/* The field that stands in for the caption while a row is being renamed.
   Every value a `var(--token)` reference, and the height is `--control-h-sm`
   rather than `--row-h` — a full-height field would touch the row's own top
   and bottom rules, where this one sits inside it the way the pin's box
   already does. */
const renameStyle = {
  width: '100%',
  minWidth: 0,
  height: 'var(--control-h-sm)',
  padding: '0 var(--space-2)',
  border: 'var(--border-w) solid var(--focus-ring)',
  borderRadius: 'var(--radius-2)',
  outline: 'none',
  background: 'var(--surface-raised)',
  color: 'var(--text-primary)',
  font: 'inherit'
}
/* The elapsed time stays mono: it is a measurement in a column, and a
   proportional face would let "18m" and "2h 14m" wander sideways as the clock
   ticks. */
const elapsedStyle = { font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)', flex: 'none' }

/* The mark a pinned row carries where every other row carries its cross. Not a
   button: "cannot be closed" is drawn rather than enforced, so the verb is
   simply absent and the menu is where the way back out lives. The box is the
   `sm` icon button's own, glyph size included, so the pin and the cross occupy
   exactly the same space and pinning a row moves nothing else on it. */
const pinBox = {
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: 'var(--control-h-sm)',
  height: 'var(--control-h-sm)',
  flex: 'none',
  color: 'var(--text-muted)'
}

const empty = computed(() => props.rows.length === 0)
</script>

<template>
  <div :style="{ display: 'flex', flexDirection: 'column', height: '100%' }">
    <!-- The capture is taken here, so every move and release during a drag
         arrives at one element whichever row the pointer has crossed into. -->
    <div
      ref="bodyRef"
      :style="body"
      @pointermove="onPointermove"
      @pointerup="end()"
      @pointercancel="end(false)"
      @lostpointercapture="end()"
    >
      <div
        v-for="row in view"
        :key="agentKey(row)"
        :data-attention="attentionLevel(row.state)"
        :style="rowStyle(row)"
        v-bind="interactiveFor(row.id).handlers"
        @click="$emit('select', row.id)"
        @pointerdown="onPointerdown(row, $event)"
        @contextmenu.prevent="openMenu(row, $event)"
      >
        <span :style="markBox">
          <Icon
            v-if="row.state === 'needs-you'"
            :name="STATUS_GLYPH['needs-you']"
            :size="MARK"
            :stroke-width="2.25"
            :style="{ color: 'var(--attn-loud)' }"
          />
          <Icon v-else :name="AGENT_ICON" :size="MARK" :style="{ color: markColour(row.state) }" />
        </span>
        <span :style="captionBoxStyle(row)">
          <input
            v-if="renaming === agentKey(row)"
            :ref="(el) => (renameField = el)"
            v-model="renameDraft"
            :maxlength="AGENT_NAME_MAX"
            :style="renameStyle"
            aria-label="Agent name"
            @keydown.enter="onRenameEnter"
            @keydown.esc.prevent="cancelRename"
            @blur="commitRename"
            @click.stop
            @pointerdown.stop
          />
          <template v-else>
            <span v-if="row.label" :style="labelStyle">{{ row.label }}</span>
            <span v-if="row.tasks?.length" :style="idsStyle">{{ row.tasks.join(', ') }}</span>
          </template>
        </span>
        <span :style="{ flex: 1 }" />
        <span :style="[elapsedStyle, { color: row.state === 'needs-you' ? 'var(--attn-loud)' : 'var(--text-muted)' }]">
          {{ row.elapsed }}
        </span>
        <!-- A pinned row draws the pin where the cross would be: the person
             asked for a row that cannot be closed, and the honest way to say so
             is to leave the verb out and offer Unpin in the menu instead. -->
        <span v-if="pinnedRow(row)" :style="pinBox">
          <Icon name="pin" :size="13" />
        </span>
        <!-- A row that is still starting has no process behind it to take away,
             and the id it carries is this window's own rather than the worker's,
             so removing it would ask about a session nobody has. Disabled rather
             than left out: the button is the widest thing on the row, and a row
             that grew one a second after appearing would shift everything on it
             sideways exactly as somebody started reading it. -->
        <IconButton
          v-else
          icon="x"
          size="sm"
          label="Remove agent"
          :disabled="!!row.starting"
          @click.stop="$emit('remove', row.id)"
        />
      </div>
      <div v-if="empty" :style="{ padding: 'var(--space-5)', color: 'var(--text-muted)', font: 'var(--weight-regular) var(--text-xs)/1.5 var(--font-sans)' }">
        No agents running. Right-click a project to start one.
      </div>
    </div>
    <PointerMenu ref="menu" :items="items" :width="AGENT_MENU_W" @select="pick" @close="menuFor = null" />
  </div>
</template>
