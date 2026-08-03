/* The extension list is assembled explicitly rather than through basic-setup:
   that would bring autocomplete, a linter and code folding — everything this
   task declined — and silently grow the bundle. */
import {
  drawSelection,
  dropCursor,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  rectangularSelection
} from '@codemirror/view'
import { EditorState } from '@codemirror/state'
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { bracketMatching, indentOnInput, indentUnit } from '@codemirror/language'
import { highlightSelectionMatches, search, searchKeymap } from '@codemirror/search'
import { editorTheme } from './theme.js'

/* Escape opens the Tab exit for two seconds — otherwise a keyboard user is
   trapped in the field, because Tab is taken by indentation here.

   @codemirror/view does install its own always-on keydown handler on
   contentDOM for this (the same setTabFocusMode(2000)) — but it is attached
   last in the key's handler chain, after every keymap binding. Any handler that
   returns true for Escape, or a binding with the preventDefault flag, breaks
   the chain before the turn reaches it — and then the mode is never armed,
   while nothing outwardly reports the refusal. Neither defaultKeymap nor
   searchKeymap does that for Escape today, but that is a property of the set of
   bindings as a whole rather than of the mechanism itself, and it can quietly
   stop being true with the next Escape binding added here. Our own entry, first
   in the list, does not depend on who returns what after it: it is part of the
   same dispatch chain as any future Escape binding, and it runs before that one
   unconditionally. false, not true: Escape has to reach defaultKeymap too,
   where it collapses a multiple selection; a handler returning true would break
   the chain itself. */
const escapeOpensTabFocus = (view) => {
  view.setTabFocusMode(2000)
  return false
}

export function editorExtensions() {
  return [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightActiveLine(),
    highlightSpecialChars(),
    history(),
    drawSelection(),
    dropCursor(),
    rectangularSelection(),
    indentOnInput(),
    bracketMatching(),
    highlightSelectionMatches(),
    search({ top: true }),
    indentUnit.of('  '),
    EditorState.tabSize.of(2),
    EditorState.allowMultipleSelections.of(true),
    /* The order matters: our Escape comes first so the mode is armed before
       defaultKeymap collapses the selection; indentWithTab comes before
       defaultKeymap, otherwise Tab falls through to the default behaviour. */
    keymap.of([
      { key: 'Escape', run: escapeOpensTabFocus },
      indentWithTab,
      ...searchKeymap,
      ...historyKeymap,
      ...defaultKeymap
    ]),
    editorTheme
  ]
}
