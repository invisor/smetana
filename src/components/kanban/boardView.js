/* What of the board is actually drawn — which columns, and which of their cards.

   The same shape as `columnOrder.js` beside it, and pulled out for the same
   reason: pure, no Vue and no DOM, which is what makes it the one part of this
   a test can reach. `DesktopApp.vue` composes the two — `orderColumns` first,
   then this — and the order is deliberate: the sequence of the columns is a
   property of the whole board and must not depend on which of them happen to be
   on screen today, or a column would come back from a hidden spell somewhere it
   never was.

   Two settings live here, both global (`kanban` in `settings.json`): which
   columns to draw, and how far back to look. Both default to today's behaviour
   exactly — every column, every task — so nothing on anybody's screen moves
   until they go and choose. */

/* The closed lists. Written out here and again in
   `src-tauri/src/settings/model.rs`, the same doubling `SIDE_TABS` and the
   storage ladder carry, and with the same obligation: what this offers must be
   a subset of what Rust accepts, because a value Rust refuses loses itself on
   the next save with nothing on screen to say so. */
export const COLUMN_MODES = ['all', 'non-empty']
export const INTERVALS = ['all', 'day', 'week', 'month']

/* What the two dropdowns offer, labels included — the settings tab draws these
   rather than writing its own, so the vocabulary and the words about it cannot
   drift apart. */
export const COLUMN_MODE_CHOICES = [
  { value: 'all', label: 'All columns' },
  { value: 'non-empty', label: 'Only columns with tasks' }
]
export const INTERVAL_CHOICES = [
  { value: 'all', label: 'All time' },
  { value: 'day', label: 'Last day' },
  { value: 'week', label: 'Last week' },
  { value: 'month', label: 'Last month' }
]

/* The shipped values, mirroring Rust's `KanbanSettings::default`. Today's board
   exactly. */
export const KANBAN_DEFAULTS = { columns: 'all', alwaysShow: [], interval: 'all', unlimited: [] }

const DAY = 24 * 60 * 60 * 1000
/* A month is thirty days rather than a calendar one: the question a person is
   asking is "roughly the last month", and a rule that changed length in
   February would be a second thing to explain for no gain. */
const WINDOW = { day: DAY, week: 7 * DAY, month: 30 * DAY }

/* A list of column names off the settings file or off an event: strings only,
   nothing empty, no duplicates. The same shape `sane_list` gives it in Rust —
   which is the authority; this copy is what keeps a malformed event from
   putting a number into the list the board filters on. */
export function columnNames(list) {
  if (!Array.isArray(list)) return []
  const seen = new Set()
  return list.filter(
    (name) => typeof name === 'string' && name !== '' && !seen.has(name) && seen.add(name)
  )
}

/* Whatever is in the settings, made safe to reason about. A field outside its
   closed list reads as the shipped value rather than emptying the board, which
   is the same leniency Rust applies to the file. */
export function readKanban(kanban) {
  const source = kanban ?? {}
  return {
    columns: COLUMN_MODES.includes(source.columns) ? source.columns : KANBAN_DEFAULTS.columns,
    alwaysShow: columnNames(source.alwaysShow),
    interval: INTERVALS.includes(source.interval) ? source.interval : KANBAN_DEFAULTS.interval,
    unlimited: columnNames(source.unlimited)
  }
}

/* Is this task inside the window — measured on `updated_at`, which is the one
   of bd's three dates that is always there, so the rule has no holes and needs
   no second answer for a task without one. It is also the question the board is
   really being asked: a task filed a month ago and picked up this morning stays
   in view, because it moved.

   The price is named rather than discovered: any write by an agent freshens the
   date, so a night's run pulls old tasks back into view — which is the truth
   about them, they moved.

   **A date that cannot be read means show the task.** Of the two ways to be
   wrong, one card too many costs a glance and one card too few costs somebody's
   work. */
export function withinInterval(task, interval, now) {
  const span = WINDOW[interval]
  if (span == null) return true
  const stamp = Date.parse(task?.updatedAt ?? '')
  if (Number.isNaN(stamp)) return true
  return now - stamp <= span
}

/* The whole rule, in one and only one order: the interval first, over the tasks
   of every column but the ones named `unlimited`, and emptiness afterwards —
   judged on what is *left*. A column the interval swept clean reads as empty
   and goes, unless `alwaysShow` names it.

   Judging emptiness against the whole board instead would leave visibly empty
   columns on screen, which is exactly what the first setting is for. */
export function visibleColumns(board, kanban, now) {
  const rules = readKanban(kanban)
  const columns = (board ?? []).map((column) =>
    rules.interval === 'all' || rules.unlimited.includes(column.status)
      ? column
      : { ...column, tasks: column.tasks.filter((task) => withinInterval(task, rules.interval, now)) }
  )
  if (rules.columns !== 'non-empty') return columns
  return columns.filter(
    (column) => column.tasks.length > 0 || rules.alwaysShow.includes(column.status)
  )
}

/* The order to store after a drag, when part of the board was not on screen.

   This is not decoration. `KanbanBoard` emits the order it *drew*, and
   `DesktopApp` writes that into `project.columnOrder` whole — so with a column
   hidden, the first drag would strike its name out of the stored order, and
   `orderColumns` would then append it to the end of the board the day it came
   back. A column jumping to the end days later, with nothing on screen tying
   the two events together, is the defect this prevents.

   The rule: a column that was not drawn keeps its slot, and the drawn names
   fill the slots that are left, in their new order. `all` is the full ordered
   board; anything in `drawn` that is not in it is appended rather than dropped,
   since a name the board just handed over is a fact and this function is not
   the place to argue with it. */
export function mergeOrder(drawn, all) {
  const names = columnNames(drawn)
  const moving = new Set(names)
  const queue = names.filter((name) => all.includes(name))
  const leftover = names.filter((name) => !all.includes(name))

  let next = 0
  /* The `?? name` guards a branch nothing can reach today — `all` comes from
     `boardColumns`, which buckets through a `Map` keyed on status and so cannot
     repeat a name — and it is written anyway because the failure would not stay
     here. A duplicate in `all` exhausts the queue, the `undefined` lands in
     `project.columnOrder`, serialises as `null`, and Rust's `Vec<String>` then
     refuses the whole project section of `settings.json`: side tab, open tabs,
     expanded folders and selection, gone for a column drawn twice. */
  const merged = all.map((name) => (moving.has(name) ? (queue[next++] ?? name) : name))
  return [...merged, ...leftover]
}

/* What the settings tab's checkbox lists are made of. Two groups, because the
   stored lists are global while a board is one project's: a name saved from
   another project's board still filters this one, so it has to be visible and
   removable rather than quietly at work. The first group is this project's
   columns, in the board's own order; the second is whatever is stored and not
   among them. */
export function columnChoices(stored, board) {
  const columns = Array.isArray(board) ? board : []
  const chosen = new Set(columnNames(stored))
  return {
    onBoard: columns.map((name) => ({ name, checked: chosen.has(name) })),
    elsewhere: columnNames(stored)
      .filter((name) => !columns.includes(name))
      .map((name) => ({ name, checked: true }))
  }
}

/* One name added to or taken out of a stored list. Returns a new array, and
   never grows a duplicate — a name arriving twice from a board that lists it
   twice would otherwise need two presses to clear. */
export function toggleColumn(stored, name, on) {
  const names = columnNames(stored)
  if (!on) return names.filter((entry) => entry !== name)
  return names.includes(name) ? names : [...names, name]
}

/* A status name as a person reads it. The board draws these uppercase in mono;
   here they sit beside sentence-case prose, so the dashes come out and nothing
   else does — a status is bd's word, and inventing a prettier one for it would
   be this file claiming to know a vocabulary it deliberately does not. */
export const columnLabel = (name) => String(name ?? '').replace(/-/g, ' ')
