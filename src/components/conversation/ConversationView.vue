<script setup>
/* A driven session, whole: its identity, its journal, the question it is
   waiting on and the field it is answered in.

   This is the second component in `src/` that imports a store, and it is the
   analogue of the first: `terminal/TerminalView.vue` is a PTY session's panel
   and this is a driven one's. What it takes from the store is a conversation
   and four verbs (`attach`/`detach`, `sendMessage`, `answerQuestion`,
   `stopConversation`); everything that decides what a session *is* stays in
   `stores/conversation.js`, and nothing here knows Tauri exists.

   **The events are translated here and nowhere else.** `session::model::Event`
   carries its kind's fields flattened in beside a kebab-case `kind`, and those
   fields are *not* renamed on the way out: `tokens_in`, `tokens_out`,
   `cost_usd`. Every numeric prop of `TurnResult` has a default, so an event
   handed over raw — `v-bind="event"`, or `:tokens-in="event.tokens_in"` read
   off a store with no translation — would draw `0 in · 0 out · 0 ms` silently,
   with no warning from Vue and nothing on screen to say the numbers are not the
   session's. The one place the wire's names appear is `rows` below.

   **A kind this panel does not know draws nothing.** The chain over the kinds
   is closed and has no fallback: that is `session::model::EventKind`'s own rule
   one layer up ("an event type a driver does not recognise produces no event"),
   and the reason is the same — a missing row costs a person nothing the
   harness's own logs do not still hold, while a wall of raw protocol costs them
   the panel. `turn-start` and the two permission kinds are deliberately among
   the ones that draw nothing: a turn's start is said by the message under it,
   and a question is drawn from `held.question` at the foot of the panel, where
   it is answered, rather than twice.

   **The scroll rule is the one non-obvious thing in this file**, and it is
   written where it is enforced, on `atEnd` below. */
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
  attach(id)
}

onMounted(() => open(props.sessionId))
watch(() => props.sessionId, open)
onBeforeUnmount(() => {
  if (attached !== null) detach(attached)
})

/* The journal as rows to draw: one per event, except that a tool call and its
   result are one row folded together by the id they share.

   The fold is why this is a list built in one pass rather than a `v-for` with a
   branch inside it: `tool-result` carries no name and no detail of its own, and
   drawing it on a line by itself would split one thing the agent did across two
   rows. A result whose call is not in this journal draws nothing — that is a
   trimmed journal, and `ToolCall` has nothing to put in its row.

   `result: null` is the running state and is what `ToolCall` is written
   against: it draws the live glyph rather than guessing at an outcome. */
const rows = computed(() => {
  const out = []
  const calls = new Map()
  for (const event of held.value?.events ?? []) {
    if (event.kind === 'user-message') {
      out.push({
        key: event.seq,
        kind: 'user',
        text: event.text,
        attachments: event.attachments ?? []
      })
    } else if (event.kind === 'text') {
      out.push({ key: event.seq, kind: 'agent', text: event.text })
    } else if (event.kind === 'reasoning') {
      out.push({ key: event.seq, kind: 'reasoning', text: event.text })
    } else if (event.kind === 'tool-use') {
      const row = { key: event.seq, kind: 'tool', name: event.name, detail: event.detail, result: null }
      calls.set(event.id, row)
      out.push(row)
    } else if (event.kind === 'tool-result') {
      const row = calls.get(event.id)
      if (row) row.result = { ok: event.ok, summary: event.summary }
    } else if (event.kind === 'result') {
      /* The four names the wire actually uses. See the header: this is the
         translation, and there is no second copy of it anywhere. */
      out.push({
        key: event.seq,
        kind: 'result',
        tokensIn: event.tokens_in,
        tokensOut: event.tokens_out,
        costUsd: event.cost_usd ?? null,
        ms: event.ms
      })
    } else if (event.kind === 'error') {
      out.push({ key: event.seq, kind: 'error', text: event.text })
    }
  }
  return out
})

/* The question the session is waiting on, derived by the store and never stored
   there — see its own note. Drawn at the foot of the panel rather than in the
   journal, because it is the one thing here that is a control. */
const question = computed(() => held.value?.question ?? null)

const state = computed(() => held.value?.state ?? 'starting')
/* Whether a turn is in flight, which is what turns the composer's one button
   into Stop. A session waiting on a permission counts: the turn is open and the
   thing to do about it is the card above the field, not another message. */
const busy = computed(() => ['starting', 'running', 'needs-you'].includes(state.value))

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
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  background: 'var(--surface)'
}

const agentName = {
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

/* The one thing in the bar that is not a value: the separator between them.
   The bar sets no font of its own, so this says its own. */
const sep = {
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

/* An `Error` event, which is the worker saying what happened where an answer
   would have gone — a message that did not reach the agent, a line the harness
   wrote to stderr. Prose, so sans; the failed hue and the status glyph, so it
   is not mistaken for the agent's own words. */
const failure = {
  display: 'flex',
  alignItems: 'flex-start',
  gap: 'var(--space-3)',
  padding: 'var(--space-4) var(--panel-pad)',
  color: 'var(--status-failed-fg)',
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)'
}

/* The store's one sentence for a person, drawn as a line inside the panel
   rather than as a toast in the corner: what it is about is the thing somebody
   just pressed here, and it is cleared by the next call that answers. */
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
      <StatusBadge :status="status" size="sm" />
      <span :style="agentName">{{ label }}</span>
      <span v-if="model" :style="sep">·</span>
      <span v-if="model" :style="quiet">{{ model }}</span>
      <span v-if="folder" :style="sep">·</span>
      <span v-if="folder" :style="quiet">{{ folder }}</span>
    </div>

    <div ref="viewport" :style="journal" @scroll="onScroll">
      <EmptyState
        v-if="!sessionId"
        icon="message-square"
        title="No conversation"
        description="Start an agent to open one."
      />
      <EmptyState
        v-else-if="!rows.length"
        icon="message-square"
        :title="state === 'starting' ? 'Starting the agent' : 'Nothing said yet'"
        :description="
          state === 'starting'
            ? 'The first words appear here as soon as it has any.'
            : 'Send a message to begin.'
        "
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
          <Reasoning v-else-if="row.kind === 'reasoning'" :text="row.text" @open="openExternal" />
          <ToolCall
            v-else-if="row.kind === 'tool'"
            :name="row.name"
            :detail="row.detail"
            :result="row.result"
          />
          <TurnResult
            v-else-if="row.kind === 'result'"
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
      <div v-if="conversationState.lastError" :style="refusal">
        <Icon name="x" :size="13" :stroke-width="2.25" />
        <span>{{ conversationState.lastError }}</span>
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
