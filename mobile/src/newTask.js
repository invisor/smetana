/* The intent the New task screen starts an agent with.

   It is the desktop's own: the "New task" window sends `{ kind: 'newTask', brainstorm, spec,
   plan, draft }` to `startAgent` (views/DesktopApp.vue, `submitNewTask`), and
   `Intent::NewTask` in crates/core/src/agents/mod.rs is what reads it. The phone offers the
   window's fields: what the person wants, pictures, a type, a priority and the three stages.
   Type and priority on Auto arrive as absence, and the stages on Auto are the agent's own
   judgement about whether the task wants a discussion, a design and a plan. There is no parent.

   Because the agent is what files the task, the session that starts is a conversation, and
   the person lands in it: the agent may well ask something before it runs `bd create`. */
import { DEFAULT_STAGE, cascade } from '@src/components/kanban/taskStages.js'
import { desktopMinor } from './bridge/states.js'

export const STAGE_DEFAULT = DEFAULT_STAGE

/* The first protocol minor whose desktop answers `attachment_write` to a phone. */
export const ATTACHMENTS_MINOR = 3

/* Copied from `components/kanban/NewTaskModal.vue`, which keeps them inside the `.vue`
   where nothing can import them. Auto travels as null, so a value is only ever one bd knows. */
const AUTO = { value: 'auto', label: 'Auto' }
export const TYPES = [AUTO, 'task', 'bug', 'feature', 'chore', 'epic', 'decision']
export const PRIORITIES = [
  AUTO,
  { value: '0', label: 'P0 · highest' },
  { value: '1', label: 'P1' },
  { value: '2', label: 'P2' },
  { value: '3', label: 'P3' },
  { value: '4', label: 'P4 · lowest' }
]

/* Whether there is anything to file. Blank text is not a task. */
export const canFile = (text) => typeof text === 'string' && text.trim().length > 0

/* Whether the desktop on the other end can store a picture the phone sends. */
export const canAttach = (status) => desktopMinor(status) >= ATTACHMENTS_MINOR

/* Whether Start may be pressed: there are words, nothing is being started, and no picture is
   still on its way to the desktop. A task filed while one uploads would go without it, and the
   picture would then turn up in the next, freshly cleared form. */
export const canStart = ({ text, busy = false, attaching = false }) => !busy && !attaching && canFile(text)

const chosen = (value) => (value === undefined || value === null || value === 'auto' || value === '' ? null : value)

/* What was attached, split the way the desktop's New task window splits it: a record with a
   `url` has a thumbnail and goes under `images`, one without is another file and goes under
   `files`. The phone's own picker is `image/*`, so a file arrives here only when the system
   hands over something it calls an image that Rust does not recognise (HEIC and the like). */
const split = (attachments) => ({
  images: attachments.filter((item) => item.url).map((item) => item.path),
  files: attachments.filter((item) => !item.url).map((item) => item.path)
})

/* The stages sent are the ones the screen shows: a stage under an Off or an Auto parent
   travels as its parent, never as the choice remembered under a different one. */
export function newTaskIntent({
  text,
  issueType,
  priority,
  brainstorm = STAGE_DEFAULT,
  spec = STAGE_DEFAULT,
  plan = STAGE_DEFAULT,
  attachments = []
}) {
  const stages = cascade(brainstorm, spec, plan)
  const level = chosen(priority)
  return {
    kind: 'newTask',
    brainstorm,
    spec: stages.spec.value,
    plan: stages.plan.value,
    draft: {
      text: String(text ?? '').trim(),
      issue_type: chosen(issueType),
      priority: level === null ? null : Number(level),
      ...split(attachments),
      parent: null
    }
  }
}
