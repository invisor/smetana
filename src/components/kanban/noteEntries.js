/* What separates one `bd note` record from the next inside the joined string
   bd hands back in the `notes` field, and the whole reason this file exists
   rather than a line in `TaskInspector.vue` (smetana-k2mo).

   `notes` is a journal: every `bd note`/`--append-notes` call appends another
   record, and bd hands the whole thing over as one string, the records joined
   by a single newline and nothing else — no per-entry delimiter, no
   timestamp, no id (`tracker::bd`'s own recorded fixture:
   `"notes":"parked: needs a decision\nparked: still waiting"`). Fed to
   `Markdown` as one blob, a single newline is an ordinary soft break —
   collapsed to a space, correctly, for the prose that field usually is — so
   every record after the first read as one glued sentence. Before smetana-3tax
   removed it, a stray `white-space: pre-wrap` in `MarkdownInline.vue` held the
   line breaks apart as a side effect; that was never a markdown rule and
   `sm-prose.css` still has no `white-space` rule for `p` and no `br` in its
   contract (`markup-contract.md`, sections 2 and 9) — reopening either is out
   of scope here.

   The join is genuinely lossy: bd does not say whether a `\n` in the string
   is the boundary between two calls or a line break a person typed inside a
   single one of them, and the two produce byte-identical strings — there is
   no way to tell them apart from the string alone in general. What is not
   lossy is the vocabulary: every note this app, its Rust side and its skills
   actually write opens with one of a small set of markers — `parked:` /
   `resolved:` (`components/kanban/parked.js`, `agents/prompt.rs`), `merged:`,
   `digest:`, `closed with follow-up`, `live check`, `epic child`
   (`resources/smetana/skills/running-tasks/SKILL.md`), or `batch <n> (`
   (`runs::queue::release`) — and every one of those calls is disciplined to a
   single line (`runs::queue`'s own tests assert `!note.contains('\n')` on
   every one it writes). A line that opens with one of them is read as the
   start of a new record; so is the very first line of the field, marker or
   not, since an issue's first-ever note may carry none. Anything else is read
   as a continuation of whatever record precedes it — which is where a
   person's own line break inside one note lands — and it is folded back onto
   that record with a single `\n` of its own, so the whole record still
   renders as one markdown paragraph rather than several.

   What this does not solve, and cannot from the string alone: two ordinary
   notes with no marker at all, written back to back with nothing between
   them, are indistinguishable from one note a person wrote across two lines.
   That pair reads as a single record rather than two — not a regression,
   since without this module every record on the field ran together anyway,
   but a real limit on what a marker-based rule can do. */

const MARKERS = [
  /^parked:/i,
  /^resolved:/i,
  /^merged:/i,
  /^digest:/i,
  /^closed with follow-up/i,
  /^live check/i,
  /^epic child/i,
  /^batch \d+ \(/i
]

const opensRecord = (line) => MARKERS.some((marker) => marker.test(line.trim()))

/* The field's own records, in the order bd appended them. Each entry may
   still hold more than one line — a person's own break inside a single call —
   joined with `\n`, the same soft break markdown already reads correctly. */
export function splitNoteEntries(notes) {
  if (!notes) return []
  const lines = String(notes).split('\n')
  const entries = []
  lines.forEach((line, index) => {
    if (index === 0 || opensRecord(line)) {
      entries.push(line)
    } else {
      entries[entries.length - 1] += `\n${line}`
    }
  })
  return entries
}

/* What `TaskInspector.vue` actually feeds `Markdown`: the same field, each
   record turned into its own paragraph. A blank line is markdown's own
   paragraph break (`markdown.js`'s `parseBlocks`), so joining the records with
   one is the whole of the normalization — no `br`, no `white-space` rule, and
   `MarkdownInline.vue` and `sm-prose.css` stay exactly as smetana-3tax left
   them. */
export function notesForDisplay(notes) {
  return splitNoteEntries(notes).join('\n\n')
}
