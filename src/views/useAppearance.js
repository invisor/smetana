/* The DOM half of appearance, shared by the two views that have a document root
   to paint: the app window and the settings window. The rules themselves are in
   `../appearance.js` and stay pure; what is here is the part that needs a
   window, and it is here rather than there so that file remains testable. */
import { onUnmounted, ref } from 'vue'
import { UI_FONT_DEFAULT, clampFont, fontVars } from '../appearance.js'

/* Whether the machine is currently asking for a dark interface. A ref rather
   than a reading, and with a listener rather than a look at startup: a laptop
   that switches theme at sunset would otherwise leave the app wrong for the
   whole evening, which is exactly the case `system` exists for. */
export function usePrefersDark() {
  const query = window.matchMedia('(prefers-color-scheme: dark)')
  const prefersDark = ref(query.matches)
  const onChange = (event) => {
    prefersDark.value = event.matches
  }
  query.addEventListener('change', onChange)
  onUnmounted(() => query.removeEventListener('change', onChange))
  return prefersDark
}

/* Everything a window's document root carries about how it looks: both switches
   every token is defined against, and the type scale at the chosen size.

   The sizes are written as custom properties on the root, which is `:root` — so
   they override the stylesheet's own definitions and every `var(--text-…)` in
   the app resolves to them, with no component changed and nothing rebuilt.

   `data-ui-font` carries no value anybody reads: it exists so that a font change
   is an *attribute* change on the root. xterm.js is handed resolved numbers
   rather than tokens (see `components/terminal/theme.js`), so the terminal is
   the one thing that does not repaint itself — it watches this root's attributes
   and re-reads on any of them. A custom property written into the `style`
   attribute would have done the same job by accident; this says so on purpose. */
export function paintRoot(el, { theme, density, uiFontSize, editorFontSize }) {
  el.setAttribute('data-theme', theme)
  el.setAttribute('data-density', density)
  for (const [name, value] of Object.entries(fontVars(uiFontSize, editorFontSize))) {
    el.style.setProperty(name, value)
  }
  el.setAttribute('data-ui-font', String(clampFont(uiFontSize, UI_FONT_DEFAULT)))
}
