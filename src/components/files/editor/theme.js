/* The editor theme is the one file in src/ allowed to produce CSS rules:
   CodeMirror renders its own DOM, and rules are the only way to reach it. The
   design system's rule is kept in full even so — every value here is a
   var(--token), with no #hex and no px anywhere.

   The main consequence: one theme covers both app themes and both densities.
   The values are references, and the browser recomputes them on its own when
   data-theme and data-density change; the editor is never rebuilt and never
   flashes.

   The { dark: true } flag is deliberately not passed. It would raise the
   EditorView.darkTheme facet, which the base themes of the search panel and of
   special-character rendering watch — and they would start substituting their
   own hardcoded colours through the &light / &dark placeholders. So the theme
   below is exhaustive: everything the base themes would paint themselves is
   repainted with a token.

   Bracket matching is repainted for a different reason: its base in
   @codemirror/language is a flat, unconditional colour that never looks at the
   darkTheme facet at all — so it would have to be overridden in any case,
   regardless of this flag. */
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
  /* The field has no focus ring of its own: that belongs to the panel around
     it, and an outline around the text in a jumpy tab row is only noise. */
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

  /* The gutter. The padding on the right is deliberate: the git strip will
     stand as its own gutter to the left of the numbers and will not touch this
     block. */
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

  /* drawSelection paints the caret and the selection, not the browser: without
     it there is no multiple caret. The native ::selection takes no part in the
     field. */
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

  /* The search panel. @codemirror/search's base theme paints it itself,
     including a linear-gradient on the buttons — and gradients are forbidden in
     this system. */
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

/* Italics only where they carry markup meaning (markdown emphasis). Comments
   are not italicised: the system has no type-style token for it, and inventing
   a value is exactly what is forbidden. */
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
