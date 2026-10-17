/* The intent the New task screen starts an agent with.

   It is the desktop's own: the "New task" window sends `{ kind: 'newTask', brainstorm, spec,
   plan, draft }` to `startAgent` (views/DesktopApp.vue, `submitNewTask`), and
   `Intent::NewTask` in src-tauri/src/agents/mod.rs is what reads it. The phone offers the
   window's fields: what the person wants, pictures, a type, a priority and the three stages.
   Type and priority on Auto arrive as absence, and the stages on Auto are the agent's own
   judgement about whether the task wants a discussion, a design and a plan. There is no parent.

   Because the agent is what files the task, the session that starts is a conversation, and
   the person lands in it: the agent may well ask something before it runs `bd create`. */
import { DEFAULT_STAGE, cascade } from '@src/components/kanban/taskStages.js'

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
export const canAttach = (status) =>
  (status?.state === 'online' || status?.state === 'update-recommended') &&
  Number(status.protocol_minor) >= ATTACHMENTS_MINOR

const chosen = (value) => (value === undefined || value === null || value === 'auto' || value === '' ? null : value)

/* The stages sent are the ones the screen shows: a stage under an Off or an Auto parent
   travels as its parent, never as the choice remembered under a different one. */
export function newTaskIntent({
  text,
  issueType,
  priority,
  brainstorm = STAGE_DEFAULT,
  spec = STAGE_DEFAULT,
  plan = STAGE_DEFAULT,
  images = []
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
      images,
      parent: null
    }
  }
}
