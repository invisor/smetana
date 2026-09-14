/* What a session row is captioned by — the prose half and the identifiers
   beside it — worked out from the `work` a row carries (`sessionWork.js`).

   Pure, no Vue and no Tauri in it, for the reason `sessionWork.js` is: two
   stores and `drivenRows.js` all want this rule at once, and a store's copy
   cannot be imported into a pure module without dragging Tauri in behind it.
   `stores/terminals.js` held the full table and `captionOf`, and
   `drivenRows.js` held a two-entry copy of the table for the two intents that
   used to be the whole of what the driven road accepted — a copy that file's
   own header said the seam cost. Every intent a person talks to can be driven
   now (`.claude/rules/terminal.md`), so there is no seam left to pay for, and
   both callers read this one table instead.

   `basename` comes from `src/paths.js` rather than from either store, for the
   reason that file sits at the top of `src/`: it is wanted by more than one
   part of the interface and has no Vue or Tauri in it either. */
import { basename } from '../../paths.js'

/* The prose half of a row's caption. Sentence case, and every one of them is
   what the session is *for* — the process behind it is `claude-7`, and that
   name is deliberately not on a row any more: five of them said nothing about
   who was doing what. */
export const CAPTION = {
  bare: 'Agent',
  newTask: 'Creating a task',
  editTask: 'Editing',
  /* Shorter than the menu row it is started from ("Answer questions"), and
     deliberately: the id sits beside it in mono, so the row already reads
     "Answering smetana-8av" and the word "questions" would only push the id
     toward the ellipsis. */
  resolveTask: 'Answering',
  /* The third caption about one named issue, and the one that is not about the
     issue's own text: an edit changes what the task says, this changes what
     was built for it. The id sits beside the word in mono, so the row reads
     "Fixing smetana-8av". */
  fixTask: 'Fixing',
  /* The one caption about a repository rather than an issue. "Conflict" and
     not "Resolving a conflict": the identifiers beside it are what say which
     one, and a row 252px wide spends every character it has on them. */
  resolveConflict: 'Conflict',
  /* The other caption about no issue at all — there is no id, no path and no
     branch beside it, because the whole of what this session was given is a
     briefing about a database. So the words carry it alone, and they name the
     tracker rather than reading as a bare "Agent": a row that said only that
     would be indistinguishable from the "+ New agent" beside it, on the one
     screen where a person has just pressed a button and wants to see that
     something is happening about it. */
  repairTracker: 'Repairing the tracker',
  /* A conversation that existed before this window did, picked up again from
     its transcript. It says what it is first and names the session second (see
     `captionOf`), because the one thing this row must not do is read as work
     taken off the board: there is no issue behind it, nothing claimed it, and a
     row that merely showed a sentence would be indistinguishable from a filing
     agent's draft. */
  resumeSession: 'Resumed session',
  setup: 'Project setup',
  /* The founding session: named for the folder it starts in rather than for
     the file it ends by writing, since the person watching it is answering
     questions about a project that does not exist yet. */
  bootstrap: 'Starting a project'
}

/* A row's caption, in two pieces because they are set differently: `label` is
   prose and belongs in sans, `tasks` are identifiers and belong in mono. The
   component is what knows that; this only says which is which.

   `claimed` defaults to the empty array: only a run reads it at all, and every
   other caller — a driven row, which never runs a batch — has none to hand
   over.

   A run with nothing claimed yet reads as a bare agent does, and that is the
   truth rather than a fallback — it is an agent, and there is no work to name
   until it takes some. Work this front end has never heard of lands there too:
   a row that says "Agent" is still a row. */
export function captionOf(work, claimed = []) {
  const kind = work?.kind
  // The three that are about one named issue, and so caption themselves with
  // it. What they are doing to it differs; that is the label's business.
  if (kind === 'editTask' || kind === 'resolveTask' || kind === 'fixTask') {
    return { label: CAPTION[kind], tasks: [work.id] }
  }
  /* The two identifiers this one is about, in mono beside the word: which
     repository — its folder's name, since the absolute path is most of a row
     on its own and the panel already says which project this is — and the
     branch that was being brought in. */
  if (kind === 'resolveConflict') {
    return { label: CAPTION[kind], tasks: [basename(work.repo ?? ''), work.theirs].filter(Boolean) }
  }
  /* The one caption that carries prose beside its own words rather than
     identifiers, which is why the session's title goes in the *label*: `tasks`
     is set in mono, and a person's own sentence in a monospaced face would read
     as an id. The id this row does not draw is deliberate — a 36-character UUID
     tells nobody which conversation this is, and the card in the Sessions tab
     has it in full. A resumed session with no title says what it is and stops
     there. */
  if (kind === 'resumeSession') {
    const title = work.title ? String(work.title) : ''
    return { label: title ? `${CAPTION.resumeSession}: ${title}` : CAPTION.resumeSession, tasks: [] }
  }
  if (kind === 'run' && claimed.length) return { label: null, tasks: claimed }
  return { label: CAPTION[kind] ?? CAPTION.bare, tasks: [] }
}
