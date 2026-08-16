/* Which of the two painted themes is on the document root right now, as a
   reactive value.

   `appearance.js` answers what a *stored* theme means and stays pure;
   `views/useAppearance.js` is what writes the answer onto the root. This is the
   third piece and the one neither of them is: what is on the root *now*, for
   code that has no access to the settings store and no business asking it —
   `stores/tabs.js` builds tab icons, and a store reaching into another store for
   a person's theme preference would be the wrong dependency entirely.

   An attribute observer rather than a settings subscription, for the reason
   `TerminalView.vue` observes the same attribute: the root is where every window
   agrees, so this works in the settings window and under `?view=gallery`, where
   the theme comes from a query parameter and no store holds it at all.

   `dark` is the answer before any attribute exists — it is the shipped default,
   and it is what `?view=gallery` paints without a `theme` parameter. */
import { readonly, ref } from 'vue'

const current = ref('dark')

const read = () => {
  const theme = document.documentElement.getAttribute('data-theme')
  current.value = theme === 'light' ? 'light' : 'dark'
}

/* Guarded because the module is a singleton and a test rebuilding the store
   graph imports it again. There is no matching `disconnect`: the observer lives
   as long as the document does, which is as long as the module does. */
if (typeof document !== 'undefined' && typeof MutationObserver !== 'undefined') {
  read()
  new MutationObserver(read).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-theme']
  })
}

export const documentTheme = readonly(current)
