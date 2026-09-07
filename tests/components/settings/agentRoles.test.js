import { describe, expect, it } from 'vitest'
import {
  chooseModel,
  chooseProvider,
  HARNESS_CHOOSES,
  modelOptions,
  pairOf,
  providerOptions,
  ROLE_ROWS,
  runLeadAgent,
  SAME_AS_DEFAULT
} from '../../../src/components/settings/agentRoles.js'

/* The rows of the Models group are five dropdown pairs, and almost all of it is
   drawing. What is a rule — and what is worth a test, since no test here can
   reach the `.vue` file — is the inheritance: what an untouched role shows,
   what its model list is taken from, and the one case that is silently wrong
   when it is wrong at all, which is choosing a model in a role that has chosen
   no harness. */

/* `agents_catalog`'s shape, cut to what these rules read. */
const AGENTS = [
  {
    id: 'claude',
    label: 'Claude Code',
    models: [
      { id: 'fable', label: 'Fable' },
      { id: 'opus', label: 'Opus' }
    ]
  },
  {
    id: 'codex',
    label: 'Codex',
    models: [{ id: 'gpt-5.6-sol', label: 'GPT-5.6-Sol' }]
  }
]

const empty = () => ({
  tasks: { agent: '', model: '' },
  code: { agent: '', model: '' },
  runLead: { agent: '', model: '' },
  reviewBranch: { agent: '', model: '' }
})

describe('the rows', () => {
  it('draws the default first and the four roles under it', () => {
    expect(ROLE_ROWS.map((row) => row.role)).toEqual([
      null,
      'tasks',
      'code',
      'runLead',
      'reviewBranch'
    ])
    expect(ROLE_ROWS.map((row) => row.label)).toEqual([
      'Default',
      'Tasks',
      'Code',
      'Run lead',
      'Branch review'
    ])
  })

  /* The role keys are the file's own — `settings/model.rs` serializes exactly
     these four — so a spelling that drifted here would write a person's choice
     into a key nothing reads and lose it at the next open. */
  it('names the roles as the settings file spells them', () => {
    const roles = ROLE_ROWS.map((row) => row.role).filter(Boolean)
    expect(Object.keys(empty())).toEqual(roles)
  })

  it('says out loud what the code row does not reach', () => {
    const code = ROLE_ROWS.find((row) => row.role === 'code')
    expect(code.description).toMatch(/workers are asked for the model only/)
    expect(code.description).toMatch(/conflicted merge/)
  })

  /* A label names a row; it does not say which agent calls land in it, and
     that is `agents::role_of`'s answer rather than anything a reader can get
     from the word `Tasks`. So every row says it, and none of them says it at
     more length than the row everything else falls back to. */
  it('gives every row a sentence, and none longer than the default row', () => {
    const byRole = Object.fromEntries(ROLE_ROWS.map((row) => [String(row.role), row.description]))
    for (const [role, description] of Object.entries(byRole)) {
      expect(description, `the ${role} row has no description`).toBeTruthy()
      expect(description.length).toBeLessThanOrEqual(byRole.null.length)
    }
    /* One word per row rather than the phrase it sits in. A phrase-level
       regex fails on an honest reword that changes no fact, so it pins the
       wording instead of the meaning; each of these is a word the sentence
       cannot lose without losing what the row is for. A parked task is the
       case no label suggests, the lead-and-workers pair is the thing the Run
       lead row is misread about, and a review is about branches. */
    expect(byRole.tasks).toMatch(/parked/)
    expect(byRole.runLead).toMatch(/lead/)
    expect(byRole.runLead).toMatch(/workers/)
    expect(byRole.reviewBranch).toMatch(/branch/)
  })
})

describe('what a row stands for', () => {
  it('an untouched role is the root pair, both halves of it', () => {
    expect(pairOf('tasks', empty(), 'claude', 'opus')).toEqual({
      agent: 'claude',
      model: '',
      inherited: true
    })
  })

  it('a role that named a harness stands for its own pair', () => {
    const roles = { ...empty(), tasks: { agent: 'codex', model: 'gpt-5.6-sol' } }
    expect(pairOf('tasks', roles, 'claude', 'opus')).toEqual({
      agent: 'codex',
      model: 'gpt-5.6-sol',
      inherited: false
    })
  })

  it('the default row is the root pair and never inherits', () => {
    expect(pairOf(null, empty(), 'claude', 'opus')).toEqual({
      agent: 'claude',
      model: 'opus',
      inherited: false
    })
  })

  /* Every settings file written before this feature existed. */
  it('a file with no roles object at all reads as inheriting', () => {
    expect(pairOf('runLead', undefined, 'claude', '')).toEqual({
      agent: 'claude',
      model: '',
      inherited: true
    })
  })
})

describe('whose subscription a run would spend', () => {
  it('is the run lead row where it names a harness', () => {
    const roles = { ...empty(), runLead: { agent: 'codex', model: 'gpt-5.6-sol' } }
    expect(runLeadAgent(roles, 'claude')).toBe('codex')
  })

  it('is the root where that row chose nothing', () => {
    expect(runLeadAgent(empty(), 'claude')).toBe('claude')
    expect(runLeadAgent(undefined, 'codex')).toBe('codex')
  })

  /* The row beside it is not this question: only the lead's harness decides
     which subscription a batch spends, since the workers live inside it. */
  it('is not moved by any other row', () => {
    const roles = { ...empty(), code: { agent: 'codex', model: 'gpt-5.6-sol' } }
    expect(runLeadAgent(roles, 'claude')).toBe('claude')
  })
})

describe('what the two dropdowns offer', () => {
  it('offers every shipped harness, and the giving-back row only to a role', () => {
    expect(providerOptions('tasks', AGENTS)).toEqual([
      { value: '', label: SAME_AS_DEFAULT },
      { value: 'claude', label: 'Claude Code' },
      { value: 'codex', label: 'Codex' }
    ])
    expect(providerOptions(null, AGENTS)).toEqual([
      { value: 'claude', label: 'Claude Code' },
      { value: 'codex', label: 'Codex' }
    ])
  })

  it('takes the model list from whichever harness the row resolves to', () => {
    expect(modelOptions('tasks', AGENTS, 'codex', false)).toEqual([
      { value: '', label: HARNESS_CHOOSES },
      { value: 'gpt-5.6-sol', label: 'GPT-5.6-Sol' }
    ])
  })

  it('an inheriting role says so in the model field too', () => {
    expect(modelOptions('tasks', AGENTS, 'claude', true)[0]).toEqual({
      value: '',
      label: SAME_AS_DEFAULT
    })
  })

  /* A hand-edited file, or a catalogue that could not be read: the field is the
     empty row alone rather than a picker holding somebody else's list.

     This is also where the lookup itself is pinned, since `modelOptions` is the
     only place in the app that turns a harness id into its models — the store
     carries the rows and does not wrap the `find`, for the reason `agentLabel`
     in `stores/agents.js` records. The empty string is in here because it is
     not a hypothetical: it is what a role with nothing chosen holds, and this
     function is asked about it on every render. */
  it('offers nothing but the empty row for a harness nobody ships', () => {
    expect(modelOptions(null, AGENTS, 'cursor', false)).toEqual([
      { value: '', label: HARNESS_CHOOSES }
    ])
    expect(modelOptions('tasks', AGENTS, '', true)).toEqual([
      { value: '', label: SAME_AS_DEFAULT }
    ])
    expect(modelOptions(null, AGENTS, null, false)).toHaveLength(1)
    expect(modelOptions(null, [], 'claude', false)).toHaveLength(1)
  })

  /* The order is each profile's own `MODELS`, strongest first, and neither this
     module nor the store reshapes it: a list reordered on the way to a dropdown
     would put a different model under the same cursor position on two
     machines. */
  it('offers the models of a harness in the order Rust offered them', () => {
    expect(modelOptions(null, AGENTS, 'claude', false)).toEqual([
      { value: '', label: HARNESS_CHOOSES },
      { value: 'fable', label: 'Fable' },
      { value: 'opus', label: 'Opus' }
    ])
  })
})

describe('what a choice changes', () => {
  it('a new harness drops the model that was chosen against the old one', () => {
    expect(chooseProvider('tasks', 'codex')).toEqual({
      role: 'tasks',
      pair: { agent: 'codex', model: '' }
    })
    expect(chooseProvider(null, 'codex')).toEqual({
      role: null,
      pair: { agent: 'codex', model: '' }
    })
  })

  it('giving a role back to the default takes its model with it', () => {
    expect(chooseProvider('code', '')).toEqual({ role: 'code', pair: { agent: '', model: '' } })
  })

  /* The case this module exists for. Storing a model with no harness beside it
     is half a pair, and `settings/model.rs` empties it on the next read: the
     choice would be gone by the next open with nothing on screen to say why. */
  it('choosing a model in an empty role fills its harness in first', () => {
    expect(chooseModel('runLead', 'opus', empty(), 'claude')).toEqual({
      role: 'runLead',
      pair: { agent: 'claude', model: 'opus' }
    })
  })

  it('leaves a harness a role already chose exactly where it is', () => {
    const roles = { ...empty(), runLead: { agent: 'codex', model: '' } }
    expect(chooseModel('runLead', 'gpt-5.6-sol', roles, 'claude')).toEqual({
      role: 'runLead',
      pair: { agent: 'codex', model: 'gpt-5.6-sol' }
    })
  })

  it('the default row writes the root pair', () => {
    expect(chooseModel(null, 'opus', empty(), 'claude')).toEqual({
      role: null,
      pair: { agent: 'claude', model: 'opus' }
    })
  })
})
