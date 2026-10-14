/* The notes a person leaves for the agent of one run, as a rule rather than as
   a form: pure, no Vue and no DOM, so a test can reach what the dialog sends.

   Optional and never remembered. Empty or whitespace-only means absent, and
   absent travels as `null` — the same shape as the floor and the pause, and
   what `RunSettings::instructions` reads as `None`. */

/* The same bound as `INSTRUCTIONS_MAX` in src-tauri/src/runs/model.rs, which is
   what refuses a longer text that arrives anyway. */
export const NOTES_MAX = 2000

export function notesPayload(text) {
  const trimmed = (text ?? '').trim()
  return trimmed === '' ? null : trimmed
}
