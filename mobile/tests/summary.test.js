import { describe, expect, it } from 'vitest'
import { forgetSummary, loadSummary, projectName, saveSummary, savedLook, taskCount } from '../src/summary.js'

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
  it('keeps the desktop\'s look, and forgets it with the rest', () => {
    const store = memory()
    saveSummary(store, { project: 'smetana', tasks: 4, look: { theme: 'light', density: 'compact' } })
    expect(savedLook(loadSummary(store))).toEqual({ theme: 'light', density: 'compact' })
    forgetSummary(store)
    expect(savedLook(loadSummary(store))).toBeNull()
  })
  it('reads a copy with no look, or a damaged one, as no look', () => {
    expect(savedLook({ tasks: 1 })).toBeNull()
    expect(savedLook({ tasks: 1, look: { theme: 3 } })).toBeNull()
    expect(savedLook(null)).toBeNull()
  })
  it('treats a damaged copy as none', () => {
    const store = memory()
    store.setItem('smetana.companion.summary', '{nonsense')
    expect(loadSummary(store)).toBeNull()
  })
})
