import { describe, expect, it } from 'vitest'
import { defaultReviewer, modelOptionsFor, reviewerRows } from '../../../src/components/git/reviewAgent.js'

const agents = [
  { id: 'claude', label: 'Claude Code', models: [{ id: 'opus', label: 'Opus' }] },
  { id: 'codex', label: 'Codex', models: [{ id: 'gpt-5.6-sol', label: 'GPT-5.6-Sol' }] }
]
const table = (agent, model, reviewBranch = { agent: '', model: '' }) => ({
  agent,
  model,
  agentRoles: { tasks: { agent: '', model: '' }, code: { agent: '', model: '' }, runLead: { agent: '', model: '' }, reviewBranch }
})

describe('reviewerRows', () => {
  it('lists only installed harnesses, in the order given, with the catalogue models', () => {
    const rows = reviewerRows(table('claude', 'opus'), ['codex'], agents)
    expect(rows.map((r) => r.id)).toEqual(['codex'])
    expect(rows[0].models).toEqual(agents[1].models)
    expect(rows[0].label).toBe('Codex')
  })
  it("takes the role's model for the role's harness and the root's for the root's", () => {
    const rows = reviewerRows(table('claude', 'opus', { agent: 'codex', model: 'gpt-5.6-sol' }), ['claude', 'codex'], agents)
    expect(rows.map((r) => r.model)).toEqual(['opus', 'gpt-5.6-sol'])
  })
  it('leaves the model empty for a harness neither pair names', () => {
    const rows = reviewerRows(table('claude', 'opus'), ['claude', 'codex'], agents)
    expect(rows.map((r) => r.model)).toEqual(['opus', ''])
  })
  it('inherits the root pair when the role names nothing', () => {
    const rows = reviewerRows(table('codex', 'gpt-5.6-sol'), ['claude', 'codex'], agents)
    expect(rows.map((r) => r.model)).toEqual(['', 'gpt-5.6-sol'])
  })
  it('skips an installed id the catalogue does not list', () => {
    expect(reviewerRows(table('claude', ''), ['gemini', 'claude'], agents).map((r) => r.id)).toEqual(['claude'])
  })
})

describe('defaultReviewer', () => {
  it("is the review role's harness when installed", () => {
    const t = table('claude', '', { agent: 'codex', model: '' })
    expect(defaultReviewer(reviewerRows(t, ['claude', 'codex'], agents), t)).toBe('codex')
  })
  it('falls back to the first installed harness', () => {
    const t = table('claude', '', { agent: 'codex', model: '' })
    expect(defaultReviewer(reviewerRows(t, ['claude'], agents), t)).toBe('claude')
    expect(defaultReviewer([], t)).toBe('')
  })
})

describe('modelOptionsFor', () => {
  it('offers Agent chooses first and keeps an unavailable saved slug', () => {
    const row = reviewerRows(table('codex', 'old-slug'), ['codex'], agents)[0]
    const options = modelOptionsFor(row, 'old-slug')
    expect(options[0]).toEqual({ value: '', label: 'Agent chooses' })
    expect(options.at(-1)).toEqual({ value: 'old-slug', label: 'old-slug (Unavailable)', disabled: true })
  })
})
