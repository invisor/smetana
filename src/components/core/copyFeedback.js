import { onScopeDispose, ref } from 'vue'
import { COPIED_MS } from '../kanban/copyId.js'

/* What happens on screen after somebody copies something, for every control in
   this app that copies onto itself rather than into a toast.

   It is here, under `core/` and beside `interactive.js`, for that file's
   reason: a composable is a thing several components' owners want, and this
   tree keeps exactly one precedent for one — `useInteractive`, which uses Vue
   and lives under `components/` because more than one caller wants it. This is
   the second.

   **The policy is one thing and it was written out four times.** Clear the
   pending timer, claim the target, blank the outcome, `await` the write, bail
   if a later press has taken the state over, clear the timer a second time, set
   the outcome, arm the reset. Two of those copies — a task's id in
   `views/DesktopApp.vue` and the same in `views/Gallery.vue` — were named in
   this project's hazards list as having already cost once: a stranded reset
   timer sat in both, and a copy that had actually worked showed no confirmation
   at all, and the fix had to be made twice by hand. The session card's menu
   then added two more. The gallery is this project's only verification of
   anything under `src/components/`, so a copy fixed in the app and not in the
   harness leaves the harness reproducing a defect the product no longer has —
   which is indistinguishable by eye from a real one.

   The clipboard is taken in rather than imported: the only files in `src/` that
   know Tauri exists are the stores, and `stores/app.js` owning `copyText` is
   what keeps the narrow `clipboard-manager:allow-write-text` capability the one
   verb this app writes with. A composable that reached for it would put Tauri
   under `components/`, and would put it inside the one runner that can see this
   rule at all. `write` is `(text) => Promise<boolean>`, which is `copyText`'s
   own shape.

   One target at a time, deliberately: the state is a single id and a single
   outcome, so copying a second thing takes the confirmation off the first — two
   controls both reading `Copied` would be a claim about a clipboard that holds
   one thing.

   How long a confirmation stands is `COPIED_MS`, borrowed from
   `kanban/copyId.js` rather than declared again. That number was written out
   three times before that module took it, and it stays one number by being
   borrowed here too — this file is the only thing in the tree that waits it
   out now. Borrowed from there and not moved here: `copyId.js` is one of this
   tree's pure rule modules, with no Vue and no DOM in it, and pointing it at a
   composable would end that. Reaching across two groups for it is what
   `agent/sessionMenu.js` already does with the same constant, for the same
   reason. Nothing here wants a token instead: `tokens/motion.css` is about
   transitions, and this is a dwell. */

/**
 * The copy-confirmation policy, once.
 *
 * @param {(text: string) => Promise<boolean>} write what puts it on the
 *   clipboard — `copyText` from `stores/app.js` in both windows that call this.
 * @returns {{
 *   target: import('vue').Ref<*>,
 *   state: import('vue').Ref<string>,
 *   noun: import('vue').Ref<string>,
 *   stateFor: (id: *) => string,
 *   nounFor: (id: *) => string,
 *   copy: (id: *, text: string, noun?: string) => Promise<void>
 * }}
 */
export function useCopyFeedback(write) {
  /* What was copied last, by whatever id its owner tells rows apart by. */
  const target = ref(null)
  /* '' | 'copied' | 'failed' */
  const state = ref('')
  /* Which of several things it was, for the callers whose confirmation names
     it. A task's id has nothing to name and leaves this empty. */
  const noun = ref('')
  let timer = null

  /* What one row gets: its own outcome, and nothing for anybody else's. The
     `id != null` guard is the whole of why a board full of cards with no id
     yet does not light up the moment the state goes back to null. */
  const stateFor = (id) => (id != null && id === target.value ? state.value : '')
  const nounFor = (id) => (id != null && id === target.value ? noun.value : '')

  async function copy(id, text, verb = '') {
    clearTimeout(timer)
    /* Claimed before the await, and with no outcome yet: the write takes a
       moment in the app, and until it answers the previous row must already
       have stopped saying it was copied. */
    target.value = id
    state.value = ''
    noun.value = verb
    /* Nothing to copy is a refusal rather than a copy of the empty string: a
       clipboard emptied by a press is worse than one left alone, and the
       control says which it was. */
    const ok = text ? await write(text) : false
    // A second press, on this row or another, has taken the state over since.
    if (target.value !== id) return
    /* Again, and this is not the same clear as the one above. Two presses on
       the same row both get past that guard, and the second one's `setTimeout`
       would overwrite the first's handle while the first timer went on running
       with nothing pointing at it. It then fires `COPIED_MS` after the *first*
       copy resolved: soon enough to cut this confirmation short, and — since it
       puts `target` back to null — soon enough to make a later copy's own guard
       bail on it, so a copy that worked would say nothing at all. A
       double-click is the most ordinary way there is to point at a word
       somebody wants. This is the line the hazards list is about. */
    clearTimeout(timer)
    state.value = ok ? 'copied' : 'failed'
    timer = setTimeout(() => {
      target.value = null
      state.value = ''
      noun.value = ''
    }, COPIED_MS)
  }

  /* `onScopeDispose` rather than `onUnmounted`, which is what the four copies
     each carried: inside a component's `setup` the two fire at the same moment,
     and this one also holds for an `effectScope` — which is how a test reaches
     a composable at all. */
  onScopeDispose(() => clearTimeout(timer))

  return { target, state, noun, stateFor, nounFor, copy }
}
