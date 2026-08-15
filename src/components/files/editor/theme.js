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

/* The diff, drawn by @codemirror/merge in this same DOM and therefore themed
   here — the file is the exception, and the exception is about CodeMirror
   rather than about the editor.

   The package brings a base theme of its own with two opinions this system does
   not share, and both are suppressed rather than left to lose a specificity
   argument. It paints the changed characters with a `linear-gradient`
   underline, on both sides and in both of its own light and dark variants, and
   gradients are forbidden here — exactly the case @codemirror/search's buttons
   already made above. And its colours are hardcoded hexes chosen against
   `&light`/`&dark`, which this theme never raises (see the note at the top), so
   every one of them would resolve to its light value whatever the app's theme
   is. What replaces the gradient is an underline in a token colour, which is
   what the gradient was drawing anyway: the exact characters that moved, marked
   without touching the syntax colour under them.

   `&` is the editor element and it is what carries `cm-merge-a` (HEAD) or
   `cm-merge-b` (the working tree) — the sides are told apart by that class and
   nothing else, so a rule without it would paint an addition in the colour of a
   deletion on the other side of the same screen.

   Two families of the base theme's rules are deliberately not answered here:
   `.cm-deletedChunk` with its accept and reject buttons, and `.cm-collapsedLines`
   with a gradient of its own. Both belong to `unifiedMergeView` and
   `collapseUnchanged`, neither of which this app builds — a rule for a class
   that never appears is a rule nobody could ever check. */
const diff = EditorView.theme({
  '&.cm-merge-a .cm-changedLine': { backgroundColor: 'var(--diff-removed-bg)' },
  '&.cm-merge-b .cm-changedLine': { backgroundColor: 'var(--diff-added-bg)' },
  '&.cm-merge-a .cm-changedText': {
    background: 'none',
    textDecoration: 'underline',
    textDecorationColor: 'var(--diff-removed-fg)'
  },
  '&.cm-merge-b .cm-changedText': {
    background: 'none',
    textDecoration: 'underline',
    textDecorationColor: 'var(--diff-added-fg)'
  },
  /* A line that is wholly new, or wholly gone, is one long `cm-changedText` —
     so the rule above would underline it end to end, which is what a file the
     other side does not have at all looks like: every line of it ruled through.
     The ground already says the line is new; the underline is there to mark
     what moved *inside* a line that otherwise stayed. `cm-insertedLine` and
     `cm-deletedLine` are the package's own names for exactly those two cases. */
  '.cm-insertedLine .cm-changedText, .cm-deletedLine .cm-changedText': {
    textDecoration: 'none'
  },
  /* The 3px strip down the inside edge of the gutter: the one mark that says a
     line changed while the pane is scrolled past it. */
  '&.cm-merge-a .cm-changedLineGutter': { backgroundColor: 'var(--diff-removed-gutter)' },
  '&.cm-merge-b .cm-changedLineGutter': { backgroundColor: 'var(--diff-added-gutter)' },
  /* The filler rows @codemirror/merge inserts to keep the two sides level. They
     are not lines of either document and must not read as one, so they take the
     gutter's own ground rather than the editor's. */
  '.cm-mergeSpacer': { backgroundColor: 'var(--editor-gutter-bg)' }
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

/* The diff rules ride with the rest rather than as an extension of their own:
   the classes they name appear in a merge view and nowhere else, so an ordinary
   editor carries a handful of rules that match nothing, against the alternative
   of two theme exports that could be assembled in the wrong order. */
export const editorTheme = [chrome, diff, syntaxHighlighting(syntax)]
