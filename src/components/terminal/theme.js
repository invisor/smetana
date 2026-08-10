/* xterm.js theme built from design-system tokens.

   This is the second and last exception to the "tokens only, through var()"
   rule, and it is not shaped like the first. EditorView.theme() takes CSS,
   so var(--token) works there and the browser repaints the editor for free
   when data-theme changes. xterm.js takes an ITheme — an object of resolved
   colour strings in JS. That is the consequence CodeMirror does not have:
   a theme switch does not repaint the terminal for free, and the theme has
   to be recomputed and reassigned — that is what the MutationObserver in
   TerminalView.vue is for.

   The rule narrows rather than lifts: every value still comes from a token,
   just read instead of substituted. Not one literal colour, size or font
   name appears in this file. */
const ANSI = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white']

const read = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim()

export function terminalTheme() {
  const theme = {
    background: read('--editor-bg'),
    foreground: read('--text-primary'),
    cursor: read('--editor-cursor'),
    cursorAccent: read('--editor-bg'),
    selectionBackground: read('--editor-selection')
  }
  for (const name of ANSI) {
    theme[name] = read(`--ansi-${name}`)
    theme[`bright${name[0].toUpperCase()}${name.slice(1)}`] = read(`--ansi-bright-${name}`)
  }
  return theme
}

/* xterm wants the font size as a number, not a CSS string — and a custom
   property is no longer one to read. The type scale is
   `calc(<n> * var(--ui-scale) * 1px)` so the app-wide font size can be a factor
   in the stylesheet (tokens/typography.css), and the computed value of an
   unregistered custom property is its text with `var()` substituted and `calc()`
   left standing: `getComputedStyle` hands back the expression, and `parseFloat`
   would read the first number in it — 11 — at every setting. `@property` would
   make it compute to a length and needs a newer Safari than this build targets.

   So the browser is asked to do the arithmetic the one way it will: an element
   whose `font-size` *is* the token, whose computed style is therefore a resolved
   length. It is thrown away immediately; this runs on a theme or font change,
   not per frame. Deliberately no fallback number — a token that stops resolving
   is a design-system bug and should surface as a broken terminal rather than as
   a size this file invented. */
function readSize(name) {
  const probe = document.createElement('div')
  probe.style.cssText = `position:absolute;visibility:hidden;font-size:var(${name})`
  document.documentElement.appendChild(probe)
  const size = parseFloat(getComputedStyle(probe).fontSize)
  probe.remove()
  return size
}

export function terminalFont() {
  return {
    fontFamily: read('--font-mono'),
    fontSize: readSize('--text-xs')
  }
}
