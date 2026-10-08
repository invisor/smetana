/* Who reviews a branch, when the window has a choice to offer.

   Pure, no Vue and no Tauri, for the reason the whole family under
   `components/` is: the dialog is a `.vue` file no test here can reach. The
   app window computes these rows out of the effective roles table and
   `agents_installed`, and hands the dialog window the result over IPC.

   The settings are the memory and the dialog is one run: a row's `model` is
   what the settings would have started this harness with — the Code review
   role's model when that role names the harness, the root's when the root
   does, and nothing otherwise — so picking a harness in the window lands on
   exactly the model the settings already chose for it. */
import { HARNESS_CHOOSES, INHERIT, modelOptions, pairOf, unavailableModelOption } from '../settings/agentRoles.js'

export const AGENT_CHOOSES = HARNESS_CHOOSES

const settingsModelFor = (table, id) => {
  const role = pairOf('reviewBranch', table?.agentRoles, table?.agent, table?.model)
  if (role.agent === id && !role.inherited) return role.model ?? INHERIT
  if (table?.agent === id) return table.model ?? INHERIT
  return INHERIT
}

/* One row per installed harness, in the order `installed` gives (Rust's `IDS`
   order). An id the catalogue does not list has no row: nothing to label it by. */
export function reviewerRows(table, installed, agents) {
  return (installed ?? [])
    .map((id) => {
      const row = (agents ?? []).find((agent) => agent?.id === id)
      return row
        ? { id, label: row.label ?? id, model: settingsModelFor(table, id), models: row.models ?? [] }
        : null
    })
    .filter(Boolean)
}

/* The review role's harness when it has a row, otherwise the first installed
   one — what `agents::pick` would substitute. */
export function defaultReviewer(rows, table) {
  if (!rows?.length) return ''
  const wanted = pairOf('reviewBranch', table?.agentRoles, table?.agent, table?.model).agent
  return rows.some((row) => row.id === wanted) ? wanted : rows[0].id
}

/* The model dropdown's options for one row, keeping a settings slug the
   catalogue no longer lists as an unavailable entry rather than dropping it. */
export function modelOptionsFor(row, model) {
  const options = modelOptions(null, [{ id: row.id, models: row.models }], row.id, false)
  return unavailableModelOption(options, model)
}
