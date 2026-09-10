# Stage 2 — the conversation panel

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `Bare` Claude Code session is read and answered in the app's own components — the journal drawn, markdown rendered, permissions answered, messages sent — with no terminal anywhere on the panel.

**Architecture:** A new store `src/stores/conversation.js` beside `terminals.js`, and a new component group `src/components/conversation/`. `terminals.js` and `components/terminal/` are not touched: a `Bare` session opens a tab of the new kind, every other intent keeps the terminal, and the two coexist until the last intent moves.

**Tech Stack:** Vue 3, `@lezer/markdown` (already in the tree under `@codemirror/lang-markdown`), CodeMirror read-only for fenced code, vitest.

**Spec:** `docs/superpowers/specs/2026-09-10-conversation-ui-design.md`
**Depends on:** `docs/superpowers/plans/2026-09-10-conversation-session-worker.md` — every command and event named below is that plan's Task 8.

## Global Constraints

- Comments, test names, assertion messages and `console.*` text in **English**, without exception. Commit messages in Russian.
- **Every visual value is a `var(--token)` reference inside a computed style object bound with `:style`.** No `<style>` block, no class, no hardcoded colour, radius, spacing or font. If a token does not exist for what you need, that is a design-system question, not a licence to write `#hex` or `8px`.
- Sentence case in UI copy. Identifiers — paths, commands, tool names, token counts — in `var(--font-mono)`; prose in sans.
- No gradients, glass, blur or emoji. No native right-click menu.
- Every new component is exported from `src/components/index.js` and added to `src/views/Gallery.vue`. There is no component test runner in this project and none is invented here: `?view=gallery` in all four theme × density combinations is the check.
- Components import siblings by relative path; product code imports from `index.js`.
- No new dependency. `@lezer/markdown` is already installed transitively; add it to `package.json` `dependencies` at its installed version, since importing a transitive dependency directly is how a build breaks on somebody else's `npm install`.
- Do not touch `src/stores/terminals.js` or `src/components/terminal/`.

---

## File structure

| file | responsibility |
|---|---|
| `src/stores/conversation.js` | the only new file in `src/` that knows Tauri exists: journals per session, the draft, the open question |
| `src/components/conversation/markdown.js` | markdown text → a plain tree of nodes; pure, no Vue |
| `src/components/conversation/ConversationView.vue` | the journal, and the scroll rule |
| `src/components/conversation/Markdown.vue` | that tree, drawn |
| `src/components/conversation/AgentMessage.vue` | a paragraph of the agent's prose |
| `src/components/conversation/UserMessage.vue` | the person's words and attachments |
| `src/components/conversation/ToolCall.vue` | name, one line of detail, result state |
| `src/components/conversation/Reasoning.vue` | folded, quiet |
| `src/components/conversation/PermissionRequest.vue` | the one loud thing |
| `src/components/conversation/TurnResult.vue` | tokens, cost, duration |
| `src/components/conversation/Composer.vue` | input, attachments, send, stop |
| `tests/stores/conversation.test.js` | the store, through `mockIPC` |
| `tests/components/conversation/markdown.test.js` | the parser |

---

## Task 1: The markdown tree

Pure and first, because it has no dependency on anything else and the components below consume it.

**Files:**
- Create: `src/components/conversation/markdown.js`, `tests/components/conversation/markdown.test.js`
- Modify: `package.json`

**Interfaces:**
- Produces: `parseMarkdown(text: string) -> Node[]` where a `Node` is one of
  `{ type: 'paragraph', children: Inline[] }`,
  `{ type: 'heading', level: number, children: Inline[] }`,
  `{ type: 'list', ordered: boolean, items: Node[][] }`,
  `{ type: 'code', lang: string|null, code: string }`,
  `{ type: 'quote', children: Node[] }`,
  and an `Inline` is one of
  `{ type: 'text', value }`, `{ type: 'strong'|'em', children }`, `{ type: 'code', value }`, `{ type: 'link', href, children }`.

- [ ] **Step 1: Pin the dependency**

```bash
node -p "require('./node_modules/@lezer/markdown/package.json').version"
```

Add `"@lezer/markdown": "^<that version>"` to `dependencies` in `package.json`, then `npm install`.

- [ ] **Step 2: Write the failing test**

```js
import { describe, expect, it } from 'vitest'
import { parseMarkdown } from '../../../src/components/conversation/markdown.js'

/* The text of every inline in a node, joined — enough to assert on structure
   without spelling out a whole inline tree in each case. */
const flatten = (children) =>
  children.map((node) => (node.type === 'text' || node.type === 'code' ? node.value : flatten(node.children))).join('')

describe('parseMarkdown', () => {
  it('turns a bare line into one paragraph', () => {
    expect(parseMarkdown('hello')).toEqual([
      { type: 'paragraph', children: [{ type: 'text', value: 'hello' }] }
    ])
  })

  it('keeps two blocks apart', () => {
    const nodes = parseMarkdown('one\n\ntwo')
    expect(nodes).toHaveLength(2)
    expect(flatten(nodes[1].children)).toBe('two')
  })

  it('reads a fenced block as code and keeps its language', () => {
    expect(parseMarkdown('```rust\nlet x = 1;\n```')).toEqual([
      { type: 'code', lang: 'rust', code: 'let x = 1;' }
    ])
  })

  it('reads a fence with no language as code all the same', () => {
    expect(parseMarkdown('```\nplain\n```')[0]).toEqual({ type: 'code', lang: null, code: 'plain' })
  })

  it('reads a bullet list, each item a list of blocks', () => {
    const [list] = parseMarkdown('- one\n- two')
    expect(list.type).toBe('list')
    expect(list.ordered).toBe(false)
    expect(list.items).toHaveLength(2)
    expect(flatten(list.items[1][0].children)).toBe('two')
  })

  it('tells an ordered list from a bullet one', () => {
    expect(parseMarkdown('1. one\n2. two')[0].ordered).toBe(true)
  })

  it('reads a heading with its level', () => {
    const [heading] = parseMarkdown('## Title')
    expect(heading).toMatchObject({ type: 'heading', level: 2 })
    expect(flatten(heading.children)).toBe('Title')
  })

  it('reads emphasis inside a paragraph rather than as literal asterisks', () => {
    const [paragraph] = parseMarkdown('a **bold** word')
    expect(paragraph.children.some((node) => node.type === 'strong')).toBe(true)
    expect(flatten(paragraph.children)).toBe('a bold word')
  })

  it('reads inline code as its own node so it can be drawn in mono', () => {
    const [paragraph] = parseMarkdown('run `cargo test` now')
    expect(paragraph.children).toContainEqual({ type: 'code', value: 'cargo test' })
  })

  it('reads a link and keeps its target', () => {
    const [paragraph] = parseMarkdown('see [the docs](https://example.com)')
    const link = paragraph.children.find((node) => node.type === 'link')
    expect(link.href).toBe('https://example.com')
    expect(flatten(link.children)).toBe('the docs')
  })

  /* The whole reason this module exists rather than a markdown-to-HTML
     library: the agent's prose routinely quotes output it did not write, and
     nothing here may produce markup. A tag survives only as text. */
  it('carries html through as text and never as markup', () => {
    const nodes = parseMarkdown('<img src=x onerror=alert(1)>')
    const asText = JSON.stringify(nodes)
    expect(asText).toContain('onerror')
    expect(nodes.every((node) => node.type !== 'html')).toBe(true)
  })

  it('answers with nothing at all for an empty string', () => {
    expect(parseMarkdown('')).toEqual([])
  })

  it('does not lose a trailing block that has no blank line after it', () => {
    expect(parseMarkdown('para\n\n- item')).toHaveLength(2)
  })
})
```

- [ ] **Step 3: Run it and watch it fail**

Run: `npm test -- tests/components/conversation/markdown.test.js`
Expected: FAIL — the module does not exist.

- [ ] **Step 4: Implement the parser**

```js
/* Markdown as a tree of plain objects, for components to draw.
 *
 * There is no HTML anywhere in this file and there must never be. The agent's
 * prose routinely quotes output it did not write, and this webview holds
 * Tauri's IPC: handing such a string to `v-html` would be a real hole rather
 * than a theoretical one. A tree closes it by construction — a tag that
 * survives parsing survives as text, and text is all a component can draw.
 *
 * The parser is `@lezer/markdown`, which the editor's markdown mode already
 * brings in. Walking its syntax tree rather than using a renderer is what keeps
 * the output ours: nodes this app knows about, and nothing else. */
import { parser } from '@lezer/markdown'

/* lezer names the node types; these are the ones this app draws. Anything not
 * listed falls through to its own text, which is the same choice the Rust side
 * makes for an event type it has never heard of. */
const INLINE = {
  StrongEmphasis: 'strong',
  Emphasis: 'em',
  InlineCode: 'code',
  Link: 'link'
}

export function parseMarkdown(text) {
  if (!text.trim()) return []
  const tree = parser.parse(text)
  const blocks = []
  const cursor = tree.cursor()
  // Descend once into the document, then take the top-level blocks in order.
  if (!cursor.firstChild()) return []
  do {
    const node = block(cursor.node, text)
    if (node) blocks.push(node)
  } while (cursor.nextSibling())
  return blocks
}

function block(node, text) {
  switch (node.name) {
    case 'Paragraph':
      return { type: 'paragraph', children: inlines(node, text) }
    case 'ATXHeading1':
    case 'ATXHeading2':
    case 'ATXHeading3':
    case 'ATXHeading4':
    case 'ATXHeading5':
    case 'ATXHeading6':
      return { type: 'heading', level: Number(node.name.slice(-1)), children: inlines(node, text) }
    case 'FencedCode':
    case 'CodeBlock':
      return code(node, text)
    case 'BulletList':
    case 'OrderedList':
      return {
        type: 'list',
        ordered: node.name === 'OrderedList',
        items: children(node, 'ListItem').map((item) =>
          children(item).map((child) => block(child, text)).filter(Boolean)
        )
      }
    case 'Blockquote':
      return { type: 'quote', children: children(node).map((child) => block(child, text)).filter(Boolean) }
    default:
      return null
  }
}

function code(node, text) {
  const info = node.getChild('CodeInfo')
  const body = node.getChild('CodeText')
  return {
    type: 'code',
    lang: info ? text.slice(info.from, info.to) : null,
    code: body ? text.slice(body.from, body.to) : ''
  }
}

/* The inlines of one block, in order, with the runs of plain text between them
 * kept — otherwise "a **bold** word" would lose both spaces. */
function inlines(node, text) {
  const out = []
  let at = node.from
  // A heading's own marker is a child too, and is not part of what it says.
  for (const child of children(node)) {
    if (child.name === 'HeaderMark' || child.name === 'QuoteMark' || child.name === 'ListMark') {
      at = Math.max(at, child.to)
      continue
    }
    const kind = INLINE[child.name]
    if (!kind) continue
    push(out, text.slice(at, child.from))
    out.push(inline(kind, child, text))
    at = child.to
  }
  push(out, text.slice(at, node.to))
  return out
}

function inline(kind, node, text) {
  if (kind === 'code') {
    const marks = children(node, 'CodeMark')
    const from = marks.length ? marks[0].to : node.from
    const to = marks.length > 1 ? marks[marks.length - 1].from : node.to
    return { type: 'code', value: text.slice(from, to) }
  }
  if (kind === 'link') {
    const url = node.getChild('URL')
    return {
      type: 'link',
      href: url ? text.slice(url.from, url.to) : '',
      children: [{ type: 'text', value: linkText(node, text) }]
    }
  }
  return { type: kind, children: inlines(node, text) }
}

function linkText(node, text) {
  const marks = children(node, 'LinkMark')
  return marks.length >= 2 ? text.slice(marks[0].to, marks[1].from) : text.slice(node.from, node.to)
}

/* A run of plain text, trimmed of nothing: the spaces around emphasis are part
 * of the sentence. Only a run that is entirely empty is dropped. */
function push(out, value) {
  if (!value) return
  const last = out[out.length - 1]
  if (last && last.type === 'text') last.value += value
  else out.push({ type: 'text', value })
}

function children(node, name) {
  const out = []
  let child = node.firstChild
  while (child) {
    if (!name || child.name === name) out.push(child)
    child = child.nextSibling
  }
  return out
}
```

- [ ] **Step 5: Run the tests**

Run: `npm test -- tests/components/conversation/markdown.test.js`
Expected: PASS, 13 tests. If lezer's node names differ from the list above, print them first — `parser.parse(text).toString()` names every node — and correct the switch rather than working around it.

- [ ] **Step 6: Commit**

```bash
git add src/components/conversation/markdown.js tests/components/conversation/markdown.test.js package.json package-lock.json
git commit -m "feat(conversation): markdown в дерево узлов, без единого фрагмента разметки"
```

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
- Create: `src/components/conversation/Markdown.vue`, `AgentMessage.vue`, `UserMessage.vue`, `ToolCall.vue`, `Reasoning.vue`, `TurnResult.vue`
- Modify: `src/components/index.js`, `src/views/Gallery.vue`

**Interfaces:**
- Consumes: `parseMarkdown` from Task 1; `Icon` from `../core/Icon.vue`; `iconFor` from `src/catppuccinIcon.js`.
- Produces: components taking one prop each — `Markdown { text }`, `AgentMessage { text }`, `UserMessage { text, attachments }`, `ToolCall { name, detail, result }` where `result` is `null | { ok, summary }`, `Reasoning { text }`, `TurnResult { tokensIn, tokensOut, costUsd, ms }`.

- [ ] **Step 1: Register the glyphs first**

`Icon` warns in dev for an unregistered name, and `core/icons.js` is the only file that names Lucide. Add whatever these components use — at minimum `chevron-right` (Reasoning's fold), `check` and `x` (a tool's outcome) — if they are not registered already. Check before adding; most are.

- [ ] **Step 2: Write `Markdown.vue`**

It renders the tree from Task 1 and recurses into itself for a quote and a list item. Fenced code goes to CodeMirror read-only through the existing editor wiring; inline code is a `<span>` in `var(--font-mono)` on `var(--surface-sunken)`.

```vue
<script setup>
import { computed } from 'vue'
import { parseMarkdown } from './markdown.js'

const props = defineProps({ text: { type: String, required: true } })
const blocks = computed(() => parseMarkdown(props.text))

const paragraph = computed(() => ({
  margin: 0,
  font: `var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)`,
  color: 'var(--text)'
}))
const inlineCode = {
  font: `var(--weight-regular) var(--text-xs)/1.4 var(--font-mono)`,
  background: 'var(--surface-sunken)',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-1)',
  padding: '0 var(--space-1)'
}
</script>
```

Draw every node type the parser can produce. A `link` is an `<a>` whose click is intercepted and handed to `openExternal` in `src/stores/app.js` — the desktop opens links in the person's own browser, and a navigation inside the webview would replace the app.

- [ ] **Step 3: Write the other five**

Each is the shape `src/components/status/StatusBadge.vue` already has and which
`Markdown.vue` above repeats: a `defineProps`, a `computed` returning an object of
`var(--token)` references, and a template binding it with `:style`. Read that file
before writing the first of these — it is the reference, and nothing below departs
from its shape.

`AgentMessage` is `Markdown` with the turn's spacing around it. `UserMessage` sits on `var(--surface-raised)` with a left border in `var(--border-strong)` so the two halves of the conversation are told apart by shape rather than by colour. `ToolCall` is one row: the glyph, the name in mono, the detail in mono and dimmed, and the result's outcome at the end — a tick or a cross with `summary` beside it, and nothing at all while it is still running. A path in `detail` takes its icon from `catppuccinIcon.js`. `Reasoning` is folded by default behind a `chevron-right` and carries `data-attention="quiet"` with `opacity: var(--attn-quiet-opacity)`. `TurnResult` is one dimmed mono line: `120 in · 40 out · $0.031 · 4.2 s`, with the cost omitted when it is `null` rather than drawn as `$0`.

- [ ] **Step 4: Export and gallery**

Add all six to `src/components/index.js` under a `// conversation` comment, and a section to `src/views/Gallery.vue` showing each with realistic content: a paragraph with a list and a fenced block, a tool call in each of its three states, a folded and an unfolded reasoning block, a turn result with and without a cost.

- [ ] **Step 5: Check by eye**

```bash
npm run dev
```

Open `http://localhost:5173/?view=gallery` and check the new section in all four combinations:

- `?view=gallery&theme=dark&density=comfortable`
- `?view=gallery&theme=dark&density=compact`
- `?view=gallery&theme=light&density=comfortable`
- `?view=gallery&theme=light&density=compact`

What to look for: nothing changes size between the themes (that would be a hardcoded value), the compact density tightens spacing without changing type colour or radius, and the file-type icon in a `ToolCall` is legible on both grounds.

- [ ] **Step 6: Commit**

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
