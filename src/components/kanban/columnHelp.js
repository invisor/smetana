/* What a board column is, and what kind of task lands in it.

   The `branchChoice.js` / `columnOrder.js` / `parked.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — no test in
   this repository can reach a `.vue`, so a rule left inside a component is a
   rule nothing checks.

   It is deliberately not a line beside the glyphs in `status/status.js`. That
   file is the design-system layer — colour, loudness, glyph — and answers what
   a status *looks* like; "which tasks end up here" is knowledge about this
   board and this project's way of working: runs, parking, findings that turned
   up during a review. Two different questions, two different files.

   Nothing here explains the tracker or how the board is built. A person hovering
   a column head is asking about their tasks, not about bd. That is also why
   `blocked` says "something it depends on", not "the parent task": on this board
   `parent-child` does not block at all — only bd's `blocks` dependency does
   (`src/stores/tracker.js`) — and a hint that taught otherwise would be worse
   than none. */
import { normalizeStatus } from '../status/status.js'

/* How long the pointer has to stay on the header before the panel appears.
   Long, because this is prose somebody reads rather than a control's own name:
   a column head is passed over constantly on the way to a card, and a hint that
   opened on the way past would be in the way rather than of use. */
export const COLUMN_HELP_DELAY = 2000

/* Keyed by `normalizeStatus`, so `ready_to_merge` and `ready-to-merge` are one
   entry. No `needs-you` and no `failed`: those are attention levels rather than
   statuses a task is filed under, and no column on this board carries either
   name. */
const HELP = {
  ready:
    'Ready to start — nothing unfinished is holding it up. A run takes its batch from here.',
  running: 'Being worked on right now: an agent has claimed it and is on it.',
  blocked:
    'Waiting on another task: something it depends on is not finished. When that one is done, the card moves to Ready on its own.',
  done: 'Finished and closed. Nothing left to do here.',
  deferred:
    'Put off on purpose, with nothing holding it up. Findings that turned up outside their own task land here. A run never picks one up — only a person moves it back to Ready.',
  pinned:
    'A standing item that is never closed. It stays out of the queue and holds nothing else up.',
  hooked:
    'An agent has taken a whole group of related tasks at once. It says who owns the work, not how far it has got, and a run leaves these alone.',
  parked:
    "A run stopped here on a question it could not answer itself. The question is in the task's notes; Answer questions starts an agent that puts it to you and returns the task to the queue.",
  'ready-to-merge':
    'Done and reviewed, waiting to be merged into the target branch. It closes once it lands there.',
  'human-check':
    'Done and merged, waiting for someone to look at it by hand. A run leaves one of these behind when it could not check the work itself; you go through it, then close it or send it back to Ready.'
}

/* A column whose status is not in the table is an ordinary outcome rather than
   a gap — the set of columns is bd's and a project invents its own statuses —
   so it gets a sentence of its own instead of silence. */
const UNKNOWN =
  'A status this app knows nothing about. Tasks are here because that is the status they carry.'

export function columnHelp(status) {
  return HELP[normalizeStatus(status)] || UNKNOWN
}
