import { describe, expect, it } from 'vitest'
import { forgetSummary, loadSummary, projectName, saveSummary, taskCount } from '../src/summary.js'

const memory = () => {
  const map = new Map()
  return { getItem: (k) => map.get(k) ?? null, setItem: (k, v) => map.set(k, v), removeItem: (k) => map.delete(k) }
}

describe('the home summary', () => {
  it('names the project by its folder', () => {
    expect(projectName('/work/smetana')).toBe('smetana')
    expect(projectName(null)).toBeNull()
  })
  it('counts tasks without the tracker\'s lock issues', () => {
    const issues = [{ id: 1 }, { id: 2, lock: true }, { id: 3 }]
    expect(taskCount(issues, (i) => Boolean(i.lock))).toBe(2)
  })
  it('keeps the last read for the unreachable screen', () => {
    const store = memory()
    expect(loadSummary(store)).toBeNull()
    saveSummary(store, { project: 'smetana', tasks: 4 })
    expect(loadSummary(store)).toMatchObject({ project: 'smetana', tasks: 4 })
    forgetSummary(store)
    expect(loadSummary(store)).toBeNull()
  })
  it('treats a damaged copy as none', () => {
    const store = memory()
    store.setItem('smetana.companion.summary', '{nonsense')
    expect(loadSummary(store)).toBeNull()
  })
})
