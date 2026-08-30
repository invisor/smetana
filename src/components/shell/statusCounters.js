/* What the two numbers in the status footer are called when somebody points at
   them, and nothing about how they look.

   Another of the `headline.js` family living beside it: the whole of one rule,
   pure, with no Vue and no DOM in it, under the directory of the part of the
   interface it is a rule about. These two spent their life as computeds inside
   `ScopeIndicator.vue`, where no test in this repository could reach them, and
   both were wrong there for exactly that reason — each was glued together with
   a plural noun and said "1 uncommitted files" for the commonest case there is,
   which nobody saw while the numbers were a fixture holding 3 and 2. The bell
   beside them had the same fault and was fixed for the same reason. Moving them
   from one `.vue` to another would have carried the blind spot along with the
   words, so they came out here on the way.

   Neither label is ever drawn over a count of zero: the counter itself is
   hidden then, and the strip closes up around it. The zero case is written down
   in the test all the same — it is what the functions answer, and a caller that
   ever does draw it should get a sentence rather than a broken one. */

/**
 * The hint over the uncommitted-files counter.
 *
 * @param {number} count how many files the selected repository has uncommitted
 * @returns {string} `1 uncommitted file`, or the plural
 */
export function dirtyLabel(count) {
  return count === 1 ? '1 uncommitted file' : `${count} uncommitted files`
}

/**
 * The hint over the agents counter.
 *
 * @param {number} count how many of this project's agents are alive
 * @returns {string} `1 agent running`, or the plural
 */
export function agentsLabel(count) {
  return count === 1 ? '1 agent running' : `${count} agents running`
}
