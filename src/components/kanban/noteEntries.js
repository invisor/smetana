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

   An earlier version of this file matched a line against a vocabulary of
   markers this app is known to write — `parked:`, `resolved:` and the like —
   and read anything else as a continuation. A sweep of the real board
   refuted that outright: 177 of 1162 non-first note lines were ordinary
   prose with no marker at all, standing on their own as records the marker
   list simply had no name for (smetana-k2mo, review pass 1). The premise was
   wrong — this app's own vocabulary is not the board's — and no fixed list
   of markers can be, since a person's own note is free text by construction
   and the app does not get to enumerate what somebody will type next.

   The rule below is the other way around: **default to a new record, and
   fold a line onto the one before it only on positive evidence that it
   cannot stand alone.** That evidence is markdown's own block structure
   rather than this app's vocabulary — a line that is indented relative to
   the record it follows, or that opens with the syntax of a list item or a
   quote (`-`, `*`, `+`, `>`, `1.`, `1)` and the kind), reads as a
   continuation; a blank line carries no content of its own and folds too,
   which is what lets a record hold more than one markdown block internally
   (`markdown.js`'s own paragraph break) without becoming two records.
   Everything else — including an ordinary sentence with no marker, no
   indentation and no list syntax — starts a new record. This is what makes a
   header line immediately followed by a `1)`…`7)` enumeration read as one
   record (the enumeration is evidence, so it folds), while two unrelated
   plain sentences appended back to back read as two (neither is evidence, so
   neither folds).

   `INDENTED` is kept on measured evidence rather than a guess. A sweep of the
   whole board (review pass 2) found 104 lines, across all 280 issues that
   carry notes, that fold for no reason other than leading whitespace — 81 of
   them indented list items and 23 a hanging-indent prose wrap under a bullet
   (a bullet's own point continuing on the next line, indented under it, a
   shape the board actually has). Every one of the 104 is a genuine
   continuation; none is a standalone record wrongly swallowed. Dropping the
   condition to close a hypothetical hole would split 104 real continuations
   on this board today to defend against a shape that does not occur in it.

   That evidence does not make `INDENTED` free of risk, and the residual
   failure it carries is silent rather than visible — say so plainly rather
   than understating it. A genuinely separate `bd note` record whose own text
   happens to open with a space or a tab folds into the record above it with
   nothing on screen to say two records became one; the board says this
   shape does not occur, which is the whole of why the condition is kept, not
   a claim that it cannot occur. And `INDENTED` is not like the other three
   triggers in this respect: a line that folds only because it opens with
   list or quote syntax still renders as its own block regardless, because
   `markdown.js`'s `startsBlock` treats `BULLET`/`ORDERED`/`QUOTE` as
   unconditional block openers — a folded bullet or quote is misgrouped with
   its neighbour but still visibly its own list or quote on screen, never
   swallowed into a run of prose. Leading whitespace alone opens nothing in
   `markdown.js`'s grammar, so a record folded only on `INDENTED` has no such
   rescue: it is the one trigger in this file whose failure is a real record
   disappearing into another with no mark of it at all, which is why it is
   the one worth the scrutiny above rather than the same footing as the
   other three. */

/* A line indented relative to the record above it: a person's own
   continuation, never a record's own opening — nothing this app or its
   skills write starts a line with leading space or a tab. */
const INDENTED = /^[ \t]/

/* The opening syntax of a markdown list item or a block quote — mirrors the
   shape of `markdown.js`'s own `BULLET`, `ORDERED` and `QUOTE` (not imported:
   that file's regexes are private, and this only needs to recognise the same
   shape, not share the pattern object). `BULLET` and `ORDERED` both require
   `(\s+)` after the marker — a bare `-` or `1)` with nothing after it does
   not open a list there, so it must not fold here either, and the tail below
   is `\s` rather than `\s|$` for exactly that reason. Anchored at the start,
   so a hyphen or a number sitting mid-sentence — "Tuesday - not before" — is
   not read as one: the whole point of this list is what a line *opens*
   with, not what it contains. */
const LIST_OR_QUOTE_START = /^(?:[-*+>]|\d{1,9}[.)])\s/

/* Whether `line` can only be read as part of the record above it — the one
   question this module asks. Never called on the field's own first line,
   which is always a record regardless of its shape: an issue's first-ever
   note may be a plain sentence, and there is no record above it to fold
   into. */
function continuesPreviousRecord(line) {
  if (line.trim() === '') return true
  if (INDENTED.test(line)) return true
  return LIST_OR_QUOTE_START.test(line)
}

/* The field's own records, in the order bd appended them. Each entry may
   still hold more than one line — a continuation folded onto it, blank lines
   included — joined back with `\n`, which is what lets `markdown.js` read an
   internal blank line as this record's own paragraph break rather than as
   the boundary to another record, and what lets a header line and the list
   under it stay one record while still parsing as a paragraph followed by a
   list (`markdown.js`'s `startsBlock` opens a new block on `BULLET`/
   `ORDERED` whether or not a blank line came first — see its own comment on
   why: a `bd` description with criteria straight under the sentence must not
   be swallowed by it). */
export function splitNoteEntries(notes) {
  if (!notes || !String(notes).trim()) return []
  const lines = String(notes).split('\n')
  const entries = []
  lines.forEach((line, index) => {
    if (index > 0 && continuesPreviousRecord(line)) {
      entries[entries.length - 1] += `\n${line}`
    } else {
      entries.push(line)
    }
  })
  return entries
}

/* What `TaskInspector.vue` actually feeds `Markdown`: the same field, each
   record turned into its own paragraph. A blank line is markdown's own
   paragraph break (`markdown.js`'s `parseBlocks`), so joining the records
   with one is the whole of the normalization — no `br`, no `white-space`
   rule, and `MarkdownInline.vue` and `sm-prose.css` stay exactly as
   smetana-3tax left them. */
export function notesForDisplay(notes) {
  return splitNoteEntries(notes).join('\n\n')
}
