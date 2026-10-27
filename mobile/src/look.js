/* What the phone's page is drawn in: the theme and the density, from what the desktop says and what
   the phone itself knows.

   Pure: the system's answer and the page's query are handed in, so that a test needs no DOM.

   The desktop's `system` means the phone's own system theme, not the desktop's, so it is resolved
   here by `effectiveTheme`, the rule the desktop's windows use. A value this file has never heard of
   reads the same way for the theme (it is not forced to `dark`) and as `comfortable` for the
   density. `?theme=` and `?density=` win over everything, for the browser, where the four looks have
   to be checked by eye (CLAUDE.md, Commands). The font size is not here: it is not copied. */
import { effectiveTheme } from '@src/appearance.js'

const DENSITIES = ['comfortable', 'compact']
const THEMES = ['dark', 'light']

export function phoneLook(look, prefersDark, params) {
  const queryTheme = params?.get('theme')
  const queryDensity = params?.get('density')
  const density = DENSITIES.includes(queryDensity)
    ? queryDensity
    : DENSITIES.includes(look?.density)
      ? look.density
      : 'comfortable'
  const theme = THEMES.includes(queryTheme) ? queryTheme : effectiveTheme(look?.theme, prefersDark)
  return { theme, density }
}
