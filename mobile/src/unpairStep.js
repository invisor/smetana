/* The two taps it takes to unpair: a pure rule, with no Vue in it. Forgetting the desktop is the
   one thing on the phone that cannot be taken back by the phone alone, so the first press only
   asks, and only a press that follows the question confirms.

   A step is `idle` or `asked`; a press is `unpair`, `confirm` or `cancel`. A press that makes
   no sense in the step it finds (a `confirm` while nothing was asked) changes nothing. */
export const IDLE = 'idle'
export const ASKED = 'asked'

export function nextUnpairStep(step, press) {
  if (step === IDLE && press === 'unpair') return ASKED
  if (step === ASKED && (press === 'cancel' || press === 'confirm')) return IDLE
  return step
}

/* Whether this press, made in this step, is the one that unpairs. */
export const confirms = (step, press) => step === ASKED && press === 'confirm'
