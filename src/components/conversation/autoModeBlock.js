/* The two replies the auto-mode block card sends, as a pure module so the one
   thing a `.vue` file cannot be tested for, the wording, is tested anyway.

   Claude Code's documentation says an approval given in conversation has to
   name the action and the specific thing that makes it dangerous, and every
   soft-deny rule carries a "must name" clause saying which specific that is;
   a bare "you can force-push" clears nothing, and an approval covers the one
   action named. So the allow text states that the person approves one action,
   quotes the classifier's category, and quotes the **whole** command. The
   journal's `ToolUse.detail` is clipped to 140 characters, which is enough for
   a row and not for this: the Rust side puts the full input (clipped at 2 000)
   on the refused call's result as `blockedInput`, and `detail` is only the
   fallback for a journal from before that field existed. */

/* `denial` is the classifier's own text (`result.denial`); what the sentence
   quotes is the bracketed category inside it. */
export function approvalText(tool, detail, denial, blockedInput = null) {
  const action = blockedInput || detail
  return (
    `I explicitly approve this one action, which the auto mode classifier refused as [${denialReason(denial)}]:\n\n` +
    `${tool}: ${action}\n\n` +
    'I have read the reason and I approve exactly this command, against exactly the target it names, once. ' +
    'Do exactly that and nothing beyond it.'
  )
}

export function refusalText(tool, detail) {
  return `Do not do this: ${tool} ${detail}. Find a way without it, or stop and tell me what you need.`
}

/* The part of a denial a person is shown as its reason: the bracketed category
   when there is one ("Modify Shared Resources"), else the text itself. */
export function denialReason(denial) {
  const bracket = /Reason: \[([^\]]+)\]/.exec(denial ?? '')
  return bracket ? bracket[1] : (denial ?? '')
}

/* Whether a block card offers Allow and Refuse. `composerShown` is the panel's
   own "may a turn be sent right now" (a session the person may write into, no
   card waiting on an answer), so the card never offers a turn the composer
   would refuse. `replying` is the panel's in-flight flag: from the first press
   until the journal closes the block, so two quick presses, or Allow then
   Refuse, cannot send two turns. */
export function blockButtonsShown({ blockOpen, composerShown, replying }) {
  return !!blockOpen && !!composerShown && !replying
}
