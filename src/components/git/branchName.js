/* Whether a name can be a branch, and the sentence to put under the field when
   it cannot. Two dialogs ask: the one that cuts a branch and the one that
   renames one. The shape rules are one list for both — git's
   `check-ref-format` does not care which command follows — and what the two do
   not share is which names count as taken, which is the pair at the foot of
   this file.

   The `branchMenu.js` / `gitActions.js` / `commitBox.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — no test in
   this repository can reach a `.vue`, so a rule left inside the dialog is a
   rule nothing checks.

   **This is not a second implementation of git's answer.** git decides, always:
   `vcs_create_branch` runs the real command and git's refusal comes back in its
   own words like every other refusal in this panel. What this exists for is the
   moment before that — a `Create` button that goes dead the character a name
   stops being possible, with one line saying which character did it, instead of
   a dialog that closes onto a red block in the panel behind it. So it is
   deliberately allowed to be *narrower* than git: a name this passes and git
   refuses is an ordinary outcome that the panel already draws. A name this
   refuses and git would have taken is the thing to avoid, which is why the
   checks below are git's own documented ones and nothing invented. */

/* `git help check-ref-format`, restricted to the parts that apply to a branch
   name somebody types. Each is a test rather than a regex over the lot, because
   what a person needs is which rule they broke, not that they broke one. */
const CHECKS = [
  [/\s/, 'A branch name cannot contain spaces.'],
  [/^-/, 'A branch name cannot start with a dash.'],
  [/^\/|\/$/, 'A branch name cannot start or end with a slash.'],
  [/\/\//, 'A branch name cannot hold two slashes in a row.'],
  [/\.\./, 'A branch name cannot contain two dots in a row.'],
  [/@\{/, 'A branch name cannot contain @{.'],
  [/[~^:?*[\\]/, 'A branch name cannot contain ~ ^ : ? * [ or a backslash.'],
  /* Both of git's dot rules at the end, and the one about a path component
     starting with one: `.hidden` and `feature/.wip` are refused the same way. */
  [/(^|\/)\./, 'No part of a branch name can start with a dot.'],
  [/\.$/, 'A branch name cannot end with a dot.'],
  [/\.lock(\/|$)/, 'A branch name cannot end with .lock.'],
  // eslint-disable-next-line no-control-regex
  [/[\x00-\x1f\x7f]/, 'A branch name cannot contain control characters.']
]

/* What is wrong with this name, or null. An empty field is **not** an error:
   nothing has been typed yet, and a dialog that opens shouting at a person who
   has done nothing is a dialog that taught them to ignore the line. `canCreate`
   is what holds the button until there is something there. */
export function branchNameError(name, branches = []) {
  const wanted = (name ?? '').trim()
  if (!wanted) return null
  if (wanted === '@') return 'A branch cannot be called @ on its own.'
  for (const [pattern, sentence] of CHECKS) {
    if (pattern.test(wanted)) return sentence
  }
  /* The one check that is not about the shape of the name. Compared exactly:
     git's own comparison is exact, and a case-insensitive one here would refuse
     a name git would take on the filesystems where it matters — which is the
     direction this file is not allowed to be wrong in. */
  if (branches.some((branch) => branch.name === wanted)) {
    return 'A branch with this name already exists.'
  }
  return null
}

/* Whether the dialog's own button may be pressed. The two halves of the panel's
   verdict come in as they do everywhere else — `gitActions.js`'s `allowed`, and
   whether git is already working — because the dialog can be open across a run
   starting underneath it. */
export function canCreate({ name, branches = [], allowed = true, busy = false }) {
  const wanted = (name ?? '').trim()
  if (!wanted || !allowed || busy) return false
  return branchNameError(wanted, branches) === null
}

/* What is wrong with a name a branch is being **renamed** to, or null.

   Every shape rule above holds unchanged — git's `check-ref-format` does not
   care which command is about to run — and the one check that is not about the
   shape has to lose one branch: the one being renamed. It is in the list, so
   `branchNameError` alone would open the field on "A branch with this name
   already exists." about the very branch the sentence is under, which is a
   dialog calling a name taken by the person it is asking.

   The unchanged name is deliberately **not** an error either. Nothing is wrong
   with it — it is the name the branch already has — so there is nothing to put
   under the field; what it does is hold the button, which is `canRename`'s job
   and exactly what an empty field does at `canCreate`. A red line about a name
   somebody has not yet touched is the thing this file's header refuses to do. */
export function renameError(name, from, branches = []) {
  return branchNameError(
    name,
    (branches ?? []).filter((branch) => branch.name !== from)
  )
}

/* Whether the rename dialog's own button may be pressed. `canCreate`'s two
   halves of the panel's verdict, for its reason — the window can be open across
   a run starting underneath it — plus the one thing that is this dialog's own:
   a name equal to the one the branch already carries is nothing to ask git for.
   Compared after trimming and exactly, since that is the name that would be
   written. */
export function canRename({ name, from, branches = [], allowed = true, busy = false }) {
  const wanted = (name ?? '').trim()
  if (!wanted || !from || wanted === from || !allowed || busy) return false
  return renameError(wanted, from, branches) === null
}
