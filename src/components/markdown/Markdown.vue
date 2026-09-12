<script setup>
/* One of bd's prose fields, drawn as the markdown it is.

   The rules are all in `markdown.js`; this file only turns a block into the
   element `sm-prose.css` (`docs/design_handoff_conversation_panel/markup-contract.md`,
   section 2) already knows how to paint. No class and no `:style` on anything
   below — the styling keys on the element name and on the handful of `data-`
   attributes the contract names, and the whole tree is meant to sit inside an
   ancestor carrying `class="sm-prose"`, which this component does not draw
   itself: a quote and a list item are prose without being a root, and the root
   belongs to whatever wraps a whole message or a whole field.

   The template has no wrapping element of its own, on purpose: a `<div>` around
   the blocks would sit between, say, `<article data-turn>` and its `<p>`
   children, and `sm-prose.css`'s `li > :where(p,pre,blockquote,…)` and
   `:first-child`/`:last-child` rules select on that direct relationship. Vue 3
   templates may have more than one root node, which is what lets the blocks
   below land as direct children of whatever this is used inside.

   No `v-html`, deliberately and permanently: the tree is drawn as Vue nodes, so
   an issue's text can never become markup and no sanitiser is needed. A link is
   emitted rather than opened, the way `AboutSettings` does it — the library
   knows nothing about Tauri, and the view binds the app's own verbs to the
   two events `MarkdownInline.vue` raises: `open` for an external link, and
   `open-local` for one that names a path on this machine. `root`, threaded
   down to every `MarkdownInline` below, is that same split's other half — see
   that file's own header for what it is and what an empty one does.

   Read-only, all of it. A task item's box is drawn by `sm-prose.css` off
   `li[data-checked]`, never a Lucide glyph and never `<input type="checkbox"
   disabled>` — see the contract for why the native control is refused.

   The one control on the tree — the code block's copy button (contract
   section 4) — stays read-only in the same sense: it acts on the clipboard,
   never on the prose, and answers with nothing this component keeps for
   itself, past the confirmation on the one button somebody pressed.

   Its state and its timer are `core/copyFeedback.js`'s `useCopyFeedback`, not
   a copy of that policy written out again — that file's own header narrates
   what a fifth copy of "clear the timer, claim the target, await the write,
   bail if a later press has taken it over, clear the timer a second time" has
   already cost this tree twice, and there is no reason to find out a fifth
   time what a fourth already proved. The one place this call site differs
   from every other is the duration: the contract's own 1600ms rather than
   `COPIED_MS`'s 1200, passed as that composable's second argument — see
   `kanban/copyId.js`'s header for why the two numbers are allowed to differ.

   The clipboard writer is `inject('smCopyText', …)`, the same shape
   `overlays/Modal.vue` reaches `views/DialogWindow.vue` through
   (`smDialogWindow`, `smDialogFill`, `smDialogClosable`): a library component
   reaching an ancestor that owns a store without importing one itself. This
   file draws markup for whichever view ends up rendering it — today
   `views/DesktopApp.vue` and `views/Gallery.vue`, the only two that draw
   anything with a `Markdown` under it — and must never import
   `@tauri-apps/api` on its own account, so the default below, wired to
   nothing, is the browser's own `navigator.clipboard`. Both of those views
   already import `stores/app.js`'s `copyText`, which prefers the Tauri
   clipboard plugin for the reason that store's own header gives —
   `navigator.clipboard` wants a secure context and a gesture the webview does
   not always agree it had, a failure with no visible cause in the packaged
   app alone — so both `provide('smCopyText', copyText)`, and the default
   below is what a future owner falls back to until it does the same.

   An `image` block is `MarkdownFigure.vue`'s, section 7's `figure[data-figure]`
   whole — the mat, the preferred inline `<svg>` form, the loading and the
   failure placeholders. `groups` below is the one thing this file still owns
   about it: folding a run of consecutive `image` blocks into the wrapping
   `div[data-figures]` row section 7 asks for, since that grouping is about the
   turn's layout and not about any one figure. Which sources an illustration
   may even draw, and what a source or an inline `<svg>` that fails the render
   check draws *instead* of the figure it asked for, is `figureSource.js`'s —
   see `MarkdownFigure.vue`'s own header for why that is not simply `link()`'s
   `OPENABLE` reused.

   `base` is the second path a figure needs beside `root`: `root` is the
   active project's own path, used to build a *local link's* `href`; `base` is
   where a *relative image source* resolves from, and the two answer different
   questions even though they often carry the same value. A link is written
   against the project the whole task lives in, and stays the project's root
   whoever is talking; a picture is written against the directory an agent was
   sitting in when it typed `![x](./fig.png)` — the session's own cwd, a
   worktree more often than not — and the task inspector has no session at
   all, only the project. `base` therefore defaults to `root` rather than to
   `''`: every caller of this file already has a `root` to give it, and the
   one that does not want a working local link (the inspector) still has an
   ordinary project directory a relative image path may sensibly be read
   against. A caller with an actual session cwd passes it as `base` explicitly
   and keeps `root` as the project's own path for its links —
   `ConversationView.vue` does exactly that, off `stores/conversation.js`'s
   `Attached::cwd`, which is a worktree rather than the project root for a
   resumed session. The task inspector is still the one caller with no
   session at all, and reads its figures against the project root instead,
   the fallback above exists for.

   **The live edge (smetana-6we6).** `streaming` says a reply is still being
   written, and while it is, the last child of the *last* block gets one more
   element after it: `span[data-edge]`, section 6's own "Streaming edge",
   which `sm-prose.css` paints as the caret pinned to the last character
   produced. It travels with `streaming` down every recursive call this file
   makes of itself — a blockquote or a list carries the flag to its own last
   nested block, `isLast` composed with the group index at each level, since
   the tree the caret belongs at the end of is the whole document's, not any
   one level of it. Handled here: a paragraph, a heading, and a fenced code
   block still being typed — the shapes a reply is actually still arriving as
   in the ordinary case. **Not handled, and recorded rather than papered
   over**: a table, a definition list, a rule or a run of images as the
   literal last block while a reply is mid-sentence inside one — none of
   those is where an agent's prose is normally still open, and the contract's
   own example is a paragraph; such a reply is complete prose with no visible
   caret for as long as that block is the last one, which corrects itself the
   moment the next block opens. Nothing else about the tree changes for
   `streaming` — the mark this file draws over is the one JS row `journal.js`
   is already forwarding as `AgentMessage`'s own `streaming` prop, so a
   finished reply and a partial one differ by exactly this one element, as
   the acceptance criteria ask.

   The bytes themselves are never this file's to fetch, and `readInlineSvg`
   aside, this component does not even decide whether a picture loaded: that
   is `MarkdownFigure.vue`'s own state machine, fed by a function rather than
   a store. `inject('smReadImage', …)`, the same shape as `smCopyText` right
   above and for the same reason — a library component may not import
   `stores/attachments.js`, which knows Tauri exists, so the two views that
   provide `smCopyText` provide this alongside it, and the default here is a
   plain rejection: a browser has no way to open an arbitrary path on the
   machine's disk at all, unlike a clipboard, which has an ordinary web
   API to fall back to. */
import { computed, inject } from 'vue'
import Icon from '../core/Icon.vue'
import MarkdownInline from './MarkdownInline.vue'
import MarkdownFigure from './MarkdownFigure.vue'
import { parseMarkdown } from './markdown.js'
import { useCopyFeedback } from '../core/copyFeedback.js'

const props = defineProps({
  /* The source. Ignored when `blocks` is given, which is what the recursive
     calls below pass — a quote and a list item are already parsed, and parsing
     them again would be the same work done twice per level of nesting. */
  text: { type: String, default: '' },
  blocks: { type: Array, default: null },
  /* The active project's absolute path, or `''` where there is none — see
     `MarkdownInline.vue`'s own header. Passed straight through to every node
     this file draws and to the recursive calls below, since a quote or a list
     item is the same prose at one remove and owes its own links the same
     answer to "is there anything here that can open one". */
  root: { type: String, default: '' },
  /* Where a relative illustration source resolves from — see this file's own
     header for why it is not simply `root`. `''` is not a real default: the
     computed below falls back to `root` the moment this prop is left unset,
     so `''` only takes effect for a caller that passes it explicitly, the way
     an empty `root` means "no project" for a link. */
  base: { type: String, default: '' },
  /* Whether this whole message is still arriving — see this file's own
     header, "The live edge". `false` for a recursive call the caret's group
     is not the last of, so only one `span[data-edge]` ever exists at once. */
  streaming: { type: Boolean, default: false }
})

const emit = defineEmits(['open', 'open-local', 'open-image'])

const tree = computed(() => props.blocks ?? parseMarkdown(props.text))

/* See this file's own header. */
const effectiveBase = computed(() => props.base || props.root)

/* `markdown.js` hands over a flat run of blocks, with two adjacent `image`
   blocks simply sitting next to each other — nothing in the parser groups
   them, because grouping is a fact about how the *renderer* lays a turn out,
   not about the source. Section 7 of the contract draws two figures in one
   turn inside `div[data-figures]`, a wrapping flex row, and a lone figure
   bare — so this is the one place the flat list becomes something the
   template can dispatch on directly, folding every run of consecutive
   `image` blocks into one `figures` entry and leaving everything else as an
   ordinary `single` one. */
const groups = computed(() => {
  const entries = []
  for (const block of tree.value) {
    if (block.type === 'image') {
      const last = entries[entries.length - 1]
      if (last && last.type === 'figures') last.blocks.push(block)
      else entries.push({ type: 'figures', blocks: [block] })
    } else {
      entries.push({ type: 'single', block })
    }
  }
  return entries
})

/* Is this the last group of the whole message — where the live edge belongs
   while `streaming`. A plain index compare rather than a computed of its own:
   it is asked once per group at render time, off `groups.value.length`, which
   is already reactive. */
function isLast(index) {
  return index === groups.value.length - 1
}

/* Whether *this* group, at this level of recursion, is where the caret
   belongs — this file's own `streaming` prop, narrowed to the one group the
   whole document's last block is inside. Every recursive call below passes
   exactly this back in as its own `streaming` prop, so the flag is `true` at
   one place across the whole nested tree or nowhere in it. */
function edgeHere(index) {
  return props.streaming && isLast(index)
}

/* `data-task` lives on the `<ul>`, not per item, so it is decided once for the
   whole list, off a single item — `markdown.js`'s `takeList` is what actually
   decides this, and its guarantee is what makes checking one enough: a list's
   items carry a checked state either all together or not at all, never a mix,
   so `sm-prose.css`'s unconditional `ul[data-task] > li::before` never lands
   on a plain bullet. This file does not re-derive that homogeneity, only
   trusts it — an ordered list is never a task list either, for the same
   reason `takeList` refuses one: the contract's box is `ul[data-task]` only. */
function isTaskList(block) {
  return block.items.length > 0 && block.items[0].checked !== null
}

/* Wrapped rather than passed bare so it always answers `Promise<boolean>`,
   `copyText`'s own shape and what `useCopyFeedback` requires of `write` —
   `navigator.clipboard.writeText` alone resolves `undefined` on success and
   rejects on failure, neither of which is a boolean. */
async function browserCopyText(text) {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch (err) {
    // Left at rest: a control that claimed success it did not have would be
    // worse than one that stays silent about a clipboard it could not reach.
    console.error('[markdown] the code block did not reach the clipboard:', err)
    return false
  }
}

const writeCode = inject('smCopyText', browserCopyText)

/* See this file's own header for why a browser's own answer is a plain
   refusal rather than a partial implementation of one. */
async function browserReadImage(base, src) {
  throw new Error('reading a picture from a path needs the desktop app')
}

const readImage = inject('smReadImage', browserReadImage)

/* The contract's own confirmation window — see `kanban/copyId.js`'s header
   for why this is not `COPIED_MS`. */
const COPY_CODE_MS = 1600

/* One `useCopyFeedback` per `Markdown` instance, keyed on the `v-for` index
   below: a quote or a list item recurses into its own instance with its own
   copy of this state, so two code blocks in two different quotes are free to
   say "Copied" at the same moment — nothing in the contract asks for one
   confirmation across a whole document, only one per control, which is this
   composable's own "one target at a time" read one level narrower. The
   parser only ever appends blocks, so the index is a stable enough key for
   which one last said "Copied"; see `useCopyFeedback` for the rest of the
   policy this file no longer writes out by hand. */
const { stateFor: copyStateFor, copy: copyCode } = useCopyFeedback(writeCode, COPY_CODE_MS)

function isCodeCopied(index) {
  return copyStateFor(index) === 'copied'
}
</script>

<template>
  <template v-for="(group, index) in groups" :key="index">
    <!-- A run of one or more illustrations. A lone one stays bare, the way
         every other block does; two or more share the wrapping row section 7
         asks for, `div[data-figures]`, which is a fact about the *run* and
         not about any one figure in it. -->
    <template v-if="group.type === 'figures'">
      <div v-if="group.blocks.length > 1" data-figures>
        <MarkdownFigure
          v-for="(figure, at) in group.blocks"
          :key="at"
          :block="figure"
          :base="effectiveBase"
          :read-image="readImage"
          @open-image="emit('open-image', $event)"
        />
      </div>
      <MarkdownFigure
        v-else
        :block="group.blocks[0]"
        :base="effectiveBase"
        :read-image="readImage"
        @open-image="emit('open-image', $event)"
      />
    </template>

    <template v-else>
      <component :is="`h${group.block.level}`" v-if="group.block.type === 'heading'">
        <MarkdownInline
          :nodes="group.block.children"
          :root="root"
          @open="emit('open', $event)"
          @open-local="emit('open-local', $event)"
        />
        <span v-if="edgeHere(index)" data-edge></span>
      </component>

      <p v-else-if="group.block.type === 'paragraph'">
        <MarkdownInline
          :nodes="group.block.children"
          :root="root"
          @open="emit('open', $event)"
          @open-local="emit('open-local', $event)"
        />
        <span v-if="edgeHere(index)" data-edge></span>
      </p>

      <figure v-else-if="group.block.type === 'code'" data-code :data-lang="group.block.lang || undefined">
        <figcaption v-if="group.block.lang">{{ group.block.lang }}</figcaption>
        <button
          type="button"
          data-copy
          :data-state="isCodeCopied(index) ? 'copied' : 'idle'"
          :aria-label="isCodeCopied(index) ? 'Copied' : 'Copy code'"
          @click="copyCode(index, group.block.text)"
        >
          <Icon name="copy" data-icon="copy" />
          <Icon name="check" data-icon="check" />
          <span>{{ isCodeCopied(index) ? 'Copied' : 'Copy' }}</span>
        </button>
        <pre><code>{{ group.block.text }}<span v-if="edgeHere(index)" data-edge></span></code></pre>
      </figure>

      <hr v-else-if="group.block.type === 'rule'" />

      <blockquote v-else-if="group.block.type === 'quote'">
        <Markdown
          :blocks="group.block.blocks"
          :root="root"
          :base="effectiveBase"
          :streaming="edgeHere(index)"
          @open="emit('open', $event)"
          @open-local="emit('open-local', $event)"
          @open-image="emit('open-image', $event)"
        />
      </blockquote>

      <component
        :is="group.block.ordered ? 'ol' : 'ul'"
        v-else-if="group.block.type === 'list'"
        :start="group.block.ordered && group.block.start !== 1 ? group.block.start : undefined"
        :data-task="isTaskList(group.block) ? '' : undefined"
      >
        <li
          v-for="(entry, at) in group.block.items"
          :key="at"
          :data-checked="entry.checked ? '' : undefined"
        >
          <Markdown
            :blocks="entry.blocks"
            :root="root"
            :base="effectiveBase"
            :streaming="edgeHere(index) && at === group.block.items.length - 1"
            @open="emit('open', $event)"
            @open-local="emit('open-local', $event)"
            @open-image="emit('open-image', $event)"
          />
        </li>
      </component>

      <dl v-else-if="group.block.type === 'dl'">
        <template v-for="(item, at) in group.block.items" :key="at">
          <dt>
            <MarkdownInline
              :nodes="item.term"
              :root="root"
              @open="emit('open', $event)"
              @open-local="emit('open-local', $event)"
            />
          </dt>
          <dd v-for="(definition, d) in item.definitions" :key="d">
            <MarkdownInline
              :nodes="definition"
              :root="root"
              @open="emit('open', $event)"
              @open-local="emit('open-local', $event)"
            />
          </dd>
        </template>
      </dl>

      <!-- The wrapper owns the border, the radius and the horizontal scroll
           (`markup-contract.md`, section 3), so the table itself never has to
           clip anything and the panel never has to grow. `data-wide` is set off
           the column count alone — more than four — which is the one thing this
           file knows ahead of layout; `sm-prose.css` is what turns that into
           `width:max-content` against `--prose-table-wide-min`. `block.align`
           is `null` for a column with no opinion, and `?? undefined` is what
           keeps that a missing attribute rather than `data-align="null"`. -->
      <div data-table-scroll v-else-if="group.block.type === 'table'">
        <table :data-wide="group.block.head.length > 4 ? '' : undefined">
          <thead>
            <tr>
              <th
                v-for="(cell, column) in group.block.head"
                :key="column"
                :data-align="group.block.align[column] ?? undefined"
              >
                <MarkdownInline
                  :nodes="cell"
                  :root="root"
                  @open="emit('open', $event)"
                  @open-local="emit('open-local', $event)"
                />
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, r) in group.block.rows" :key="r">
              <td
                v-for="(cell, column) in row"
                :key="column"
                :data-align="group.block.align[column] ?? undefined"
              >
                <MarkdownInline
                  :nodes="cell"
                  :root="root"
                  @open="emit('open', $event)"
                  @open-local="emit('open-local', $event)"
                />
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </template>
  </template>
</template>
