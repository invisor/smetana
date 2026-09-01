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
   below has to answer for everything the base themes would paint themselves,
   and repaint it with a token. It does, for every class the app's extension set
   can put on the screen; what that claim does not cover is named at the end of
   this note, with where each of them lives.

   Answering means naming the class at a depth that wins, and for nearly every
   rule here that depth is the plain one. buildTheme's finish() *replaces*
   &light and &dark with the placeholder's class rather than adding it to the
   theme's own (@codemirror/view/dist/index.js, the sel.replace in finish), so a
   base `&light X` rule and a plain `X` of ours compile to exactly the same
   depth — measured from the mounted sheet for .cm-content, .cm-gutters,
   .cm-activeLine, .cm-specialChar, .cm-panels and .cm-textfield, every one of
   them an exact tie. A tie is settled by the order the style modules reach the
   document, and ours is mounted after every one of the base themes — they are
   all at Prec.lowest — so ours wins: the same derivation the `conflict` block
   below sets out at length, and the same conclusion. Ours is not mounted *last* —
   a stronger claim than the argument needs, and not a true one: drawSelection
   raises rules at Prec.highest that come after it, and they touch ::selection
   and caret-color only, neither of which turns on mount order.

   So a rule here needs the base's own shape wherever the base's own selector is
   strictly deeper than a plain one, and the family that matters for this note is
   the selection layer while the field has the focus, five classes against a
   plain rule's two — see the selection block below for the shape it has to take.

   It is not the only place a plain rule of ours is out-specified, and none of
   this is a list to stop looking at. The **pressed** search-panel button is a
   second: @codemirror/view nests an `&:active` inside `&light .cm-button`, a
   class deeper than a plain `.cm-button`, so the `backgroundImage: 'none'`
   there covered the resting button and not the pressed one and the gradient was
   on the screen. It is answered below at `&.cm-editor .cm-button:active` — the
   base's shape plus one class on the element `&` already names. The
   `&.cm-gutters-before` and `&.cm-gutters-after` that the same package nests
   inside `&light .cm-gutters` are a third, a class deeper than the plain gutter
   rule below and setting a border width there; they are harmless, but only
   because the border-style tie at plain depth goes to us and a width on a style
   of none paints nothing. Measure against the mounted sheet before taking a
   plain rule here for a winning one.

   Bracket matching is repainted for a different reason: its base in
   @codemirror/language is a flat, unconditional colour that never looks at the
   darkTheme facet at all — so it would have to be overridden in any case,
   regardless of this flag.

   What the claim leaves out, and it is one thing rather than a list of defects:
   the base themes carry rules for classes **this app's extension set never puts
   in the DOM**, and a rule answering a class that cannot appear is a rule
   nobody could ever check. Four are @codemirror/view's — `.cm-placeholder`,
   `.cm-highlightSpace`, `.cm-highlightTab` and `.cm-trailingSpace`, each with a
   colour or a radial-gradient of its own, and every one of them belonging to an
   extension `extensions.js` does not build. Two more are @codemirror/merge's
   and are named beside the `diff` block below, where the same reasoning is set
   out for them: `.cm-deletedChunk` and `.cm-collapsedLines` belong to
   `unifiedMergeView` and `collapseUnchanged`, and this app builds neither. Add
   one of those extensions and its base rules become this file's to answer.

   Everything else the base themes would paint is answered below, including the
   two that were not until smetana-c2kw: the pressed button's gradient, and the
   ground @codemirror/language gives a bracket with no partner. */
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
  /* The selection, and it is two rules because @codemirror/view's base theme
     paints it with two: `&light .cm-selectionBackground` for a field that has
     lost the focus, and
     `&light.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground`
     for one that has it — two classes deep and five. Both go through &light,
     which with the darkTheme facet down (see the top of this file) is the branch
     the editor always takes, so both are that package's own hardcoded colours.
     What was here was a single two-class rule. It tied with the first, which
     our mount order won, and lost the second outright: selecting a whole file
     painted it in the base theme's lilac, in both app themes, with the text no
     longer readable over it. Only the focused state was ever wrong.

     So each is answered at its own shape, plus one class. `.cm-layer` is that
     class and it is the same element — LayerView puts cm-layer and
     cm-selectionLayer on the one div it appends to the scroller — which carries
     both rules past the base's rather than level with it, so neither depends on
     our style module being mounted after the base one. !important was the other
     way and is forbidden here.

     They are two keys and deliberately not one comma list, which is what
     style-mod would have emitted as a single rule with two selectors. Browsers
     take such a rule's specificity from whichever of its selectors matched, and
     happy-dom — the DOM this repository's tests run in — takes it from the first
     one written, so a list would have been a rule that behaves one way in the
     product and another way anywhere it could be checked. */
  '.cm-layer.cm-selectionLayer .cm-selectionBackground': {
    backgroundColor: 'var(--editor-selection)'
  },
  '&.cm-focused > .cm-scroller > .cm-layer.cm-selectionLayer .cm-selectionBackground': {
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
  /* Both bracket rules answer the colour *and* the ground, because
     @codemirror/language's base theme sets a ground for each and nothing else
     — `#328c8252` and `#bb555544`, hardcoded and never routed through &light or
     &dark, so a rule of ours that named only the colour left the package's own
     tint on the screen in both app themes. These two are the shape the base
     writes, exactly, and win on mount order rather than on depth: the tie and
     why it goes to us are the top of this file. The pair is deliberately not
     one colour with the loudness turned down — a bracket with no partner and a
     bracket with one are the same mark in two states, and the whole of the
     difference is which ground it takes. */
  '&.cm-focused .cm-nonmatchingBracket': {
    backgroundColor: 'var(--editor-bracket-unmatched-bg)',
    color: 'var(--syn-invalid)'
  },
  '.cm-specialChar': { color: 'var(--syn-invalid)' },

  /* The search panel. @codemirror/search's base theme paints it itself, and
     @codemirror/view's paints its buttons with a linear-gradient — one for the
     resting button and a second, nested inside the first as `&:active`, for the
     pressed one. Gradients are forbidden in this system, so both are answered
     below: `backgroundImage: 'none'` on the plain rule and again on the pressed
     one, which has to carry a class more than the base's to say it. */
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
  /* The pressed button, and the one rule in this block written at a depth of
     its own. @codemirror/view nests `&:active` inside `&light .cm-button`,
     which compiles to `.<base> .cm-button:active` — a class deeper than the
     plain `.cm-button` above, so the gradient suppressed there was never
     suppressed here. `&` is the editor element and `cm-editor` is a class it
     always carries, so `&.cm-editor` is that same element named twice: the
     base's own shape plus one class, which puts this rule past the base's
     rather than level with it. The same move the selection layer above makes,
     for the same reason — !important is the other way and is forbidden here. */
  '&.cm-editor .cm-button:active': {
    backgroundColor: 'var(--action-secondary-bg-active)',
    backgroundImage: 'none'
  },
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

   A line that is wholly new, or wholly gone, is therefore underlined end to end
   — the package's own long-standing look, since its gradient did the same — and
   there is **no narrower rule available**. `buildChunkDeco` wraps every chunk on
   side a in `<del class="cm-deletedLine">` and every chunk on side b in
   `<ins class="cm-insertedLine">` and then marks the changed sub-ranges *inside*
   that, so every `cm-changedText` in the view is a descendant of one of those
   two classes and neither of them tells a whole-line insertion from a
   one-character edit. A rule quietening them by that descent quietens the marks
   on every line, and then a one-character change on an otherwise identical line
   is invisible — which is exactly what shipped here for one commit and is worse
   than what the package draws by default. If the end-to-end underline is the
   wrong look, it is a colour question for the design system, not a selector to
   guess at.

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
  /* The 3px strip down the inside edge of the gutter: the one mark that says a
     line changed while the pane is scrolled past it. */
  '&.cm-merge-a .cm-changedLineGutter': { backgroundColor: 'var(--diff-removed-gutter)' },
  '&.cm-merge-b .cm-changedLineGutter': { backgroundColor: 'var(--diff-added-gutter)' },
  /* The filler rows @codemirror/merge inserts to keep the two sides level. They
     are not lines of either document and must not read as one, so they take the
     gutter's own ground rather than the editor's. */
  '.cm-mergeSpacer': { backgroundColor: 'var(--editor-gutter-bg)' }
})

/* The conflict markers a merge leaves in a file, drawn as structure rather than
   as text — `conflictHighlight.js` puts the classes on the lines, this says what
   they come to. The current side takes one ground and the incoming side
   another, because telling the two apart is the whole point; the four marker
   lines take the conflict's colour and no ground of their own.

   Reusing `--diff-added-bg` / `--diff-removed-bg` was rejected: in this same
   window those two already mean "added" and "removed". Painting both sides
   `--git-conflict` was rejected for the opposite reason — it would say there is
   a conflict and refuse to say which side is which.

   **The selectors carry `.cm-content` for the cascade, and that is load-bearing
   rather than tidy.** Inside `DiffView` the very same `.cm-line` element already
   carries `cm-changedLine`, painted above by `&.cm-merge-b .cm-changedLine` —
   which, after `buildTheme` prefixes it with the theme's own class, is three
   classes deep. A rule written as `.cm-line.cm-sm-conflict-current` is three
   deep too, and an exact tie is settled by the order the style modules reach the
   document. That order is **the reverse of this array**: `updateStyleModules`
   mounts `facet.concat(baseTheme).reverse()`, and style-mod gives precedence to
   whatever is later in the array it is handed — so a block placed *after* `diff`
   here has its rules inserted *before* diff's and loses every tie. Measured, not
   assumed. `.cm-content` makes it four deep, which wins outright and keeps
   winning whatever order these three constants are assembled in.

   The colour of a marker line has to reach the spans inside it as well as the
   line: the language mode has already coloured `<<<<<<< HEAD` as operators and
   an identifier, and a colour on the line alone is inherited text that every one
   of those spans overrides. */
const conflict = EditorView.theme({
  '.cm-content .cm-line.cm-sm-conflict-current': {
    backgroundColor: 'var(--conflict-current-bg)'
  },
  '.cm-content .cm-line.cm-sm-conflict-incoming': {
    backgroundColor: 'var(--conflict-incoming-bg)'
  },
  '.cm-content .cm-line.cm-sm-conflict-marker, .cm-content .cm-line.cm-sm-conflict-marker span': {
    color: 'var(--git-conflict)',
    fontWeight: 'var(--weight-medium)'
  }
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

/* The diff and conflict rules ride with the rest rather than as extensions of
   their own: the classes they name appear in a merge view and in a file left
   mid-merge, so an ordinary editor carries a handful of rules that match
   nothing, against the alternative of three theme exports that could be
   assembled in the wrong order. The order below decides nothing on its own —
   see the note above `conflict` for why, and for why that is deliberate. */
export const editorTheme = [chrome, diff, conflict, syntaxHighlighting(syntax)]
