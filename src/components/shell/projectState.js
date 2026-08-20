/* How a project's state is said in words. Two call sites — the rail's
   tooltip, which is the one place `live` and `loud` are told apart by something
   other than hue, and the panel header's summary line — so the words are here
   rather than in either of them: two copies of a sentence are two sentences
   within the month.

   Pure, no Vue, for the reason the `projectMenu.js` family is: nothing in this
   repository can test a `.vue`.

   The counts come from `projectStates` in stores/terminals.js. `undefined` is
   ordinary and means nobody has any sessions there that the rail counts — no
   agents, a person's own shells not counting — which is what a window that has
   just opened sees for every project.

   **There is no noun in these sentences, and its absence is deliberate — do not
   restore it as a typo.** The premise it was written from has gone:
   `SessionMark` carried no work kind, so `projectStates` counted a plain shell
   exactly like an agent, and "1 agent running" was a claim the map could not
   support. The mark carries a kind now (smetana-low) and the map drops shells
   by it, so the sentences *could* name agents. The words are unchanged all the
   same, because putting the noun back is a decision about copy that nobody has
   taken — and because the map still says nothing about which agent or how many
   of which kind. Restore it deliberately or not at all; a test here pins the
   absence either way.

   Two of the three strings are the handoff's verbatim — `main · 1 waiting on
   you` and `idle` — and the third deliberately is not: the handoff writes the
   live case as `1 agent live`, which carries exactly the noun the paragraph
   above leaves out, so it is `1 running` here instead. A fidelity sweep against
   the handoff will meet that one difference; it is settled, not missed. */

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
