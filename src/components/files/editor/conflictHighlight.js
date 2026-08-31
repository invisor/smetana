/* The thin half of conflict highlighting: line decorations over whatever
   `conflictBlocks.js` says the blocks are. There is no rule in this file — it
   holds no notion of what a marker looks like — because a ViewPlugin is DOM and
   nothing here can be tested. What the three classes come to is `theme.js`'s,
   the one file in `src/` allowed to write CSS.

   One extension, registered once in `extensions.js`, and it therefore reaches
   all three places a file is drawn: the editor and both panes of the diff. */
import { RangeSetBuilder } from '@codemirror/state'
import { Decoration, ViewPlugin } from '@codemirror/view'
import { conflictBlocks } from './conflictBlocks.js'

/* The marker lines take no ground of their own, only the conflict's colour: the
   block is already bounded by the two sides' stripes, and a third ground on the
   four lines that bound it would be decoration. The base section of a diff3
   conflict is left plain for the same reason — its marker is coloured and its
   lines are not. */
const CURRENT = Decoration.line({ class: 'cm-sm-conflict-current' })
const INCOMING = Decoration.line({ class: 'cm-sm-conflict-incoming' })
const MARKER = Decoration.line({ class: 'cm-sm-conflict-marker' })

/* Line decorations for one document state. `iterLines` rather than
   `doc.toString().split()`: the document is read on every change, and a file at
   this app's 2 MB ceiling would otherwise be copied whole per keystroke. */
const build = (state) => {
  const lines = []
  for (const line of state.doc.iterLines()) lines.push(line)

  const builder = new RangeSetBuilder()
  for (const block of conflictBlocks(lines)) {
    /* Where the current side stops: at the base marker in a diff3 conflict, at
       the separator in an ordinary one. */
    const currentEnd = block.base === null ? block.separator : block.base
    for (let number = block.start; number <= block.end; number += 1) {
      const decoration =
        number === block.start || number === block.base ||
        number === block.separator || number === block.end
          ? MARKER
          : number < currentEnd
            ? CURRENT
            : number > block.separator
              ? INCOMING
              : null
      /* The base section falls through with nothing. */
      if (decoration === null) continue
      const { from } = state.doc.line(number)
      builder.add(from, from, decoration)
    }
  }
  return builder.finish()
}

export function conflictHighlight() {
  return ViewPlugin.fromClass(
    class {
      constructor(view) {
        this.decorations = build(view.state)
      }

      /* Only the text can change what a conflict is, so a selection or a
         viewport change rebuilds nothing. The whole document is scanned rather
         than the viewport, because a block opened above the visible range
         decides the colour of everything inside it. */
      update(update) {
        if (update.docChanged) this.decorations = build(update.state)
      }
    },
    { decorations: (plugin) => plugin.decorations }
  )
}
