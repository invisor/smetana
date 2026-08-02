/* Набор расширений собирается явно, а не через basic-setup: тот приносит
   автодополнение, линтер и свёртку — всё, от чего эта задача отказалась, —
   и молча растит бандл. */
import {
  EditorView,
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
import { indentationMarkers } from '@replit/codemirror-indentation-markers'
import { editorTheme } from './theme.js'

/* Escape на две секунды открывает выход по Tab — иначе клавиатурный
   пользователь заперт в поле, потому что Tab здесь занят отступом. Механизм
   штатный (EditorView.setTabFocusMode), но в стандартный keymap не входит.

   false, а не true: Escape должен доехать и до defaultKeymap, где он схлопывает
   множественное выделение. Обработчик, вернувший true, обрывает цепочку. */
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
    indentationMarkers({
      hideFirstIndent: true,
      colors: {
        light: 'var(--editor-indent-guide)',
        dark: 'var(--editor-indent-guide)',
        activeLight: 'var(--editor-indent-guide)',
        activeDark: 'var(--editor-indent-guide)'
      }
    }),
    /* Порядок значим: наш Escape идёт первым, чтобы взвести режим до того,
       как defaultKeymap схлопнет выделение; indentWithTab — до defaultKeymap,
       иначе Tab уйдёт в поведение по умолчанию. */
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
