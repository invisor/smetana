/* The note a human promote leaves beside the status it writes, and the one
   rule the places in `DesktopApp.vue` that can move a task to ready by a
   person's own hand all read, rather than each inventing its own sentence —
   four now that Unblock (smetana-44mw) joins starting a run over a card,
   promoting a whole Deferred column, and a direct status change from the card
   menu or the inspector header.

   The `readyPromote.js` family: pure, with no Vue and no DOM in it, and a file
   of its own for the reason that family exists — a `.vue` file is the one
   thing no test in this repository can reach, so a rule left inside the
   component that draws it is a rule nothing checks.

   The defect this closes (smetana-fpw7): `deferred` is how a run's own
   findings stay out of `bd ready` — only a person moves one to `open` — but a
   person doing that wrote nothing except the status itself. A lead seeing the
   same task back in `bd ready` a batch later had no way to tell "a person
   promoted this" from "the status slipped", read it as the second, and wrote
   it back to `deferred` — undoing somebody's decision three times running on
   the same three tasks. The status write already carries `append_notes`
   (`IssuePatch` in `src-tauri/src/tracker/bd.rs`), so the trace costs no new
   field and no second call: one `bd update` sets the status and appends this
   note in the same breath.

   `promoted:` is the marker, matching the two it sits beside — `parked:`
   (`kanban/parked.js`) and `resolved:` — the same convention read by a
   program: two words it can grep for, then prose in the project's other
   language for whoever reads the task next. */

const MARKER = 'promoted:'

/* One sentence per call site, because "a person did this" is not enough on
   its own — the whole point is telling a lead *where* the promote came from,
   so the trail reads as an event rather than a stamp repeated three times
   with nothing to tell them apart. Each is Russian prose after the marker,
   matching `parked:` and `resolved:`; the marker itself stays English so a
   program can still find it. */
const REASON = {
  /* `DesktopApp.vue`'s `startTheRun`, over one card's Run button —
     `readyPromote.js`'s own `promotesToReady` gates the write this note rides
     beside. */
  run: 'человек нажал Run на карточке, и запуск прогона перевёл задачу в ready.',
  /* `DesktopApp.vue`'s `confirmPromote`, behind `PromoteColumnModal` —
     the whole Deferred column moved in one gesture. */
  column: 'человек продвинул колонку Deferred целиком через Promote to ready.',
  /* `DesktopApp.vue`'s `setTaskStatus`, behind both the card's own menu and
     the Task & details header — including `moveToReadyAnyway`, which is the
     same write after the open-question warning. */
  status: 'человек перевёл задачу в ready из меню карточки или из заголовка инспектора.',
  /* `DesktopApp.vue`'s `toggleLock`, the Unblock half of the manual lock
     (smetana-44mw) — the card's own menu and the Task & details header again,
     the same two doors `status` above already answers for. Block writes no
     note at all: the stored `blocked` status is the whole of the record, and
     the actor already stands in bd's own history. Unblock needs one because a
     lead who sees the task back in `bd ready` a batch later has no other way
     to tell a person's own release from a status that merely slipped — the
     same gap `run`, `column` and `status` above already close, one write
     later in a task's life. */
  unblock: 'человек снял блокировку с задачи из меню карточки или из заголовка инспектора.'
}

/* '' for a source this file has not been told about, deliberately: a caller
   passing an unknown key is a programming error, and writing no note is the
   failure that leaves the least behind — a status change with no note at all
   is still less wrong than a marker with nothing readable after it. */
export function promotedNote(source) {
  const reason = REASON[source]
  return reason ? `${MARKER} ${reason}` : ''
}
