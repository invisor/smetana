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

/* xterm wants the font size as a number, not a CSS string — the token is
   read and parsed here so no literal has to appear in the component. No
   fallback number either: --text-xs is defined unconditionally, and a
   missing token is a design-system bug that should surface as a broken
   terminal, not be quietly papered over with a size this file invented. */
export function terminalFont() {
  return {
    fontFamily: read('--font-mono'),
    fontSize: parseFloat(read('--text-xs'))
  }
}
