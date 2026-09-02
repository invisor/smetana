/* What `[defaults]` in `.smetana/project.toml` may hold, as a rule rather than
   as a form, and what the window that edits it says when there is no file to
   edit.

   The `branchChoice.js` / `configFreshness.js` family: pure, no Vue and no DOM,
   which is the whole reason it is a file of its own — no test in this
   repository can reach a `.vue`, so a rule left inside the component is a rule
   nothing checks.

   It covers four keys and no more. The rest of that file — the repositories,
   the gate lists, the preflight, the merge hazards, the live check — stays the
   setup agent's, because it is discovered by looking at the folder rather than
   decided in a dialog. These four are the ones somebody turns while watching a
   board.

   Field names are the file's own, in snake_case, and that is deliberate rather
   than sloppy: `ProjectConfig` crosses IPC in the spelling the TOML uses, so a
   draft in this shape is already the payload, and the file, the console and
   this module all say `max_parallel_tasks`. See the note over `ProjectConfig`
   in `src-tauri/src/runs/config.rs`. */

/* The same four values `Defaults::default()` produces on the Rust side. Written
   out here rather than read off a loaded config, because the case this exists
   for is a file that carries none of them. Frozen so that a draft built from it
   has to be a copy: a form writing through into the fall-back would make every
   later dialog open on the last one's numbers. */
export const DEFAULTS_FALLBACK = Object.freeze({
  target_branch: null,
  min_priority: 2,
  max_parallel_tasks: 3,
  review_passes: 5
})

/* The label for "no branch chosen". Absence, not an empty string: the file's
   `target_branch` is an `Option<String>`, and what `None` buys is the run
   dialog falling back to the branch the project is on — a project that has not
   chosen a target must not have `main` chosen for it. */
export const NO_BRANCH = 'No default — use the current branch'

/* Narrower than the `u8` the file holds, and that is their purpose: the type
   stops 300, the bound stops the typo that spawns two hundred agents
   overnight. A ceiling on a mistake, not a statement about the machine.

   bd's priority scale is the one of the three that is closed rather than
   chosen: 0 to 4 is what the tracker has, so the field is a select and the
   bound is the scale itself.

   The consequence, which is asked for rather than overlooked: a file whose
   author deliberately wrote `max_parallel_tasks = 20` opens this form with a
   permanent "Between 1 and 16." and a dead Save, so no other field can be
   changed there without agreeing to be narrowed first. That is the bound doing
   its job — the point of it being tighter than the `u8` — and the file stays
   whatever it says until somebody presses Save. */
const RANGES = {
  min_priority: [0, 4],
  max_parallel_tasks: [1, 16],
  review_passes: [1, 10]
}

/* The four values off a loaded `ProjectConfig`, or off nothing at all. `null`
   is an ordinary input here: the dialog can be opened a frame before the config
   has landed, and a file with no `[defaults]` section is the common case. */
export function draftFrom(config) {
  const stored = config?.defaults ?? {}
  return {
    target_branch: stored.target_branch ?? DEFAULTS_FALLBACK.target_branch,
    min_priority: stored.min_priority ?? DEFAULTS_FALLBACK.min_priority,
    max_parallel_tasks: stored.max_parallel_tasks ?? DEFAULTS_FALLBACK.max_parallel_tasks,
    review_passes: stored.review_passes ?? DEFAULTS_FALLBACK.review_passes
  }
}

/* `branches` is `target_branches`' answer — `{name, missing_in}` apiece, the
   same list the run dialog's own branch field is filled from — and only the
   name is wanted here.

   A branch that is stored but no longer in the list is kept as an option of its
   own. Dropping it would make opening this dialog a way to silently change a
   value, which is the one thing a settings screen must never do. */
export function branchOptions(branches, chosen) {
  const names = (branches ?? []).map((branch) => branch?.name).filter(Boolean)
  const kept = chosen && !names.includes(chosen) ? [chosen] : []
  return [
    { value: '', label: NO_BRANCH },
    ...[...names, ...kept].map((name) => ({ value: name, label: name }))
  ]
}

/* `{}` when every field is in range, else the field's own sentence under the
   field. A whole number is part of the check rather than a separate one: a
   `u8` on the other side takes no fraction, and "2.5 review passes" is out of
   range in the only sense the file has. */
export function validateDraft(draft) {
  const errors = {}
  for (const [field, [low, high]] of Object.entries(RANGES)) {
    const value = draft?.[field]
    if (!Number.isInteger(value) || value < low || value > high) {
      errors[field] = `Between ${low} and ${high}.`
    }
  }
  /* Nothing about `target_branch`: every string is a possible branch name, and
     absence is a legitimate value rather than an empty field. */
  return errors
}

/* An empty branch and no branch are one state, so a draft that has only turned
   `''` into `null` is not a change worth enabling a button for. `Select` has no
   way to hand back `null`, which is where the `''` comes from. */
const sameBranch = (a, b) => (a || null) === (b || null)

export function isDirty(draft, original) {
  return (
    !sameBranch(draft?.target_branch, original?.target_branch) ||
    draft?.min_priority !== original?.min_priority ||
    draft?.max_parallel_tasks !== original?.max_parallel_tasks ||
    draft?.review_passes !== original?.review_passes
  )
}

/* The file the four keys above live in, named once. It is an identifier, so
   every sentence that mentions it hands it over separately rather than
   embedding it: the window draws it in mono, the way this app draws every path
   it puts in front of somebody. */
export const CONFIG_FILE = '.smetana/project.toml'

/* Why the form has no fields, for every state that is not a parsed file. Read
   by the state's own name — `missing`, `broken` and `ok`, which is what
   `project_config` answers and `stores/runs.js` keeps whole — so a fourth state
   invented in Rust turns up here as the last sentence rather than as a form
   filled with fall-backs somebody could press Save on.

   Two halves rather than one string, with the path between them: the file is an
   identifier and the window sets it in mono, which a single sentence would have
   no way to ask for.

   The window it belongs to is not only about this file any more, which is what
   makes the case worth a sentence at all rather than a greyed menu item. It
   also carries this machine's caveman level for the project, which lives in
   `settings.json` and is nothing to do with the repository — so a project
   nobody has set up still has something to change here, and the fields are the
   only part that has to say why it is absent. */
const NOTICE = {
  missing: {
    lead: 'This project has no',
    tail: 'yet, so there is nothing here to fill in. Set the project up from the same menu and these fields appear.'
  },
  broken: {
    lead: "This project's",
    tail: 'will not parse, so there are no values to put in a form. Set the project up again from the same menu to have it written afresh.'
  }
}

const UNKNOWN_NOTICE = {
  lead: "There is no saying what state this project's",
  tail: 'is in, so no fields are offered for it. The level above is this machine’s own and is unaffected.'
}

/* Whether the four fields and Save are drawn at all: a parsed file and nothing
   looser. `configured` in `views/DesktopApp.vue` is the same question one word
   shorter, and the two must stay the same question — a form opened over a file
   this side has not parsed would be four fall-backs presented as the project's
   values. */
export function offersDefaults(state) {
  return state === 'ok'
}

/* The sentence that stands in their place, or `null` when there is nothing to
   explain because the fields are there. */
export function configNotice(state) {
  if (offersDefaults(state)) return null
  return NOTICE[state] ?? UNKNOWN_NOTICE
}
