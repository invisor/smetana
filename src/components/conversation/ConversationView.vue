<script setup>
/* A driven session, whole: its identity, its journal, the question it is
   waiting on and the field it is answered in.

   This is the second component in `src/` that imports a store, and it is the
   analogue of the first: `terminal/TerminalView.vue` is a PTY session's panel
   and this is a driven one's. What it takes from the store is a conversation
   and four verbs (`attach`/`detach`, `sendMessage`, `answerQuestion`,
   `stopConversation`); everything that decides what a session *is* stays in
   `stores/conversation.js`, and nothing here knows Tauri exists.

   **What this file does not hold is the two rules about the journal itself.**
   `journal.js` beside it turns events into rows — the fold of a tool call
   together with its result, and the one translation of the wire's `tokens_in`,
   `tokens_out` and `cost_usd` — and says which states count as a turn in
   flight. Both are whole rules with a test each, and a rule inside a `.vue`
   file is a rule nothing in this repository can read; that module's header
   carries the reasoning, including what a silent failure of either would look
   like on screen.

   **The scroll rule is the one non-obvious thing left in this file**, and it is
   written where it is enforced, on `stick` below. */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import AgentMessage from './AgentMessage.vue'
import Composer from './Composer.vue'
import PermissionRequest from './PermissionRequest.vue'
import Reasoning from './Reasoning.vue'
import ToolCall from './ToolCall.vue'
import TurnResult from './TurnResult.vue'
import UserMessage from './UserMessage.vue'
import EmptyState from '../core/EmptyState.vue'
import Icon from '../core/Icon.vue'
import StatusBadge from '../status/StatusBadge.vue'
import { isBusy, journalRows } from './journal.js'
import { basename } from '../../paths.js'
/* Three stores beside the conversation's own, and each is here because this
   component is the one that has to answer rather than raise.

   `openExternal` is the sharpest of the three, and it goes against
   `kanban/TaskInspector.vue`, which raises `open` for the view to bind.
   Deliberately: that panel imports no store at all, while this one already
   does, and the failure the other way round is silent — `Markdown` re-emits an
   href at every level of its tree, and a drawer who binds `:text` and forgets
   `@open` ships an agent's prose with links that do nothing, which no test in
   this project can catch. The panel that owns the session owns its links.

   The other two are the header's, and neither is on the wire: `session_attach`
   answers with the journal, its sequence number and the state, and nothing
   else. */
import { agentLabel } from '../../stores/agents.js'
import { openExternal } from '../../stores/app.js'
import { settings } from '../../stores/settings.js'
import {
  answerQuestion,
  attach,
  conversationFor,
  conversationState,
  detach,
  sendMessage,
  statusOf,
  stopConversation
} from '../../stores/conversation.js'

/* Which session this panel draws, handed in rather than read from the store —
   the same split `TerminalView.vue`'s own prop is written up under, and for the
   same reason: a panel that read a selection of its own would be a second
   answer to "which session is on screen" beside the one the centre already
   has. `null` is a panel with nothing behind it, which draws the empty state
   rather than nothing at all. */
const props = defineProps({
  sessionId: { type: [String, Number], default: null }
})

/* The record this panel is drawing.

   Taken in `setup` and in a watcher, never in a template: `conversationFor`
   makes the record if this window has none, and making state as a side effect
   of a read is a write during a render. A `shallowRef` because what is held is
   already reactive — the store's own object — and re-wrapping it would only
   make a second proxy of the same thing. */
const held = shallowRef(null)

/* The session this panel is attached to right now, which is also the one it
   detaches on unmount. Deliberately not reactive: nothing draws it. */
let attached = null

function open(id) {
  if (attached !== null) {
    detach(attached)
    attached = null
  }
  if (id === null || id === undefined) {
    held.value = null
    return
  }
  held.value = conversationFor(id)
  attached = id
  /* A panel that has just opened belongs at the end of its journal, whatever
     the last one was scrolled to. */
  stick = true
  /* And it carries nothing over from the session it was last pointed at. The
     draft is deliberately the other way — the store keys one per session, so it
     comes back with the conversation it was typed into — while these are held
     here and would otherwise be attached to somebody else's next message.
     Unreachable while nothing fills the list; it is one line now and a defect
     the day drops are wired. */
  attachments.value = []
  attach(id)
}

onMounted(() => open(props.sessionId))
watch(() => props.sessionId, open)
onBeforeUnmount(() => {
  if (attached !== null) detach(attached)
})

/* The journal as rows to draw — `journal.js`, which is where the fold and the
   translation are written and tested. */
const rows = computed(() => journalRows(held.value?.events ?? []))

/* The question the session is waiting on, derived by the store and never stored
   there — see its own note. Drawn at the foot of the panel rather than in the
   journal, because it is the one thing here that is a control. */
const question = computed(() => held.value?.question ?? null)

/* The last refusal, if it is this session's — see `refusal` below for why the
   test is on the session rather than on there being one at all. */
const ourRefusal = computed(() =>
  conversationState.lastError?.session === props.sessionId
    ? conversationState.lastError.text
    : ''
)

const state = computed(() => held.value?.state ?? 'starting')
const busy = computed(() => isBusy(state.value))

/* Who is on the other end. None of this is on the wire — `session_attach`
   answers with the journal, the sequence number and the state, and nothing
   else — so it is read from the front end's own settings, where the same three
   values decided what was spawned: `Intent::Bare` takes the `Default` role,
   which is the root pair (`settings::role_model` over `agents::role_of`).

   There is deliberately **no glyph for the agent's brand**. `core/icons.js` is
   lucide and holds no Claude or Codex mark, and vendoring one would be a third
   exception to "no pictures" after the app icon and the Catppuccin file icons.
   The label carries it, and it works for the next harness on the day it is
   added.

   An empty model is the harness choosing for itself, which is what an unset
   `model` in settings means, so nothing is drawn rather than an empty gap. */
const label = computed(() => agentLabel(settings.agent))
const model = computed(() => settings.model)
const folder = computed(() => (settings.activeProject ? basename(settings.activeProject) : ''))

/* The activity strip's own `waiting` sentence — the same label the bar
   already reads, put to the one other sentence this panel says on its
   behalf. `TurnResult.vue`'s own header carries the rest of the strip's
   reasoning; this is the one word it needs that only the store can give. */
const waitingLabel = computed(() => `${label.value} is thinking`)

/* `session::model::SessionState` in this design system's words, from the store
   for the reason the terminal's own translation lives in `terminals.js`. */
const status = computed(() => statusOf(state.value))

const viewport = ref(null)

/* **Stuck to the end while the person is already at the end, and loose the
   moment they scroll up.** Re-sticking happens when they scroll back down, and
   never on a new event: a panel that scrolled on every arrival is unreadable
   during a long turn, which is precisely when somebody is reading it.

   Not reactive, because nothing draws it — it is remembered between a scroll
   and the next batch of events and nothing else. */
let stick = true

/* How close to the end still counts as being at it. A pixel of slack for the
   sub-pixel arithmetic `scrollHeight - scrollTop - clientHeight` comes out of
   on a fractional device ratio, where the three do not add up exactly and a
   panel scrolled all the way down measures a fraction short of its own end.
   Not a design value and deliberately not a token: nothing about it is drawn. */
const SLACK = 2

function onScroll() {
  const el = viewport.value
  if (!el) return
  stick = el.scrollHeight - el.scrollTop - el.clientHeight <= SLACK
}

function toEnd() {
  const el = viewport.value
  if (el) el.scrollTop = el.scrollHeight
}

/* After the rows have been laid out and not before: `scrollHeight` a tick
   earlier is the height of the journal without whatever has just arrived in
   it. */
async function follow() {
  if (!stick) return
  await nextTick()
  toEnd()
}

watch([rows, question], follow)

/* The journal's own box changing size is the other half of the same rule, and
   it is the composer that changes it: a field growing to six rows takes that
   much off the bottom of the journal, and a panel that was at its end would
   quietly no longer be. The observer costs one line and covers the window being
   resized with it. */
let sizes = null
onMounted(() => {
  sizes = new ResizeObserver(() => {
    if (stick) toEnd()
  })
  if (viewport.value) sizes.observe(viewport.value)
})
onBeforeUnmount(() => sizes?.disconnect())

/* What is going with the next message, and nothing puts anything in it yet.

   The plan asks for files dropped on this panel to land here. They do not: a
   drop never reaches the webview — Tauri reports it against the *window* — and
   the two subscriptions that exist for that event both belong elsewhere,
   `watchSessionDrops` to the terminal store this subsystem must not depend on
   and `watchDrops` to the attachment store, which lives in the New task
   window's webview precisely so that no drop is heard twice
   (`.claude/rules/attachments.md`). A third copy of that lifecycle in here
   would be the thing both of those notes warn against, so the list stays a prop
   of `Composer` with removable chips, and filling it is its own task. */
const attachments = ref([])

async function send() {
  const record = held.value
  if (!record) return
  await sendMessage(props.sessionId, record.draft, [...attachments.value])
  /* The files go with the words, and only when the words went. The store clears
     a draft on the way out of a call that answered and never before — a send
     that failed keeps everything a person wrote, and that has to include what
     they attached, which exists nowhere else either. `lastError` is the one
     thing that says which of the two happened, since `sendMessage` reports
     rather than throws. */
  if (!conversationState.lastError) attachments.value = []
}

const stop = () => stopConversation(props.sessionId)
const answer = (decision) => answerQuestion(props.sessionId, question.value.id, decision)

const root = {
  display: 'flex',
  flexDirection: 'column',
  flex: 1,
  minWidth: 0,
  minHeight: 0,
  background: 'var(--surface)'
}

/* The session's identity, in the bar the pane's own header is: the agent, the
   model, the folder and where the session stands. Identifiers, so mono. */
const bar = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  flex: '0 0 auto',
  height: 'var(--scope-bar-h)',
  padding: '0 var(--panel-pad)',
  /* The last line of defence under the rule below: with only the two muted
     spans able to give, a panel narrower than the badge and the label together
     would push the bar wider than the pane and put a horizontal scrollbar under
     the whole column. Clipped instead — the parts are in the order they are
     most worth keeping. */
  overflow: 'hidden',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  background: 'var(--surface)'
}

/* The label, and the two properties that decide what gives way when the bar
   runs out of room.

   **The identity is the wrong thing to shrink**, and without the `0 0` in that
   `flex` it is exactly what flexbox picks: every child of this row is shrinkable by
   default, so a long model and a long folder squeezed "Claude Code" from one
   73px line into a 40px box and it wrapped to two — 22px of text in a bar that
   is 24px tall in compact. The meta spans below have `minWidth: 0` and an
   ellipsis for this, and now they are the only ones that can give, which is
   what that ellipsis was for. `nowrap` is the second half: a label that cannot
   shrink must also not break, or a harness with a two-word name wraps on a
   narrow panel instead. The badge at the head of the bar is held the same way,
   from the template, since it is a component and its own style object is its
   own. */
const agentName = {
  flex: '0 0 auto',
  whiteSpace: 'nowrap',
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)'
}

const quiet = {
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)'
}

/* A meta pair: the separator and the value it introduces, in one box so that
   what gives way gives way whole. Measured in Chrome at 200px of panel, a
   separator that shrank on its own account was left behind pointing at a value
   that had ellipsised away to nothing — "Claude Code · ·". The pair is the only
   shrinkable thing in the bar, and it takes its own separator with it.

   The separator is its own span inside it because it is the one thing in the
   bar that is not a value: it is drawn in the border colour rather than the
   muted text one, and the bar sets no font, so it says its own. */
const metaPair = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  flex: '0 1 auto',
  minWidth: 0,
  overflow: 'hidden'
}

const sep = {
  flex: '0 0 auto',
  color: 'var(--border-strong)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}

const journal = {
  flex: 1,
  minWidth: 0,
  minHeight: 0,
  overflowY: 'auto',
  overflowX: 'hidden',
  /* The end of a short conversation sits at the top of the panel rather than
     floating in the middle of it: a journal is read from its first line down,
     and centring it would move every row as the second one arrived. */
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'stretch'
}

/* Whatever is not the journal: the open question, the last refusal, the field.
   One box, so the journal's scroll ends above all three. */
const foot = {
  flex: '0 0 auto',
  display: 'flex',
  flexDirection: 'column',
  borderTop: 'var(--border-w) solid var(--border-subtle)',
  background: 'var(--surface)'
}

const questionPad = { padding: 'var(--panel-pad) var(--panel-pad) 0' }

/* A bare `error` row — an `Error` event `journal.js` found no open turn to
   fold into, which is the worker saying a message never reached the agent at
   all rather than a turn that opened and then failed. A turn's own failure is
   the activity strip's `failed` moment now (`TurnResult.vue`), drawn where
   `row.kind === 'activity'` is below; this is what is left over for the one
   case that is not a turn ending. Prose, so sans; the failed hue and the
   status glyph, so it is not mistaken for the agent's own words. */
const failure = {
  display: 'flex',
  alignItems: 'flex-start',
  gap: 'var(--space-3)',
  padding: 'var(--space-4) var(--panel-pad)',
  color: 'var(--status-failed-fg)',
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)'
}

/* The store's sentence for a person, drawn as a line inside the panel rather
   than as a toast in the corner — but **only the sentences about the session
   this panel is holding**. The store carries the session a refusal belongs to
   for exactly this: a spawn refusal for a session that was never made has
   nothing to do with a conversation somebody is reading, and this line has no
   dismiss and clears only on the next call that answers, so it would have stood
   at the foot of a healthy conversation for as long as the window lived.
   Everything this refuses is drawn by the toast in `DesktopApp.vue`'s corner,
   which is the reader for whatever has no panel of its own. */
const refusal = {
  display: 'flex',
  alignItems: 'flex-start',
  gap: 'var(--space-3)',
  padding: 'var(--space-4) var(--panel-pad) 0',
  color: 'var(--status-failed-fg)',
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)'
}
</script>

<template>
  <div :style="root">
    <div :style="bar">
      <StatusBadge :status="status" size="sm" :style="{ flex: '0 0 auto' }" />
      <span :style="agentName">{{ label }}</span>
      <span v-if="model" :style="metaPair">
        <span :style="sep">·</span>
        <span :style="quiet">{{ model }}</span>
      </span>
      <span v-if="folder" :style="metaPair">
        <span :style="sep">·</span>
        <span :style="quiet">{{ folder }}</span>
      </span>
    </div>

    <div ref="viewport" :style="journal" @scroll="onScroll">
      <EmptyState
        v-if="!sessionId"
        icon="message-square"
        title="No conversation"
        description="Start an agent to open one."
      />
      <!-- One sentence and not two. `starting` used to draw "Starting the
           agent" here, on the assumption that a session says something of its
           own accord and the panel is waiting on it. It is the other way round:
           a driven session is spawned with `--input-format stream-json` and
           waits on the person, so every empty panel this app can open is one
           nobody has spoken into yet. The sentence that told somebody to wait
           was the visible half of the deadlock `journal.js`'s `BUSY` describes
           — it named the agent as the one still to move while the composer
           refused the only move there was. -->
      <EmptyState
        v-else-if="!rows.length"
        icon="message-square"
        title="Nothing said yet"
        description="Send a message to begin."
      />
      <template v-else>
        <template v-for="row in rows" :key="row.key">
          <UserMessage
            v-if="row.kind === 'user'"
            :text="row.text"
            :attachments="row.attachments"
            @open="openExternal"
          />
          <AgentMessage v-else-if="row.kind === 'agent'" :text="row.text" @open="openExternal" />
          <Reasoning
            v-else-if="row.kind === 'reasoning'"
            :text="row.text"
            :ms="row.ms"
            @open="openExternal"
          />
          <ToolCall
            v-else-if="row.kind === 'tool'"
            :name="row.name"
            :detail="row.detail"
            :result="row.result"
          />
          <TurnResult
            v-else-if="row.kind === 'activity'"
            :state="row.state"
            :label="waitingLabel"
            :started-at="row.startedAt"
            :text="row.text"
            :tokens-in="row.tokensIn"
            :tokens-out="row.tokensOut"
            :cost-usd="row.costUsd"
            :ms="row.ms"
          />
          <div v-else-if="row.kind === 'error'" :style="failure">
            <Icon name="x" :size="13" :stroke-width="2.25" />
            <span>{{ row.text }}</span>
          </div>
        </template>
      </template>
    </div>

    <div :style="foot">
      <div v-if="question" :style="questionPad">
        <PermissionRequest
          :tool="question.tool"
          :detail="question.detail"
          :options="question.options"
          @answer="answer"
        />
      </div>
      <div v-if="ourRefusal" :style="refusal">
        <Icon name="x" :size="13" :stroke-width="2.25" />
        <span>{{ ourRefusal }}</span>
      </div>
      <Composer
        v-if="held"
        v-model="held.draft"
        :attachments="attachments"
        :busy="busy"
        @update:attachments="attachments = $event"
        @send="send"
        @stop="stop"
      />
    </div>
  </div>
</template>
