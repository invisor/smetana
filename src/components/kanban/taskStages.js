/* The cascade under the new-task dialog's Brainstorming switch: what the Spec
   and Plan dropdowns show, whether a person may touch them, and what is
   therefore sent.

   The three stages depend on each other in one direction. A design document is
   what the discussion produces, so there is nothing to write when no discussion
   happened; a plan plans what the design settled, so there is nothing to plan
   when no design was written. Spec is a person's to choose only while
   Brainstorming is On, and Plan only while Spec is On.

   A stage nobody may touch reads as its parent rather than as a placeholder:
   under an Off it shows Off, under an Auto it shows Auto. That is what makes
   the screen state exactly what will be sent — a greyed control reading "Auto"
   under a settled Off would claim the agent still had a judgement to make.

   Pure and outside the component for the reason `columnOrder.js` and
   `branchChoice.js` are: a `.vue` file is the one thing no test in this
   repository can reach, and this is a rule that can be wrong in exactly one of
   nine combinations. `prompt.rs` normalises the same cascade again on the far
   side of the wire — the two agree by vocabulary rather than by code, the same
   way `runScopes.js` and the run worker's `admit` do. */

/* The one vocabulary all three switches share. Auto is not a null position: it
   is the agent's judgement to make, stated as such, since nothing in this app
   has read the text of the task. */
export const STAGES = [
  { value: 'auto', label: 'Auto' },
  { value: 'on', label: 'On' },
  { value: 'off', label: 'Off' }
]

/* What a stage opens on, and what a child returns to when its parent moves. */
export const DEFAULT_STAGE = 'auto'

/* One link of the chain: what a stage shows given its parent's position and
   what the person last chose for it. Only under an On is the choice theirs. */
export function stageUnder(parent, chosen) {
  return parent === 'on'
    ? { value: chosen, interactive: true }
    : { value: parent, interactive: false }
}

/* Both links at once. The Plan hangs off what Spec *shows*, not off what was
   chosen for it, which is what stops a stale `spec: 'on'` under an Off
   Brainstorming from reopening the Plan underneath it. */
export function cascade(brainstorm, spec, plan) {
  const specStage = stageUnder(brainstorm, spec)
  return { spec: specStage, plan: stageUnder(specStage.value, plan) }
}
