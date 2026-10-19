/* The composer's draft as data: an ordered list of segments, each
   `{ kind: 'text' | 'quote', text }`, and every rule over that list.

   Pure, with no Vue and no DOM in it, because `Composer.vue` is the one thing
   no runner in this repository can reach, so the whole of the rule lives here
   and the component only draws it. Nothing in this file mutates its input:
   `Composer.vue` emits a fresh array on every edit, and the store's "clear the
   draft only if it is still the one that went" rule compares by reference.

   Three invariants, kept here and never by a component:

   - the list **ends with a text segment**, so there is always a line to write on;
   - **no two text segments are adjacent** — they are merged;
   - **no two quotes are adjacent** — an empty text always stands between them,
     so every quote has somewhere for its own reply.

   The first segment may be a quote. The ordinary case is an empty field
   receiving a quote, and a blank line above it would only invite somebody to
   write above a quotation they did not mean to answer. The last segment is
   always a text for the opposite reason: the reply goes under the quote.

   The empty draft is `[{ kind: 'text', text: '' }]`. */

const isSegment = (entry) =>
  !!entry &&
  (entry.kind === 'text' || entry.kind === 'quote') &&
  typeof entry.text === 'string'

const text = (value) => ({ kind: 'text', text: value })
const quote = (value) => ({ kind: 'quote', text: value })

/* Two texts that meet are joined by a newline when both have words, and by
   nothing when either is empty — so a merge never invents a blank line. */
const joinTexts = (above, below) => (above && below ? `${above}\n${below}` : above + below)

export function emptyDraft() {
  return [text('')]
}

/* A string (an older caller, a test, the phone) becomes one text segment; a list
   is repaired to the invariants, dropping entries that are not segments;
   anything else is the empty draft. */
export function normalizeDraft(value) {
  if (typeof value === 'string') return [text(value)]
  if (!Array.isArray(value)) return emptyDraft()
  const out = []
  for (const entry of value) {
    if (!isSegment(entry)) continue
    const last = out[out.length - 1]
    if (entry.kind === 'text') {
      if (last && last.kind === 'text') out[out.length - 1] = text(joinTexts(last.text, entry.text))
      else out.push(text(entry.text))
    } else {
      if (last && last.kind === 'quote') out.push(text(''))
      out.push(quote(entry.text))
    }
  }
  if (!out.length || out[out.length - 1].kind !== 'text') out.push(text(''))
  return out
}

/* Nothing to say: no quote, and every text blank. A quote with no words is not
   empty — it is sendable, as an attachment with no words is. */
export function isEmptyDraft(segments) {
  return normalizeDraft(segments).every((segment) => segment.kind === 'text' && !segment.text.trim())
}

/* A quote goes at the end. In front of the trailing text when that is blank, so
   the draft does not collect empty lines; otherwise after it, with a new empty
   text under the quote for the reply. */
export function appendQuote(segments, quoted) {
  const list = normalizeDraft(segments)
  const last = list[list.length - 1]
  /* Unless a quote already stands right before that blank text: the blank text
     is then the reply to it, and stays where it is. */
  if (!last.text.trim() && list[list.length - 2]?.kind !== 'quote') {
    return [...list.slice(0, -1), quote(quoted), last]
  }
  return [...list, quote(quoted), text('')]
}

/* Appended to the trailing text, with one space in front only when there is
   already something to join it to (the rule `attachToAgent` has always had). */
export function appendText(segments, added) {
  const list = normalizeDraft(segments)
  const last = list[list.length - 1]
  return [...list.slice(0, -1), text(last.text ? `${last.text} ${added}` : added)]
}

/* Removes the quote at `index` and merges the texts either side of it. `focus`
   says where the caret goes: the merged text, at the length of what stood above
   the quote — the join. An index that is not a quote changes nothing. */
export function removeQuote(segments, index) {
  const list = normalizeDraft(segments)
  if (!Number.isInteger(index) || list[index]?.kind !== 'quote') return { segments, focus: null }
  const above = list[index - 1]
  const rest = [...list.slice(0, index), ...list.slice(index + 1)]
  return {
    segments: normalizeDraft(rest),
    focus: above ? { index: index - 1, offset: above.text.length } : { index: 0, offset: 0 }
  }
}

/* What a selection becomes as a quote, or `null` for nothing worth quoting. */
export function quoteFromSelection(selected) {
  if (typeof selected !== 'string') return null
  const cleaned = selected.replace(/\r\n?/g, '\n').trim()
  return cleaned || null
}

/* The wire form. A quote is a markdown block quote: every line prefixed `> `,
   a blank line inside it a bare `>` so the block stays one block. Blank texts
   contribute nothing; blocks are separated by one blank line. Nothing else — no
   preface and no attribution. */
export function composeMessage(segments) {
  const blocks = []
  for (const segment of normalizeDraft(segments)) {
    if (segment.kind === 'text') {
      if (segment.text.trim()) blocks.push(segment.text)
    } else {
      blocks.push(
        segment.text
          .split('\n')
          .map((line) => (line.trim() ? `> ${line}` : '>'))
          .join('\n')
      )
    }
  }
  return blocks.join('\n\n').trim()
}

/* A string goes as it is; a list is composed. */
export function messageText(message) {
  return typeof message === 'string' ? message : composeMessage(message)
}
