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

/* Escape на две секунды открывает выход по Tab — иначе клавиатурный
   пользователь заперт в поле, потому что Tab здесь занят отступом.

   @codemirror/view действительно ставит для этого свой always-on keydown-
   хендлер на contentDOM (тот же setTabFocusMode(2000)) — но он подключён
   последним в цепочке обработчиков клавиши, после всех keymap-биндингов.
   Любой обработчик, который для Escape вернёт true, или биндинг с флагом
   preventDefault, обрывает цепочку раньше, чем очередь дойдёт до него, — и
   тогда режим не взводится, хотя внешне ничего не сообщает об отказе. Сейчас
   ни defaultKeymap, ни searchKeymap на Escape так не поступают, но это
   свойство набора биндингов целиком, а не самого механизма, и оно способно
   тихо перестать быть верным при следующем добавленном сюда Escape-биндинге.
   Собственная запись первой в списке не зависит от того, кто и что вернёт
   после неё: она — часть той же цепочки dispatch'а, что и любой будущий
   Escape-биндинг, и выполняется раньше него безусловно. false, а не true:
   Escape должен доехать и до defaultKeymap, где он схлопывает множественное
   выделение; обработчик, вернувший true, оборвал бы цепочку сам. */
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
