/* What a merge conflict is, in the text of a file and nothing else.

   The whole of the rule lives here rather than in the ViewPlugin that draws it,
   for the reason the rest of this family exists: a plugin is DOM, and no test in
   this repository can reach DOM. The plugin keeps no rule of its own.

   It is keyed on the text and deliberately not on git's status, because a file
   is drawn in three places — the editor and both panes of the diff — and only
   one of them has a repository behind it, while the markers are visible in all
   three. VS Code keys on the same thing.

   A block counts only when the full sequence appears in order:

       <<<<<<< HEAD                 start
       ...the current side
       ||||||| merged common        base, diff3 only, optional
       ...the common ancestor
       =======                      separator
       ...the incoming side
       >>>>>>> their-branch         end

   Anything short of that is ordinary text. Without the whole sequence a
   `=======` under a heading in Markdown — setext, and a real thing to write —
   would paint half the file as somebody else's merge. */

const START = '<<<<<<<'
const BASE = '|||||||'
const SEPARATOR = '======='
const END = '>>>>>>>'

/* Exactly seven of the character at the very start of the line, and then either
   the end of the line or a space. Eight is not a marker: git writes seven, and
   a row of `=` under a heading is as likely to be eight as seven, so the count
   is the only thing separating the two. A bare marker with nothing after it is
   a marker — git writes `=======` that way always, and `<<<<<<<` that way when
   the label is empty.

   "The end of the line" is the end of the string handed in, and the caller is
   what makes that the same thing: CodeMirror splits on /\r\n?|\n/, so a line
   never arrives carrying its terminator. A `=======\r` would fail this check —
   the string is eight long and the eighth character is not a space — and that is
   stated rather than handled, because the only way to reach it is to hand this
   module lines some other splitter produced. Split on the line break, not on
   "\n" alone, and the question does not arise. */
const isMarker = (line, marker) =>
  line.startsWith(marker) && (line.length === marker.length || line[marker.length] === ' ')

/* The blocks of `lines`, in order, as 1-based line numbers — which is what
   CodeMirror counts in, and what keeps the caller from doing the arithmetic
   twice. `base` is the `|||||||` line of a diff3 conflict, or null.

   The four numbers are the marker lines themselves. What lies between them is
   the caller's to colour: the current side is above `base ?? separator`, the
   incoming side is below `separator`, and the base section between the two has
   no side of its own. */
export function conflictBlocks(lines) {
  const blocks = []
  let start = null
  let base = null
  let separator = null

  const forget = () => {
    start = null
    base = null
    separator = null
  }

  lines.forEach((line, index) => {
    const number = index + 1

    /* A second `<<<<<<<` starts over rather than nesting: git never nests one,
       so the earlier opening was never a conflict and whatever it collected
       must not survive into this one. */
    if (isMarker(line, START)) {
      forget()
      start = number
      return
    }

    /* Outside a block every marker is ordinary text — this is the line that
       refuses the Markdown heading. */
    if (start === null) return

    if (isMarker(line, BASE)) {
      if (base === null && separator === null) base = number
      return
    }

    if (isMarker(line, SEPARATOR)) {
      if (separator === null) separator = number
      return
    }

    if (isMarker(line, END)) {
      /* A close with no separator above it closes nothing: the sequence was
         never a conflict, and the lines it spans stay plain. */
      if (separator !== null) blocks.push({ start, base, separator, end: number })
      forget()
    }
  })

  /* A block left open at the end of the file is not drawn. Half a conflict is
     text somebody wrote, and colouring it would run to the last line. */
  return blocks
}
