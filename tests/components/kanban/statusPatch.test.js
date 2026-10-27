import { describe, expect, it } from 'vitest'
import { statusPatch } from '../../../src/components/kanban/statusPatch.js'
import { promotedNote } from '../../../src/components/run/promotedNote.js'

describe('statusPatch', () => {
  it('carries a promoted note only on a genuine promote to ready', () => {
    expect(statusPatch('deferred', 'open')).toEqual({ status: 'open', append_notes: promotedNote('status') })
    expect(statusPatch('open', 'open')).toEqual({ status: 'open' })
    expect(statusPatch('in_progress', 'open')).toEqual({ status: 'open' })
    expect(statusPatch('open', 'closed')).toEqual({ status: 'closed' })
  })

  it('carries a note for a status that vanished from the store', () => {
    expect(statusPatch(undefined, 'open').append_notes).toBe(promotedNote('status'))
  })

  it('writes no note for a target other than ready', () => {
    expect(statusPatch('parked', 'pinned')).toEqual({ status: 'pinned' })
  })
})
