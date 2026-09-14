/* What a start will call its work once it is a session, worked out from the
   very intent that is being sent. The `kind` tags on this side and on
   `SessionWork`'s are the same words by construction — `Intent::work` in
   `src-tauri/src/agents/mod.rs` maps one onto the other — so the placeholder
   row and the session's own row say the same thing, and the handover a
   moment later changes nothing on screen.

   Pure, no Vue and no Tauri in it, which is what lets it live outside either
   store. `stores/terminals.js` held the full translation and
   `stores/conversation.js` held a two-entry copy of it — one for every intent
   the PTY road could reach, one for the two that used to be the whole of what
   the driven road accepted. Now every intent a person talks to can be
   driven (`.claude/rules/terminal.md`), so the two lists are one list, and
   both stores import it rather than each keeping their own.

   What an intent carries and this does not is the agent's briefing rather than
   anything drawn: the paths of the images attached to a task, the
   brainstorming switch, a run's settings, a conflict's file list. */

/**
 * @param {object | undefined} intent
 * @returns {object} the `work` a session's row is captioned by
 */
export function workOf(intent) {
  // No intent at all is the bare case: "+ New agent" hands over
  // `{ kind: 'bare' }` explicitly, but a caller with nothing to say — the same
  // shape `startConversation`'s own default keeps — reads the same way.
  if (!intent) return { kind: 'bare' }
  if (intent.kind === 'editTask') return { kind: 'editTask', id: intent.id }
  if (intent.kind === 'fixTask') return { kind: 'fixTask', id: intent.id }
  if (intent.kind === 'resolveTask') return { kind: 'resolveTask', id: intent.id }
  if (intent.kind === 'resolveConflict') {
    return { kind: 'resolveConflict', repo: intent.repo, theirs: intent.theirs }
  }
  /* The title alone, exactly as `Intent::work` sends it on: the id and the
     directory are the agent's briefing, and the row draws the conversation
     somebody recognises. */
  if (intent.kind === 'resumeSession') {
    return { kind: 'resumeSession', title: intent.title ?? null }
  }
  /* The report's path alone, exactly as `Intent::work` sends it on: the pairs
     are the agent's briefing, and this is what the tab opened afterwards is
     found by. */
  if (intent.kind === 'reviewBranch') {
    return { kind: 'reviewBranch', report: intent.report }
  }
  if (intent.kind === 'newTask') {
    return {
      kind: 'newTask',
      text: intent.draft.text,
      issueType: intent.draft.issue_type ?? null,
      priority: intent.draft.priority ?? null,
      /* Spelled the same on both sides of the wire, unlike `issue_type`: the
         draft panel draws it as a row of its own, and a placeholder without it
         would drop that row for the second the start lasts and then grow it
         back when the session lands. */
      parent: intent.draft.parent ?? null
    }
  }
  return { kind: intent.kind }
}
