/* Тема редактора — единственный файл в src/, которому разрешено порождать
   CSS-правила: CodeMirror рисует свой DOM сам, и повлиять на него можно только
   правилами. Правило дизайн-системы при этом сохраняется целиком — каждое
   значение здесь var(--token), ни одного #hex и ни одного px.

   Отсюда следует главное: тема одна на обе темы приложения и обе плотности.
   Значения — ссылки, и браузер пересчитывает их сам при смене data-theme и
   data-density; редактор не пересоздаётся и не мигает.

   Флаг { dark: true } намеренно не передаётся. Он поднял бы фасет
   EditorView.darkTheme, а на него смотрят базовые темы панели поиска и
   спецсимволов — и начали бы подставлять собственные захардкоженные цвета
   через плейсхолдеры &light / &dark. Поэтому тема ниже исчерпывающая: всё,
   что базовые темы красят сами, перекрашено токеном.

   Парные скобки перекрашены по другой причине: их база в @codemirror/language
   — плоский, безусловный цвет, который на фасет darkTheme вообще не смотрит,
   — так что её пришлось бы перекрывать в любом случае, вне зависимости от
   этого флага. */
import { EditorView } from '@codemirror/view'
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language'
import { tags as t } from '@lezer/highlight'

const chrome = EditorView.theme({
  '&': {
    height: '100%',
    backgroundColor: 'var(--editor-bg)',
    color: 'var(--syn-variable)',
    fontFamily: 'var(--font-mono)',
    fontSize: 'var(--text-code-size)',
    fontWeight: 'var(--weight-regular)'
  },
  /* Своей рамки фокуса у поля нет: она принадлежит панели вокруг, а обводка
     по периметру текста в дерганом списке вкладок только шумит. */
  '&.cm-focused': { outline: 'none' },
  '.cm-scroller': {
    fontFamily: 'inherit',
    lineHeight: 'var(--leading-code)'
  },
  '.cm-content': {
    padding: 'var(--space-4) 0',
    caretColor: 'var(--editor-cursor)'
  },
  '.cm-line': { padding: '0 var(--space-5)' },

  /* Гаттер. Отступ справа оставлен намеренно: git-полоса встанет отдельным
     гаттером слева от номеров и этот блок не тронет. */
  '.cm-gutters': {
    backgroundColor: 'var(--editor-gutter-bg)',
    color: 'var(--editor-line-number)',
    border: 'none',
    paddingRight: 'var(--space-2)'
  },
  '.cm-lineNumbers .cm-gutterElement': { padding: '0 var(--space-3)' },
  '.cm-activeLineGutter': {
    backgroundColor: 'var(--editor-active-line)',
    color: 'var(--editor-line-number-active)'
  },
  '.cm-activeLine': { backgroundColor: 'var(--editor-active-line)' },

  /* Каретку и выделение рисует drawSelection, а не браузер: без него нет
     множественной каретки. Нативный ::selection в поле не участвует. */
  '.cm-cursor, .cm-dropCursor': {
    borderLeftColor: 'var(--editor-cursor)',
    borderLeftWidth: 'var(--border-w-strong)'
  },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': {
    backgroundColor: 'var(--editor-selection)'
  },
  '.cm-selectionMatch': { backgroundColor: 'var(--editor-selection-match)' },
  '.cm-searchMatch': { backgroundColor: 'var(--editor-match-highlight)' },
  '.cm-searchMatch.cm-searchMatch-selected': {
    backgroundColor: 'var(--editor-match-highlight)',
    outline: 'var(--border-w) solid var(--editor-cursor)'
  },
  '&.cm-focused .cm-matchingBracket': {
    backgroundColor: 'var(--editor-selection-match)',
    color: 'inherit'
  },
  '&.cm-focused .cm-nonmatchingBracket': { color: 'var(--syn-invalid)' },
  '.cm-specialChar': { color: 'var(--syn-invalid)' },

  /* Панель поиска. Базовая тема @codemirror/search красит её сама, включая
     linear-gradient на кнопках — а градиенты в этой системе запрещены. */
  '.cm-panels': {
    backgroundColor: 'var(--surface)',
    color: 'var(--text-primary)',
    fontFamily: 'var(--font-sans)',
    fontSize: 'var(--text-ui-size)'
  },
  '.cm-panels.cm-panels-top': {
    borderBottom: 'var(--border-w) solid var(--border-subtle)'
  },
  '.cm-panels.cm-panels-bottom': {
    borderTop: 'var(--border-w) solid var(--border-subtle)'
  },
  '.cm-panel.cm-search': {
    padding: 'var(--space-3) var(--space-5)',
    display: 'flex',
    alignItems: 'center',
    gap: 'var(--space-3)',
    flexWrap: 'wrap'
  },
  '.cm-panel.cm-search label': {
    color: 'var(--text-secondary)',
    display: 'inline-flex',
    alignItems: 'center',
    gap: 'var(--space-2)'
  },
  '.cm-textfield': {
    backgroundColor: 'var(--surface-raised)',
    color: 'var(--text-primary)',
    border: 'var(--border-w) solid var(--border)',
    borderRadius: 'var(--radius-3)',
    padding: 'var(--space-2) var(--space-3)',
    fontFamily: 'var(--font-mono)',
    fontSize: 'var(--text-code-size)'
  },
  '.cm-button': {
    backgroundColor: 'var(--action-secondary-bg)',
    backgroundImage: 'none',
    color: 'var(--text-primary)',
    border: 'var(--border-w) solid var(--border)',
    borderRadius: 'var(--radius-3)',
    padding: 'var(--space-2) var(--space-4)',
    fontFamily: 'var(--font-sans)',
    fontSize: 'var(--text-ui-size)'
  },
  '.cm-button:hover': { backgroundColor: 'var(--action-secondary-bg-hover)' },
  '.cm-button:active': { backgroundColor: 'var(--action-secondary-bg-active)' },
  '.cm-panel.cm-search [name="close"]': {
    color: 'var(--text-secondary)',
    background: 'none',
    border: 'none',
    cursor: 'pointer'
  },
  '.cm-panel.cm-search [name="close"]:hover': { color: 'var(--text-primary)' }
})

/* Курсив только там, где он несёт смысл разметки (markdown emphasis).
   Комментарии курсивом не выделяются: в системе нет токена начертания под
   это, а выдумывать значение — ровно то, что запрещено. */
const syntax = HighlightStyle.define([
  { tag: [t.keyword, t.modifier, t.controlKeyword, t.operatorKeyword], color: 'var(--syn-keyword)' },
  { tag: [t.string, t.special(t.string), t.regexp], color: 'var(--syn-string)' },
  { tag: [t.number, t.bool, t.null, t.atom], color: 'var(--syn-number)' },
  { tag: [t.comment, t.lineComment, t.blockComment, t.docComment], color: 'var(--syn-comment)' },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.macroName], color: 'var(--syn-function)' },
  { tag: [t.typeName, t.className, t.namespace, t.standard(t.typeName)], color: 'var(--syn-type)' },
  { tag: [t.variableName, t.propertyName, t.attributeName], color: 'var(--syn-variable)' },
  { tag: [t.operator, t.derefOperator, t.compareOperator, t.logicOperator, t.arithmeticOperator], color: 'var(--syn-operator)' },
  { tag: [t.punctuation, t.separator, t.bracket, t.paren, t.brace, t.squareBracket, t.angleBracket], color: 'var(--syn-punctuation)' },
  { tag: [t.invalid], color: 'var(--syn-invalid)' },
  { tag: [t.heading], color: 'var(--syn-keyword)', fontWeight: 'var(--weight-semibold)' },
  { tag: [t.link, t.url], color: 'var(--syn-function)' },
  { tag: [t.strong], fontWeight: 'var(--weight-semibold)' },
  { tag: [t.emphasis], fontStyle: 'italic' }
])

export const editorTheme = [chrome, syntaxHighlighting(syntax)]
