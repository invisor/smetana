/* Whether an agent finished a turn between two looks at the agents panel. The
   interface's own vocabulary has exactly one end of a turn: a row's state left
   `running` for anything else (`ready`, `needs-you`, `done`, `failed`). Both
   roads — driven sessions through `statusOf`, PTY ones through `toUiState` —
   already speak it by the time they meet in the panel's rows, so the rule is
   asked there and not in either store.

   Pure, with no Vue and no DOM in it, which is what makes it reachable by a
   test at all: the watch that asks it lives in a `.vue` file.

   Both arguments are maps from a row's key (`agentKey`) to its `state`. A key
   present on one side only does not count: a new row is a start and a vanished
   one is a close, and neither is a turn ending. */

export function turnEnded(before, after) {
  if (!before || !after) return false
  for (const [key, state] of before) {
    if (state !== 'running') continue
    if (!after.has(key)) continue
    if (after.get(key) !== 'running') return true
  }
  return false
}
