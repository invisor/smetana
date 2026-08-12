/* What the bell has to say, and the two sources that say anything today: the
   attachment store growing, and a run that is over.

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
/* **Nothing in this module may read `runsState` at evaluation time** — only
   inside `syncRunCards`, which runs long after both modules are up. The import
   is one half of a cycle (`runs.js` calls `syncRunCards`), and the bundler
   emits *this* module first, before `runs.js` has evaluated and before the
   `const runsState` exists: a module-scope `watch(() => runsState.runs)` — the
   natural-looking improvement over the explicit calls in `runs.js` — would
   therefore throw on the very first line of the built app and leave a white
   window, while working perfectly in `npm run dev`, where the browser's own
   module order is the other way round. Verified against the emitted chunk, not
   assumed. */
import { runsState } from './runs.js'
import {
  crossedThreshold,
  projectBytes,
  rememberAfter,
  runNotification,
  stillOver,
  storageNotification
} from '../components/notifications/notifications.js'

export const notificationsState = reactive({
  /* Whole notification objects, in `SOURCES` order. The badge counts this, the
     panel draws it. */
  items: []
})

/* The order the panel draws its sources in, and the whole of it.

   It is a property of the list rather than of who spoke last, and that is the
   fix for what the two sources did while each arranged its own half: the
   storage card was prepended when it was announced, run cards were put in front
   of everything when the list moved, so whichever source had most recently had
   something to say sat on top and the panel had no order anybody could rely on.
   Both writers now hand their result to `arrange`, so the sequence is the same
   however the cards got there.

   Runs first: a night that has ended is what somebody came back to the window
   to read, while a folder that has grown is housekeeping and will still be
   there tomorrow. A source this list has never heard of sorts to the end rather
   than to the front, since an unknown card has not earned the top of a panel
   budgeted at one or two rows. */
const SOURCES = ['run', 'storage']

const rank = (item) => {
  const at = SOURCES.indexOf(item.source)
  return at === -1 ? SOURCES.length : at
}

/* `sort` is stable, so cards of one source keep the order their own writer gave
   them — run cards oldest run first, as `runsState.runs` holds them. */
const arrange = (items) => [...items].sort((a, b) => rank(a) - rank(b))

/* The storage source is one card at a time — one folder, one statement about
   it — so a new one replaces the old rather than stacking beside it. */
const dropStorage = () => {
  notificationsState.items = notificationsState.items.filter((item) => item.source !== 'storage')
}

const put = (notification) => {
  dropStorage()
  notificationsState.items = arrange([notification, ...notificationsState.items])
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

/* ---- the second source: runs that are over --------------------------- */

/* Runs whose card somebody has taken away, by token, in memory and nowhere
   else. There is nothing for a stored flag to be about: a run does not survive
   a restart any more than a session does, so the list it is derived from starts
   empty on every launch and a remembered token would refer to nothing. The
   token is issued once per app process and never reused, so this set cannot
   silence a later run — not even one in another project. */
const dismissedRuns = new Set()

/* The run cards, rebuilt from `runsState.runs`.

   **Derived, never accumulated**, which is the whole reason this bell is not an
   inbox: a card exists while its stopped run is in that list and goes when the
   run does — the project changing, or a run of the same scope replacing it,
   which is the rule `runs.js` already keeps and the reason a stopped run stays
   on the bar at all. Nothing here remembers that a card was ever made, so there
   is no second source of truth to go stale, and no run ends up announced twice.

   Cards of other sources are left exactly as they are: this rebuilds one
   source's half of the list and knows nothing about the rest of it.

   The list it reads is the active project's — `runs.js` guards it against its
   own stale responses and against events arriving after a switch — so there is
   no project check here to keep in step with that one. What this must not do is
   read anything else about the project, and it does not. */
export function syncRunCards() {
  const cards = runsState.runs
    .filter((run) => !dismissedRuns.has(run?.token))
    .map(runNotification)
    .filter(Boolean)
  const others = notificationsState.items.filter((item) => item.source !== 'run')
  notificationsState.items = arrange([...cards, ...others])
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
  /* A run card is derived from a list that outlives the card, so taking it out
     of `items` alone would put it straight back on the next sync. The token is
     remembered instead — and only in memory, for the reason `dismissedRuns`
     gives. */
  if (card.source === 'run') dismissedRuns.add(card.token)
  notificationsState.items = notificationsState.items.filter((item) => item.id !== id)
}
