import { describe, expect, it } from 'vitest'
import { searchIssues } from '../../../src/components/shell/taskSearch.js'

const issue = (over = {}) => ({
  id: 'smetana-a1a',
  title: 'A title',
  status: 'open',
  issue_type: 'task',
  description: null,
  acceptance_criteria: null,
  design: null,
  notes: null,
  labels: [],
  updated_at: '2026-08-01T00:00:00Z',
  ...over
})

describe('searchIssues', () => {
  it('answers nothing at all for an empty query', () => {
    expect(searchIssues([issue()], '')).toEqual([])
    expect(searchIssues([issue()], '   ')).toEqual([])
  })

  it('puts an exact id above every other kind of match', () => {
    const hits = searchIssues(
      [issue({ id: 'smetana-b2b', title: 'smetana-a1a is mentioned here' }), issue()],
      'smetana-a1a'
    )
    expect(hits.map((hit) => hit.id)).toEqual(['smetana-a1a', 'smetana-b2b'])
    expect(hits[0].field).toBe('id')
  })

  it('matches an id by its prefix', () => {
    expect(searchIssues([issue()], 'smetana-a1').map((hit) => hit.id)).toEqual(['smetana-a1a'])
  })

  it('puts a title above prose, and an earlier occurrence above a later one', () => {
    const hits = searchIssues(
      [
        issue({ id: 'x-1', title: 'Nothing here', description: 'the bell rings' }),
        issue({ id: 'x-2', title: 'Long preamble before the bell' }),
        issue({ id: 'x-3', title: 'Bell at the front' })
      ],
      'bell'
    )
    expect(hits.map((hit) => hit.id)).toEqual(['x-3', 'x-2', 'x-1'])
  })

  it('ignores case on both sides', () => {
    expect(searchIssues([issue({ title: 'The Bell' })], 'bELL')).toHaveLength(1)
  })

  it('searches every prose field and names the one that matched', () => {
    const fields = {
      description: 'description',
      acceptance_criteria: 'acceptanceCriteria',
      design: 'design',
      notes: 'notes'
    }
    for (const [key, name] of Object.entries(fields)) {
      const hits = searchIssues([issue({ [key]: 'a needle in it' })], 'needle')
      expect(hits).toHaveLength(1)
      expect(hits[0].field).toBe(name)
    }
  })

  it('searches labels', () => {
    const hits = searchIssues([issue({ labels: ['frontend', 'needle'] })], 'needle')
    expect(hits[0].field).toBe('labels')
  })

  it('carries a snippet around a prose match, and none for a title match', () => {
    const long = `${'x'.repeat(200)} needle ${'y'.repeat(200)}`
    const [hit] = searchIssues([issue({ description: long })], 'needle')
    expect(hit.snippet).toContain('needle')
    expect(hit.snippet.length).toBeLessThan(120)
    expect(hit.snippet.startsWith('…')).toBe(true)
    expect(hit.snippet.endsWith('…')).toBe(true)
    expect(searchIssues([issue({ title: 'needle' })], 'needle')[0].snippet).toBe('')
  })

  it('breaks a tie on the newest first, then on the id', () => {
    const hits = searchIssues(
      [
        issue({ id: 'x-2', title: 'needle', updated_at: '2026-01-01T00:00:00Z' }),
        issue({ id: 'x-1', title: 'needle', updated_at: '2026-08-01T00:00:00Z' }),
        issue({ id: 'x-3', title: 'needle', updated_at: '2026-08-01T00:00:00Z' })
      ],
      'needle'
    )
    expect(hits.map((hit) => hit.id)).toEqual(['x-1', 'x-3', 'x-2'])
  })

  it('never returns more than the limit', () => {
    const many = Array.from({ length: 40 }, (_, n) => issue({ id: `x-${n}`, title: 'needle' }))
    expect(searchIssues(many, 'needle')).toHaveLength(20)
    expect(searchIssues(many, 'needle', 3)).toHaveLength(3)
  })

  it('survives an issue whose prose fields are all missing', () => {
    expect(() => searchIssues([{ id: 'x-1', title: 'needle' }], 'needle')).not.toThrow()
  })

  it('takes any iterable, since the store holds a Map', () => {
    const map = new Map([['x-1', issue({ id: 'x-1', title: 'needle' })]])
    expect(searchIssues(map.values(), 'needle')).toHaveLength(1)
  })
})
