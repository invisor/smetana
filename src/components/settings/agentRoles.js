/* The Models group on the Agents tab, as a rule rather than as markup: which
   rows it draws, what each of its two dropdowns offers, and what a choice in
   one of them changes.

   Out of `AgentSettings.vue` for the reason every rule in this tree is out of
   the component that draws it — a `.vue` file is the one thing no test in this
   repository can reach, and this rule has a case that is easy to get wrong and
   invisible when it is: choosing a model in a role that has chosen no harness
   has to fill the harness in as well, because the pair is indivisible on the
   Rust side and a role holding a model behind an inherited harness is the one
   state `settings/model.rs` throws away.

   Nothing here names a harness or a model. The vocabulary is Rust's — the rows
   `agents_catalog` answers with, reached through `stores/agents.js` — and this
   file only says which of them go in which list. */

/* The empty string is not a harness and not a model: in a role it means
   "whatever the root says", and in the root's own model it means "pass no flag
   at all and let the harness pick". A dropdown cannot show an empty label, so
   both are drawn as a row of their own. */
export const INHERIT = ''

/* What an inheriting role shows in both of its fields. Sentence case, and it
   says what will happen rather than what is stored. */
export const SAME_AS_DEFAULT = 'Same as default'

/* What the default row's own model field shows when nothing is chosen. A
   different sentence from the one above because it is a different fact: the
   default row has nothing to inherit from, and what it does instead is say
   nothing to the agent at all.

   "Agent" rather than "harness", which is the word the rest of this tab uses,
   and short on purpose: it shares a field with `GPT-5.6-Terra`, and `Dropdown`
   ellipsises a label that does not fit rather than growing the field. */
export const HARNESS_CHOOSES = 'Agent chooses'

/* The five rows, in the order they are drawn. `Default` leads because
   everything under it falls back to it, and the other four are in the order a
   piece of work goes through them: a task is filed, code is written, a run
   carries it, a branch is reviewed.

   `role` is `null` for the default row and the key of `settings.agentRoles`
   otherwise — the same four keys Rust serializes, so a spelling that drifted
   here would be a choice written into a key nothing reads.

   The description is carried on one row only. Every other row's behaviour is
   what the label already says; the Code row's is not, and saying it outright is
   cheaper than a setting that quietly does half of what it looks like it
   does. */
export const ROLE_ROWS = [
  {
    role: null,
    label: 'Default',
    description:
      'Behind every row below, and what a session with no row of its own uses — a bare agent, project setup, a one-off question. Every row here reaches the next session started; the ones already running keep what they started with.'
  },
  { role: 'tasks', label: 'Tasks', description: '' },
  {
    role: 'code',
    label: 'Code',
    description:
      "This agent fixes merged work and finishes a conflicted merge. A run's workers are asked for the model only: they are started inside the run agent's own session, so nothing here can change which agent they are."
  },
  { role: 'runLead', label: 'Run lead', description: '' },
  { role: 'reviewBranch', label: 'Branch review', description: '' }
]

/* The pair one row currently stands for: its own where it named a harness, and
   the root's — both halves — where it did not.

   `role` is `null` for the default row, whose pair is the root's by definition.
   A role object that is not there at all answers as inheriting, which is what a
   settings file written before this existed leaves behind. */
export function pairOf(role, roles, rootAgent, rootModel) {
  const stored = role ? roles?.[role] : null
  if (role && stored?.agent) {
    return { agent: stored.agent, model: stored.model ?? INHERIT, inherited: false }
  }
  return { agent: rootAgent, model: role ? INHERIT : rootModel, inherited: Boolean(role) }
}

/* Which harness a run would actually start, which is the `runLead` row's — its
   own where it named one, the root's where it did not.

   Named for the question rather than left to each caller as a `pairOf` on a
   particular role, because the two callers are the two windows' subscription
   probes and neither of them is about roles: the allowance strip in the app and
   the block on the Agents tab both ask "whose subscription would tonight's
   batch spend", and `runs/commands.rs` answers the same question the same way
   for a caller that names nobody. Reading the root `agent` there instead — which
   is what both did before roles existed — draws Claude Code's allowance, and
   the sentence about a run taking fewer tasks, over a run spending Codex's. */
export function runLeadAgent(roles, rootAgent) {
  return pairOf('runLead', roles, rootAgent, INHERIT).agent
}

/* What the harness dropdown offers on one row: every harness this build ships,
   and — for a role, never for the default — the row that gives the choice back.

   `agents` is `stores/agents.js`'s list, so an id added in Rust is offered here
   for free and one removed there stops being offered. */
export function providerOptions(role, agents) {
  const shipped = agents.map((agent) => ({ value: agent.id, label: agent.label }))
  return role ? [{ value: INHERIT, label: SAME_AS_DEFAULT }, ...shipped] : shipped
}

/* What the model dropdown offers on one row: the models of whichever harness
   that row currently resolves to, under the row that chooses none.

   The first row's sentence differs by whether the harness is this row's own or
   inherited, because the two are different facts — see `HARNESS_CHOOSES`. A
   harness the catalogue has never heard of offers nothing, and the field is
   then the empty row alone rather than a picker with a stale list in it. */
export function modelOptions(role, agents, agent, inherited) {
  const models = agents.find((row) => row.id === agent)?.models ?? []
  const nothing = { value: INHERIT, label: inherited ? SAME_AS_DEFAULT : HARNESS_CHOOSES }
  return [nothing, ...models.map((model) => ({ value: model.id, label: model.label }))]
}

/* The edit a harness dropdown makes, as the fields it changes.

   Choosing a harness clears the model beside it, always: the model that was
   there was chosen against the harness that has just been replaced, and
   `settings/model.rs` would throw it away on the next read anyway. Doing it here
   as well is what stops the field showing a model the new harness has never
   heard of for as long as the window is open.

   The two kinds of row make the **same** edit, which is why there is no branch
   here: `role` is already `null` for the default row, and a ternary whose arms
   are identical reads as though the two differed. Where they do differ is who
   unpacks the answer — the default row's `null` lands on the root `agent` and
   `model`, a role's on its own pair — and that is `SettingsWindow.vue`'s to
   know, not this module's. `null` for the harness on a role is the inheritance
   coming back, and it takes the model with it. */
export function chooseProvider(role, agent) {
  return { role, pair: { agent, model: INHERIT } }
}

/* The edit a model dropdown makes.

   The case this file exists for: a role that has chosen no harness gets the
   root's written into it first. Without that the app would be storing a model
   with no harness beside it — half a pair, which validation empties, so the
   choice would vanish at the next read with nothing on screen to say why.

   For the default row and for a role that already names a harness, the harness
   is left exactly as it is. */
export function chooseModel(role, model, roles, rootAgent) {
  if (!role) return { role: null, pair: { agent: rootAgent, model } }
  const agent = roles?.[role]?.agent || rootAgent
  return { role, pair: { agent, model } }
}
