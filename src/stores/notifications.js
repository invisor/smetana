/* What the bell has to say, and the one source that says anything today.

   This file knows nothing about Tauri: the back end it reads is
   `attachments.js`, which already owns the one command that weighs the store
   (`attachments_survey` — the very command the settings window's Storage tab
   reads, so the screen a person is sent to and the card that sent them there
   cannot quote different numbers). What is here is the wiring between a
   measurement, the number the project remembers, and the list a panel draws.

   **The list is derived.** Nothing about a notification is written to disk
   except one number per project — the highest threshold already announced —
   and that number lives in `settings.json` beside the column order, because the
   folder it is about is per project too. A stored inbox of past events was
   considered and dropped in the design: everything worth announcing here is a
   statement about something the app can look at right now, so a stored copy
   goes stale the moment the thing it describes moves, and the failure is a bell
   shouting about a folder somebody emptied an hour ago. The cost is accepted
   and named: there is nothing to say about the past.

   A card is not thrown away on the next measurement, though — it stands until
   somebody answers it or until it stops being true (`stillOver`). Announcing is
   the moment a threshold is crossed; the card is what that moment left behind. */
import { reactive } from 'vue'
import { surveyStorage } from './attachments.js'
import { settings } from './settings.js'
import {
  crossedThreshold,
  projectBytes,
  rememberAfter,
  stillOver,
  storageNotification
} from '../components/notifications/notifications.js'

export const notificationsState = reactive({
  /* Whole notification objects, newest first if there is ever more than one.
     The badge counts this, the panel draws it. */
  items: []
})

/* The storage source is one card at a time — one folder, one statement about
   it — so a new one replaces the old rather than stacking beside it. */
const dropStorage = () => {
  notificationsState.items = notificationsState.items.filter((item) => item.source !== 'storage')
}

const put = (notification) => {
  dropStorage()
  notificationsState.items = [notification, ...notificationsState.items]
}

const storageCard = () => notificationsState.items.find((item) => item.source === 'storage') ?? null

/* Weigh the active project's share of the attachment store and say something
   about it if there is anything new to say.

   Called when the project is resolved at start, when it changes, when the window
   takes focus, and after an attachment is saved — the same answer the file tree
   and the branch give, and for the same reason: a watcher over the app's own
   data directory would be a second watcher subsystem with its own lifecycle for
   a number that costs milliseconds to read.

   Two guards on the answer, and both are the same guard `git.js` and
   `terminals.js` carry: this can be in flight while somebody switches project,
   and without them the *last response* would win rather than the *last call* —
   one project's size announced under another project's name, and, worse, its
   threshold written into the other project's settings entry. `Survey.project` is
   what Rust actually measured, which is the stronger half: the survey is
   answered against the tracker worker's idea of the active project, and that
   moves a moment after this front end's does. */
export async function measureStorage(project) {
  /* A card belonging to somewhere else goes first, before anything is asked and
     whatever the answer turns out to be. The warning is about the active
     project's folder and only that — a neighbouring project's folder says
     nothing until somebody works in it — so a card left over from the project
     just departed would be a statement about a folder this window is no longer
     looking at, and one made under the wrong project's name. */
  const left = storageCard()
  if (left && left.project !== project) dropStorage()
  if (!project) return

  let survey
  try {
    survey = await surveyStorage()
  } catch {
    /* `attachments.js` has already logged it. A read that failed says nothing
       new about the folder, so nothing on screen changes: the card that is
       there was true when it was made, and the absence of one is not evidence. */
    return
  }
  if (settings.activeProject !== project || survey?.project !== project) return

  const bytes = projectBytes(survey)
  /* No answer, which today means no readable board — the survey counts nothing
     in that state by design, and a zero read as a size would be a lie in both
     directions: it would announce nothing about a folder that may be full, and
     it would re-arm a ladder off a number nobody measured. So an unreadable
     board changes nothing at all — no card is made, no card is taken away, and
     the remembered threshold stays where it is.

     It also keeps the card honest about its own action: "Clean up" leads to a
     button Rust refuses while the board cannot be read, and a warning whose
     action cannot be carried out is worse than silence. */
  if (bytes === null) return

  const announced = settings.project.storageWarnedMib ?? null
  const crossing = crossedThreshold(bytes, announced)
  if (crossing !== null) put(storageNotification(project, bytes, crossing))
  else {
    /* A card that is already up stands until it stops being true — the folder
       cleaned back under its threshold takes it with it, and the same
       measurement re-arms the ladder below. While it stands it is rewritten
       from the size just measured rather than left with the one it was made
       from: the card is a statement about the folder now, and a folder that
       grew from 12 MiB to 40 without reaching the next step would otherwise be
       described by a number half an hour old. The id is the project and the
       threshold, so this replaces the card rather than adding one. */
    const card = storageCard()
    if (card && !stillOver(bytes, card.threshold)) dropStorage()
    else if (card) put(storageNotification(project, bytes, card.threshold))
  }

  /* Announcing and re-arming are one write, which is why there is no separate
     "already warned" flag: the number is set to whatever the folder still
     reaches, every time. */
  settings.project.storageWarnedMib = rememberAfter(bytes, announced)
}

/* Dismiss. For a storage card this is exactly the write an announcement makes,
   for the size measured at that moment — which is why there is no dismissed
   flag anywhere: the card is derived, and a threshold recorded as announced is
   the whole of what "I have seen this" means. The card comes back when the
   folder passes the next threshold, or when it falls under this one and passes
   it again. */
export function dismiss(id) {
  const card = notificationsState.items.find((item) => item.id === id)
  if (!card) return
  if (card.source === 'storage' && settings.activeProject === card.project) {
    settings.project.storageWarnedMib = rememberAfter(card.bytes, card.threshold)
  }
  notificationsState.items = notificationsState.items.filter((item) => item.id !== id)
}
