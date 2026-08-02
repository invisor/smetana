/* Набор расширений собирается явно, а не через basic-setup: тот приносит
   автодополнение, линтер и свёртку — всё, от чего эта задача отказалась, —
   и молча растит бандл. */
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
    /* indentWithTab идёт первым в списке, чтобы Tab достался ему раньше, чем
       defaultKeymap применит к нему своё поведение по умолчанию. Escape,
       открывающий выход по Tab клавиатурному пользователю, отдельной записи
       не требует: тот же EditorView.setTabFocusMode на те же два секунды уже
       взводится всегда включённым keydown-хендлером самого @codemirror/view —
       собственная запись здесь была двойником, ничего не менявшим. */
    keymap.of([indentWithTab, ...searchKeymap, ...historyKeymap, ...defaultKeymap]),
    editorTheme
  ]
}
