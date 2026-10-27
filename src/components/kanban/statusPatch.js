/* What a status write sends to bd, as a rule rather than as a line inside a component.

   The `taskMenu.js` / `lockCascade.js` family: pure, no Vue and no DOM. The desktop's card menu and
   the phone's task sheet both write a status the same way, and a patch built twice is a patch
   that drifts, so the one rule lives here.

   A `promoted:` note rides beside the status in the same `bd update` only where the write is
   genuinely a promote: the target is ready and the issue's own status (`before`, read fresh by
   the caller and not the menu's possibly stale copy) still needs the move. Every other status the
   menu can write - done, blocked, a project's own custom one, or ready written over a task
   already there - gets no note, since nothing was promoted (smetana-fpw7). */
import { needsReady } from '../run/readyPromote.js'
import { promotedNote } from '../run/promotedNote.js'
import { READY } from './parked.js'

export function statusPatch(before, status) {
  return status === READY && needsReady(before) ? { status, append_notes: promotedNote('status') } : { status }
}
