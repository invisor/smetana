/* The two replies the auto-mode block card sends, as a pure module so the one
   thing a `.vue` file cannot be tested for, the wording, is tested anyway.

   Claude Code's documentation says an approval given in conversation has to
   name the action and the specific thing that makes it dangerous; a bare "you
   can force-push" clears nothing, and an approval covers the one action named.
   So the allow text names the tool and its detail and quotes the classifier's
   own reason back. `detail` is the journal's `ToolUse.detail`, clipped to 140
   characters on the Rust side, which is accepted. */

export function approvalText(tool, detail, reason) {
  return `I allow this action: ${tool} ${detail}. Do exactly that and nothing beyond it.\n\nThe block said: ${reason}`
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
