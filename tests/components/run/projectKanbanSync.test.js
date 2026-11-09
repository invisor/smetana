import { describe, expect, it } from 'vitest'
import { KANBAN_SYNC_DEFAULTS, sameSync, syncDraft, syncErrors } from '../../../src/components/run/projectKanbanSync.js'

describe('project Kanban sync draft', () => {
  it('starts disabled with commands and interval, without sharing objects', () => {
    const first = syncDraft(null)
    const second = syncDraft(null)
    expect(first).toEqual(KANBAN_SYNC_DEFAULTS)
    first.enabled = true
    expect(second.enabled).toBe(false)
    expect(sameSync(null, KANBAN_SYNC_DEFAULTS)).toBe(true)
  })

  it('validates only an enabled configuration', () => {
    expect(syncErrors({ enabled: false, pullCommand: '', pushCommand: '', pullIntervalMinutes: 0 })).toEqual({})
    for (const pullIntervalMinutes of ['', 0, -1, 1.5, 1441]) {
      expect(syncErrors({ ...KANBAN_SYNC_DEFAULTS, enabled: true, pullIntervalMinutes })).toHaveProperty('pullIntervalMinutes')
    }
    expect(syncErrors({ ...KANBAN_SYNC_DEFAULTS, enabled: true, pullCommand: ' ', pushCommand: '' })).toEqual({
      pullCommand: 'Enter a pull command.', pushCommand: 'Enter a push command.'
    })
    expect(syncErrors({ ...KANBAN_SYNC_DEFAULTS, enabled: true })).toEqual({})
  })
})
