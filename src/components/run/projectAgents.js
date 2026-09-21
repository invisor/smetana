/* The Agents group of the Project settings dialog, as a rule rather than as
   markup: what the switch's two labels are, what turning it on seeds the
   table with, when two tables count as the same one, and when the dialog's
   single Save may be pressed.

   The `projectDefaults.js` family beside it: pure, no Vue and no DOM, for the
   reason every rule in this tree is out of the component that draws it — a
   `.vue` file is the one thing no test here can reach.

   This group edits a different file from the one beside it in the same
   dialog. `projectDefaults.js` is about `.smetana/project.toml`, committed and
   shared with whoever else works in the repository; this one is about
   `settings.project.agents` in `settings.json`, this machine's own — which
   harness a project uses is a fact about a person's subscriptions and not
   about the repository (`.claude/rules/settings.md`). The two live in one
   dialog and share one Save button, which is why `canSaveProject` below takes
   both halves' state rather than either file answering for the other. */

/* The four stored role keys, derived from `settings/agentRoles.js`'s
   `ROLE_ROWS` rather than written out as a sixth literal copy of a list the
   hazards notes already track five of: that file's rows include the `null`
   default row this module has no business naming, so the keys alone are
   `ROLE_ROWS`'s own `role` field with the one falsy entry filtered out —
   still one list, read rather than repeated. */
import { ROLE_ROWS } from '../settings/agentRoles.js'
const ROLE_NAMES = ROLE_ROWS.map((row) => row.role).filter(Boolean)

/* The switch's own two words. Sentence case, said once here so the row and
   any test of it read the identical sentence. */
export const OWN_AGENTS_LABEL = 'Use own agents for this project'
export const OWN_AGENTS_DESCRIPTION =
  'Off, this project follows the harness and models chosen on the Agents tab of Settings. On, it keeps a table of its own, on this machine — useful for a project you want built by Codex where the rest of your work runs on Claude Code.'

/* A whole, safe copy of an agent table — the root's three fields, or a
   project's own stored block — with all four roles written out explicitly
   rather than left to arrive from whichever object was handed in.

   Two callers want exactly this: turning the switch on seeds the draft from
   the global (root) table, and opening the dialog over a project that already
   carries a block has to copy it rather than edit the prop in place. Both
   want the same shape out the other end, so there is one function rather than
   two — "seed from the global table" is the name because that is the case the
   design asked for, and reading it as a general normalising copy is what lets
   the second caller reuse it. Missing pieces read as "inherit", the same
   reading `settings/model.rs` gives a role it has never heard of, so a table
   copied from a file written before a role existed is indistinguishable from
   one that always inherited it. */
export function seedFromGlobal(table) {
  const agentRoles = {}
  for (const role of ROLE_NAMES) {
    const pair = table?.agentRoles?.[role]
    agentRoles[role] = {
      agent: typeof pair?.agent === 'string' ? pair.agent : '',
      model: typeof pair?.model === 'string' ? pair.model : ''
    }
  }
  return {
    agent: typeof table?.agent === 'string' ? table.agent : '',
    model: typeof table?.model === 'string' ? table.model : '',
    agentRoles
  }
}

/* Whether two tables are the same one, field by field — `null` included,
   since `null` is a real state here ("the root table, entirely") and not a
   stand-in for an empty object. `null` equals only `null`; a table, however it
   arrived, is compared against another the same shape `seedFromGlobal`
   produces so that a role missing from one side and empty on the other still
   reads as equal. */
export function sameAgentTable(a, b) {
  if (a === null || b === null) return a === b
  if ((a.agent ?? '') !== (b.agent ?? '') || (a.model ?? '') !== (b.model ?? '')) return false
  for (const role of ROLE_NAMES) {
    const pa = a.agentRoles?.[role]
    const pb = b.agentRoles?.[role]
    if ((pa?.agent ?? '') !== (pb?.agent ?? '')) return false
    if ((pa?.model ?? '') !== (pb?.model ?? '')) return false
  }
  return true
}

/* Whether the dialog's single Save may be pressed.

   Two files, one button, and the rule is stricter than "either half changed":
   a file that offers fields at all (`fields`) has to be *valid* before either
   half may be saved, because Save writes `defaults` and `agents` in the one
   press `saveProjectSettings` makes — a block changed alone while the
   defaults sit on an invalid number would otherwise have no way to be saved
   without first fixing a form the person may not be touching at all. `busy`
   holds everything, the same as `projectDefaults.js`'s own `canSave` did.

   Where there is no file to offer fields from (`!fields`), the defaults half
   is not this dialog's to change at all, so only the agents block decides —
   which is the case the design asked for by name: "changed the block with no
   file → true". */
export function canSaveProject({ busy, fields, defaultsDirty, defaultsValid, agentsDirty }) {
  if (busy) return false
  if (fields && !defaultsValid) return false
  const defaultsChangeable = Boolean(fields) && Boolean(defaultsDirty)
  return defaultsChangeable || Boolean(agentsDirty)
}
