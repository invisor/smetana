/* How a project's state is said in words. Two call sites — the rail's
   tooltip, which is the one place `live` and `loud` are told apart by something
   other than hue, and the panel header's summary line — so the words are here
   rather than in either of them: two copies of a sentence are two sentences
   within the month.

   Pure, no Vue, for the reason the `projectMenu.js` family is: nothing in this
   repository can test a `.vue`.

   The counts come from `projectStates` in stores/terminals.js. `undefined` is
   ordinary and means nobody has any sessions there, which is what a window that
   has just opened sees for every project.

   **There is no noun in these sentences, and its absence is deliberate — do not
   restore it as a typo.** `projectStates` is built from `SessionMark`, which
   carries an id, a project and a state and no work kind at all, so a plain
   shell is counted exactly like an agent and a shell that rings the bell
   reaches `needs-you` the same way one does. "1 agent running" would therefore
   be a claim this map cannot support, and it is a claim the app answers
   elsewhere and differently: `liveAgentCount` filters through
   `isShellSession`, so the scope bar could read 0 while a header under it read
   1, on one screen, about one project. The dot beside these words claims only
   that something here wants you, which is true of a shell too; the words are
   held to the same. Counting agents apart from shells needs a field on the mark,
   and that is a change to Rust and to the store rather than a rewording here.

   Two of the three strings are the handoff's verbatim — `main · 1 waiting on
   you` and `idle` — and the third deliberately is not: the handoff writes the
   live case as `1 agent live`, which carries exactly the noun this map cannot
   support, so it is `1 running` here instead. A fidelity sweep against the
   handoff will meet that one difference; it is settled, not missed. */

/** "1 waiting on you" | "2 running" | "idle". */
export function stateLabel(row) {
  /* Waiting is counted before running for the reason `projectStates` lets
     `loud` win over `live`: a project with something waiting on a person is the
     reason the rail exists, and another session getting on with its work there
     must not be what the tile says instead. */
  if (row?.loud) return `${row.loud} waiting on you`
  if (row?.live) return `${row.live} running`
  return 'idle'
}

/** The panel header's one line: "develop · 1 running". */
export function projectSummary(branch, row) {
  /* An empty branch is dropped rather than joined: a line opening with a
     separator reads as a missing word, and a project whose head has not been
     read yet is the ordinary case for the first moment of a window. */
  return [branch, stateLabel(row)].filter(Boolean).join(' · ')
}
