# Stage 2 — the conversation panel

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `Bare` Claude Code session is read and answered in the app's own components — the journal drawn, markdown rendered, permissions answered, messages sent — with no terminal anywhere on the panel.

**Architecture:** A new store `src/stores/conversation.js` beside `terminals.js`, and a new component group `src/components/conversation/`. `terminals.js` and `components/terminal/` are not touched: a `Bare` session opens a tab of the new kind, every other intent keeps the terminal, and the two coexist until the last intent moves.

**Tech Stack:** Vue 3, the shared markdown parser and components in `src/components/markdown/`, vitest.

**Spec:** `docs/superpowers/specs/2026-09-10-conversation-ui-design.md`
**Depends on:** `docs/superpowers/plans/2026-09-10-conversation-session-worker.md` — every command and event named below is that plan's Task 8.

## Global Constraints

- Comments, test names, assertion messages and `console.*` text in **English**, without exception. Commit messages in Russian.
- **Every visual value is a `var(--token)` reference inside a computed style object bound with `:style`.** No `<style>` block, no class, no hardcoded colour, radius, spacing or font. If a token does not exist for what you need, that is a design-system question, not a licence to write `#hex` or `8px`.
- Sentence case in UI copy. Identifiers — paths, commands, tool names, token counts — in `var(--font-mono)`; prose in sans.
- No gradients, glass, blur or emoji. No native right-click menu.
- Every new component is exported from `src/components/index.js` and added to `src/views/Gallery.vue`. There is no component test runner in this project and none is invented here: `?view=gallery` in all four theme × density combinations is the check.
- Components import siblings by relative path; product code imports from `index.js`.
- No new dependency, and no second markdown parser. Prose is parsed and drawn by `src/components/markdown/`, which the task inspector already uses; see Task 1.
- Do not touch `src/stores/terminals.js` or `src/components/terminal/`.

---

## File structure

| file | responsibility |
|---|---|
| `src/stores/conversation.js` | the only new file in `src/` that knows Tauri exists: journals per session, the draft, the open question |
| `src/components/conversation/ConversationView.vue` | the journal, and the scroll rule |
| `src/components/conversation/AgentMessage.vue` | a paragraph of the agent's prose |
| `src/components/conversation/UserMessage.vue` | the person's words and attachments |
| `src/components/conversation/ToolCall.vue` | name, one line of detail, result state |
| `src/components/conversation/Reasoning.vue` | folded, quiet |
| `src/components/conversation/PermissionRequest.vue` | the one loud thing |
| `src/components/conversation/TurnResult.vue` | tokens, cost, duration |
| `src/components/conversation/Composer.vue` | input, attachments, send, stop |
| `tests/stores/conversation.test.js` | the store, through `mockIPC` |

---

## Task 1: The markdown tree — already in the tree

Nothing to write. Markdown is parsed once for the whole front end by
`src/components/markdown/markdown.js`, and drawn by `Markdown.vue` and
`MarkdownInline.vue` beside it. The module was written for the task inspector and
moved out of `src/components/kanban/` into its own group by smetana-738c, exactly
so that this panel could use it: one parser and one pair of components, rather
than two grammars diverging in silence.

Import the components from `src/components/index.js` (`Markdown`,
`MarkdownInline`) as product code does, and `parseMarkdown` from the module
itself if a tree is wanted without drawing it. No dependency is added: the parser
is handwritten and has none.

The tree it returns — the contract the components below are written against:

- `{ type: 'paragraph', children: Inline[] }`
- `{ type: 'heading', level, children: Inline[] }`
- `{ type: 'list', ordered, start, items: { checked: boolean|null, blocks: Node[] }[] }`
- `{ type: 'code', lang: string|null, text }`
- `{ type: 'quote', blocks: Node[] }`
- `{ type: 'rule' }`

and an `Inline` is one of `{ type: 'text', value }`,
`{ type: 'strong'|'em', children }`, `{ type: 'code', value }`,
`{ type: 'link', href, children }`.

Two behaviours of that module this plan relies on: no character of the source
disappears — an unrecognised construct and any HTML tag come back as ordinary
text — and a `link` node is produced only for `http` and `https`, which is what
makes an agent's output safe to hand to `openExternal`. `tests/components/markdown/markdown.test.js`
covers all of it; a defect found there is its own task, not a fix inside this plan.
---

## Task 2: The store

**Files:**
- Create: `src/stores/conversation.js`, `tests/stores/conversation.test.js`
- Modify: `src/stores/mockBackend.js` (answer the new read commands, reject the writes)

**Interfaces:**
- Consumes: `session_start`, `session_attach`, `session_since`, `session_send`, `session_answer`, `session_stop`, and the `session:events` / `session:state` events — all from Stage 1 Task 8.
- Produces: `initConversation()`, `attach(id)` and `detach(id)`, `conversationFor(id)` → `{ events, state, question, draft }` (reactive), `startConversation(project, intent)`, `sendMessage(id, text, attachments)`, `answerQuestion(id, questionId, decision)`, `stopConversation(id)`, and `conversationState` holding `lastError`.

- [ ] **Step 1: Write the failing test**

```js
import { beforeEach, describe, expect, it } from 'vitest'
import { loadStores } from '../support/stores.js'

const event = (seq, kind, over = {}) => ({ seq, at: '2026-09-10T12:00:00Z', kind, ...over })

const text = (seq, body) => event(seq, 'text', { text: body })

const permission = (seq, id = 'q1') =>
  event(seq, 'permission', { id, tool: 'Bash', detail: 'rm -rf /tmp/x', options: ['allow', 'deny'] })

describe('the conversation store', () => {
  let stores

  beforeEach(async () => {
    stores = await loadStores()
  })

  it('takes the snapshot the worker hands back on attach', async () => {
    stores.mockCommand('session_attach', () => ({
      events: [text(1, 'hello')],
      seq: 1,
      state: 'ready'
    }))
    await stores.conversation.attach(1)
    expect(stores.conversation.conversationFor(1).events).toHaveLength(1)
    expect(stores.conversation.conversationFor(1).state).toBe('ready')
  })

  it('appends events that arrive in sequence', async () => {
    stores.mockCommand('session_attach', () => ({ events: [text(1, 'a')], seq: 1, state: 'running' }))
    await stores.conversation.attach(1)
    await stores.emit('session:events', { id: 1, events: [text(2, 'b')] })
    expect(stores.conversation.conversationFor(1).events.map((e) => e.text)).toEqual(['a', 'b'])
  })

  /* The whole reason `seq` exists. A gap means the worker trimmed away what
     this window never saw, and drawing the tail would be drawing a
     conversation with a silent hole in the middle of it. */
  it('re-attaches rather than drawing a gap when an event arrives out of sequence', async () => {
    let attaches = 0
    stores.mockCommand('session_attach', () => {
      attaches += 1
      return { events: [text(9, 'fresh')], seq: 9, state: 'running' }
    })
    await stores.conversation.attach(1)
    await stores.emit('session:events', { id: 1, events: [text(7, 'from the future')] })
    expect(attaches).toBe(2)
    expect(stores.conversation.conversationFor(1).events.map((e) => e.text)).toEqual(['fresh'])
  })

  it('ignores events for a session this window is not holding', async () => {
    stores.mockCommand('session_attach', () => ({ events: [], seq: 0, state: 'starting' }))
    await stores.conversation.attach(1)
    await stores.emit('session:events', { id: 2, events: [text(1, 'somebody else')] })
    expect(stores.conversation.conversationFor(1).events).toHaveLength(0)
  })

  it('holds the unanswered question where a component can find it', async () => {
    stores.mockCommand('session_attach', () => ({ events: [permission(1)], seq: 1, state: 'needs-you' }))
    await stores.conversation.attach(1)
    expect(stores.conversation.conversationFor(1).question).toMatchObject({ id: 'q1', tool: 'Bash' })
  })

  it('lets the question go once it has been answered', async () => {
    stores.mockCommand('session_attach', () => ({ events: [permission(1)], seq: 1, state: 'needs-you' }))
    await stores.conversation.attach(1)
    await stores.emit('session:events', {
      id: 1,
      events: [event(2, 'permission-answered', { id: 'q1', decision: 'allow' })]
    })
    expect(stores.conversation.conversationFor(1).question).toBe(null)
  })

  it('holds only the newest question when two are somehow open', async () => {
    stores.mockCommand('session_attach', () => ({
      events: [permission(1, 'q1'), permission(2, 'q2')],
      seq: 2,
      state: 'needs-you'
    }))
    await stores.conversation.attach(1)
    expect(stores.conversation.conversationFor(1).question.id).toBe('q2')
  })

  it('sends a message with its attachments', async () => {
    let sent = null
    stores.mockCommand('session_attach', () => ({ events: [], seq: 0, state: 'ready' }))
    stores.mockCommand('session_send', (args) => {
      sent = args
      return null
    })
    await stores.conversation.attach(1)
    await stores.conversation.sendMessage(1, 'hello', ['/tmp/a.png'])
    expect(sent).toEqual({ id: 1, text: 'hello', attachments: ['/tmp/a.png'] })
  })

  it('refuses to send nothing at all', async () => {
    let calls = 0
    stores.mockCommand('session_attach', () => ({ events: [], seq: 0, state: 'ready' }))
    stores.mockCommand('session_send', () => {
      calls += 1
      return null
    })
    await stores.conversation.attach(1)
    await stores.conversation.sendMessage(1, '   ', [])
    expect(calls).toBe(0)
  })

  it('clears the draft once the message is away, and not before', async () => {
    stores.mockCommand('session_attach', () => ({ events: [], seq: 0, state: 'ready' }))
    stores.mockCommand('session_send', () => {
      throw new Error('the worker is not running')
    })
    await stores.conversation.attach(1)
    stores.conversation.conversationFor(1).draft = 'unsent words'
    await stores.conversation.sendMessage(1, 'unsent words', [])
    expect(stores.conversation.conversationFor(1).draft).toBe('unsent words')
    expect(stores.conversation.conversationState.lastError).toBeTruthy()
  })

  it('carries a failure to answer into lastError rather than swallowing it', async () => {
    stores.mockCommand('session_attach', () => ({ events: [permission(1)], seq: 1, state: 'needs-you' }))
    stores.mockCommand('session_answer', () => {
      throw new Error('there is no question q1 waiting for an answer')
    })
    await stores.conversation.attach(1)
    await stores.conversation.answerQuestion(1, 'q1', 'allow')
    expect(stores.conversation.conversationState.lastError).toContain('q1')
  })

  it('follows the state the worker reports', async () => {
    stores.mockCommand('session_attach', () => ({ events: [], seq: 0, state: 'starting' }))
    await stores.conversation.attach(1)
    await stores.emit('session:state', { id: 1, state: 'needs-you' })
    expect(stores.conversation.conversationFor(1).state).toBe('needs-you')
  })
})
```

Read `tests/support/stores.js` before writing this — it explains why the module graph is rebuilt per test, and it is where `mockCommand` and `emit` come from. If those two helpers are named differently there, use its names; do not add helpers of your own.

- [ ] **Step 2: Run it and watch it fail**

Run: `npm test -- tests/stores/conversation.test.js`
Expected: FAIL — the store does not exist.

- [ ] **Step 3: Write the store**

Follow `src/stores/terminals.js` for the shape: module-level `reactive` state, an `init` that registers the listeners once, `invoke` wrapped so a rejection lands in `lastError` rather than in an unhandled rejection.

The rules it holds:

- **An event whose `seq` is not exactly one past the last one seen triggers a fresh `session_attach`.** Never patch the gap.
- **`question` is derived, not stored** — the newest `permission` with no matching `permission-answered` after it. A stored copy is a second source of truth for the loudest thing on the screen.
- **The draft survives a failed send.** Clearing it optimistically loses a person's words when the worker is down.
- **Events for an unknown session are dropped**, not buffered. A window that has not attached has nothing to draw them into, and `session_attach` will hand it the snapshot.

- [ ] **Step 4: Teach the mock backend**

In `src/stores/mockBackend.js`, answer `session_attach` with an empty journal and `session_since` with `[]`, and reject `session_start`, `session_send`, `session_answer` and `session_stop` loudly — the same treatment the tracker's writes get, and for the same reason: a write that looked like it worked in a browser would be worse than none.

- [ ] **Step 5: Run the tests**

Run: `npm test -- tests/stores/conversation.test.js`
Expected: PASS, 12 tests.

- [ ] **Step 6: Commit**

```bash
npm test
git add src/stores/ tests/stores/conversation.test.js
git commit -m "feat(conversation): store журнала, вопроса и черновика"
```

---

## Task 3: The quiet components

Five of the eight: everything that is neither loud nor an input. Built together because each is a few dozen lines of the same shape, and a reviewer would not reject one while approving its neighbour.

**Files:**
- Create: `src/components/conversation/AgentMessage.vue`, `UserMessage.vue`, `ToolCall.vue`, `Reasoning.vue`, `TurnResult.vue`
- Modify: `src/components/index.js`, `src/views/Gallery.vue`

**Interfaces:**
- Consumes: `Markdown` from `../markdown/Markdown.vue` (Task 1); `Icon` from `../core/Icon.vue`; `iconFor` from `src/catppuccinIcon.js`.
- Produces: `AgentMessage { text }` and `UserMessage { text, attachments }`, both with `emits: ['open']`; `ToolCall { name, detail, result }` where `result` is `null | { ok, summary }`; `Reasoning { text }`; `TurnResult { tokensIn, tokensOut, costUsd, ms }`.

- [ ] **Step 1: Register the glyphs first**

`Icon` warns in dev for an unregistered name, and `core/icons.js` is the only file that names Lucide. Add whatever these components use — at minimum `chevron-right` (Reasoning's fold), `check` and `x` (a tool's outcome) — if they are not registered already. Check before adding; most are.

- [ ] **Step 2: Write the five**

Each is the shape `src/components/status/StatusBadge.vue` already has: a
`defineProps`, a `computed` returning an object of `var(--token)` references, and a
template binding it with `:style`. Read that file before writing the first of these
— it is the reference, and nothing below departs from its shape.

`AgentMessage` is the shared `Markdown` with the turn's spacing around it — a fenced block inside it draws as that component already draws one, a `<pre>` in `var(--font-mono)`, and syntax highlighting is not this plan's. `UserMessage` sits on `var(--surface-raised)` with a left border in `var(--border-strong)` so the two halves of the conversation are told apart by shape rather than by colour. `ToolCall` is one row: the glyph, the name in mono, the detail in mono and dimmed, and the result's outcome at the end — a tick or a cross with `summary` beside it, and nothing at all while it is still running. A path in `detail` takes its icon from `catppuccinIcon.js`. `Reasoning` is folded by default behind a `chevron-right` and carries `data-attention="quiet"` with `opacity: var(--attn-quiet-opacity)`. `TurnResult` is one dimmed mono line: `120 in · 40 out · $0.031 · 4.2 s`, with the cost omitted when it is `null` rather than drawn as `$0`.

The one thing not to drop while wiring these up: the shared `Markdown` opens no
link itself. It emits `open` with the href and re-emits it upward at every level
of the tree, so `AgentMessage` and `UserMessage` have to declare `emits: ['open']`
and forward it, and whatever draws them binds it to `openExternal` in
`src/stores/app.js` — the way `kanban/TaskInspector.vue` and `views/Gallery.vue`
already do. The desktop opens a link in the person's own browser; a navigation
inside the webview would replace the app. Bind `:text` alone and an agent's prose
ships with links that do nothing, which no test in this project can catch.

- [ ] **Step 3: Export and gallery**

Add all five to `src/components/index.js` under a `// conversation` comment, and a section to `src/views/Gallery.vue` showing each with realistic content: a paragraph with a list and a fenced block, a tool call in each of its three states, a folded and an unfolded reasoning block, a turn result with and without a cost.

- [ ] **Step 4: Check by eye**

```bash
npm run dev
```

Open `http://localhost:5173/?view=gallery` and check the new section in all four combinations:

- `?view=gallery&theme=dark&density=comfortable`
- `?view=gallery&theme=dark&density=compact`
- `?view=gallery&theme=light&density=comfortable`
- `?view=gallery&theme=light&density=compact`

What to look for: nothing changes size between the themes (that would be a hardcoded value), the compact density tightens spacing without changing type colour or radius, and the file-type icon in a `ToolCall` is legible on both grounds.

- [ ] **Step 5: Commit**

```bash
git add src/components/conversation/ src/components/index.js src/views/Gallery.vue src/components/core/icons.js
git commit -m "feat(conversation): тихие компоненты журнала — проза, инструмент, размышление, итог хода"
```

---

## Task 4: The permission request

Its own task because it is the one loud thing on the panel, and the one component a reviewer should be able to reject on its own.

**Files:**
- Create: `src/components/conversation/PermissionRequest.vue`
- Modify: `src/components/index.js`, `src/views/Gallery.vue`

**Interfaces:**
- Produces: `PermissionRequest { tool, detail, options }`, emitting `answer` with one of `'allow' | 'allow-always' | 'deny'`.

- [ ] **Step 1: Write it**

Draw it at `loud`: `statusColors('needs-you')` from `src/components/status/status.js` for the border and the glyph, filled solid, exactly as `StatusBadge` does at that level. The tool's name and the detail in mono — this is the one place where reading the exact command matters, so the detail is not clipped and wraps instead. One `Button` per offered option, the deny one plain rather than red: the colour budget is already spent by the frame around it.

This component is the reason the system's one-or-two-loud-rows-a-screen budget exists, and a session has at most one open question, so the budget holds by construction.

- [ ] **Step 2: Add it to the gallery**

Three cases: `Bash` with a long command that must wrap, a `Write` with a file path, and one with only `allow` and `deny` on offer so the third button's absence is visible.

- [ ] **Step 3: Check by eye**

The four combinations again, and one thing in particular: at `light` the filled loud surface must still carry legible text, since it inverts to `var(--surface-raised)`.

- [ ] **Step 4: Commit**

```bash
git add src/components/conversation/ src/components/index.js src/views/Gallery.vue
git commit -m "feat(conversation): запрос разрешения — единственное громкое место панели"
```

---

## Task 5: The composer

**Files:**
- Create: `src/components/conversation/Composer.vue`
- Modify: `src/components/index.js`, `src/views/Gallery.vue`

**Interfaces:**
- Produces: `Composer { modelValue, attachments, busy }` with `v-model` on the text, emitting `send`, `stop` and `update:attachments`.

- [ ] **Step 1: Write it**

A growing textarea to a ceiling, then it scrolls. Enter sends; shift+Enter is a newline. While `busy`, the send button becomes stop — one control, since a person cannot want both. Attachments show as removable chips carrying `basename` from `src/paths.js` — not a second copy of that function.

Dropped files reach it through the existing attachment store; the drop target and the hit test are the panel's business, exactly as `.claude/rules/attachments.md` describes for the terminal panel today.

- [ ] **Step 2: Gallery, all four combinations, commit**

```bash
git add src/components/conversation/ src/components/index.js src/views/Gallery.vue
git commit -m "feat(conversation): поле ввода с вложениями и одной кнопкой отправки-остановки"
```

---

## Task 6: The panel

**Files:**
- Create: `src/components/conversation/ConversationView.vue`
- Modify: `src/components/index.js`, `src/views/Gallery.vue`, `src/views/DesktopApp.vue`

**Interfaces:**
- Consumes: everything above, plus the store from Task 2.
- Produces: `ConversationView { sessionId }`.

- [ ] **Step 1: Write the view**

It attaches on mount, detaches on unmount, and draws one component per event. The header carries the session's identity — the agent's label in mono, the model, the folder's `basename`, and a `StatusBadge` for the state. There is no glyph for an agent brand and none is invented: `core/icons.js` is lucide and has no Claude or Codex mark, and vendoring one would be a third exception to "no pictures" after the app icon and the Catppuccin file icons.

The scroll rule, which is the only non-obvious thing in the file: **stick to the bottom while the person is already at the bottom, and stop the moment they scroll up.** Re-sticking happens when they scroll back down, never on a new event. Getting this wrong in the other direction — always scrolling — makes the panel unreadable during a long turn.

- [ ] **Step 2: Draw it in the gallery**

`Gallery.vue` renders every exported component once, and this one needs a session. Give it a fixture journal through the mock rather than a live worker — the gallery must work in a browser with no Tauri behind it.

- [ ] **Step 3: Open the tab in the app**

In `DesktopApp.vue`, a `Bare` session started under Claude Code opens a tab drawing `ConversationView`; everything else keeps `TerminalView`. One `v-if` on the session's kind. This is a union of two tab kinds, deliberately not an abstraction over two backends — the terminal is going away, and a seam built to outlive the migration would.

- [ ] **Step 4: Check it against a live agent**

```bash
npm run tauri dev
```

Start a `Bare` session. Expected, in order on the panel: the agent's paragraph, a permission request for the first tool, the tool call and its result once answered, and a turn result at the end. Then check that closing and reopening the tab redraws the whole conversation — the journal is Rust's, and this is what proves it.

- [ ] **Step 5: Run everything and commit**

```bash
npm test
cd src-tauri && cargo test && cd ..
git add src/ 
git commit -m "feat(conversation): панель разговора и её вкладка для сессий Bare"
```

---

## Self-review notes for the executor

- If a component needs a value with no token behind it, stop. That is a design-system question and the answer is not a hex.
- Nothing here may reach for `v-html`, including for "just the code block". The parser exists so that it never has to.
- `terminals.js` and `components/terminal/` are untouched by every task above. Needing to change one is the sign that the tab seam has been misread.
- The gallery entries are not optional paperwork: they are the only test these eight components will ever have.
