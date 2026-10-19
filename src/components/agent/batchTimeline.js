/* The rows of an Autopilot batch's timeline, folded out of the structured log.

   The log (`LogEntry` on the Rust side) is flat and in order: a remark, a tool
   call, the call's result some entries later, a system note. A timeline wants
   one row per thing a person reads, so a result is folded into the call it
   belongs to by `id` and draws nothing of its own; a result whose call the
   worker's budget has already dropped folds into nothing and is ignored.

   `live` is the one place this module has an opinion about time: a call with no
   result is open only while the session is, and a session that exited with a
   call still open leaves it as `result: null, live: false`, which the view says
   as "no result" rather than pulsing for ever. Pure — no Vue, no DOM — so each
   rule is tested. */

export const TOOL_LABEL = {
  Task: 'Agent',
  ScheduleWakeup: 'Wakeup',
  AskUserQuestion: 'Ask',
  NotebookEdit: 'Notebook'
}

export const toolLabel = (name) => TOOL_LABEL[name] ?? name

/* The handoff borrows status hues for kinds of tool. They sit in a mono column
   at the small size and never in a badge, so they cannot be read as a status. */
const TONES = {
  Read: 'var(--status-ready-fg)',
  Grep: 'var(--status-ready-fg)',
  Glob: 'var(--status-ready-fg)',
  Task: 'var(--status-blocked-fg)',
  ScheduleWakeup: 'var(--status-blocked-fg)',
  Edit: 'var(--status-done-fg)',
  Write: 'var(--status-done-fg)',
  MultiEdit: 'var(--status-done-fg)',
  NotebookEdit: 'var(--status-done-fg)'
}

export const toolTone = (name) => TONES[name] ?? 'var(--text-secondary)'

/* Tools whose command is a path or a pattern, so a path under the project is
   shown relative to it in the collapsed row. */
export const FILE_TOOLS = ['Read', 'Edit', 'Write', 'NotebookEdit', 'Glob', 'Grep']

export function displayCommand(entry, project) {
  const root = (project ?? '').replace(/\/+$/, '')
  if (root !== '' && FILE_TOOLS.includes(entry.tool) && entry.command.startsWith(`${root}/`)) {
    return entry.command.slice(root.length + 1)
  }
  return entry.command
}

/* The first word is brighter than the rest in the row. */
export function splitHead(text) {
  const at = text.search(/\s/)
  if (at < 0) return { head: text, rest: '' }
  return { head: text.slice(0, at), rest: text.slice(at).trimStart() }
}

const two = (n) => String(n).padStart(2, '0')

export function timeOf(at) {
  const date = new Date(at)
  if (Number.isNaN(date.getTime())) return ''
  return `${two(date.getHours())}:${two(date.getMinutes())}:${two(date.getSeconds())}`
}

export function dayOf(at) {
  const date = new Date(at)
  if (Number.isNaN(date.getTime())) return ''
  return `${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())}`
}

export function timelineRows(entries, { project = '', exited = false } = {}) {
  const results = new Map()
  for (const entry of entries) {
    if (entry.kind === 'result') results.set(entry.id, { ok: entry.ok, summary: entry.summary })
  }

  const rows = []
  let day = null
  for (const entry of entries) {
    let row
    if (entry.kind === 'say' || entry.kind === 'note') {
      row = { key: entry.seq, kind: entry.kind, time: timeOf(entry.at), text: entry.text }
    } else if (entry.kind === 'tool') {
      const result = results.get(entry.id) ?? null
      const { head, rest } = splitHead(displayCommand(entry, project))
      row = {
        key: entry.seq,
        kind: 'tool',
        time: timeOf(entry.at),
        tool: entry.tool,
        toolLabel: toolLabel(entry.tool),
        tone: toolTone(entry.tool),
        head,
        rest,
        command: entry.command,
        cwd: entry.cwd ?? null,
        live: result === null && !exited,
        result
      }
    } else {
      continue
    }
    const date = dayOf(entry.at)
    if (day !== null && date !== day) rows.push({ key: `day-${entry.seq}`, kind: 'day', date })
    day = date
    rows.push(row)
  }
  return rows
}
