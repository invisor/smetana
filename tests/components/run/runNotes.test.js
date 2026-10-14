import { describe, expect, it } from 'vitest'
import { NOTES_MAX, notesPayload } from '../../../src/components/run/runNotes.js'

describe('notesPayload', () => {
  it('sends null for nothing, for whitespace and for a missing value', () => {
    expect(notesPayload('')).toBeNull()
    expect(notesPayload('  \n\t ')).toBeNull()
    expect(notesPayload(undefined)).toBeNull()
    expect(notesPayload(null)).toBeNull()
  })

  it('trims the ends and keeps the lines between', () => {
    expect(notesPayload('  do not commit\n\nask first \n')).toBe('do not commit\n\nask first')
  })

  it('keeps the bound the Rust side enforces', () => {
    expect(NOTES_MAX).toBe(2000)
  })
})
