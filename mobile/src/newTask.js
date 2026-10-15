/* The intent the New task screen starts an agent with.

   It is the desktop's own: the "New task" window sends `{ kind: 'newTask', brainstorm, spec,
   plan, draft }` to `startAgent` (views/DesktopApp.vue, `submitNewTask`), and
   `Intent::NewTask` in src-tauri/src/agents/mod.rs is what reads it. The phone offers the
   window's one field - what the person wants - and leaves the other choices where the window
   opens them: the three stages on Auto, which is the agent's own judgement about whether the
   task wants a discussion, a design and a plan, and the type and the priority on Auto too,
   which arrive as absence. There are no images (a path on the desktop is not something a
   phone can name) and no parent.

   Because the agent is what files the task, the session that starts is a conversation, and
   the person lands in it: the agent may well ask something before it runs `bd create`. */

export const STAGE_DEFAULT = 'auto'

/* Whether there is anything to file. Blank text is not a task. */
export const canFile = (text) => typeof text === 'string' && text.trim().length > 0

export function newTaskIntent(text) {
  return {
    kind: 'newTask',
    brainstorm: STAGE_DEFAULT,
    spec: STAGE_DEFAULT,
    plan: STAGE_DEFAULT,
    draft: {
      text: String(text ?? '').trim(),
      issue_type: null,
      priority: null,
      images: [],
      parent: null
    }
  }
}
