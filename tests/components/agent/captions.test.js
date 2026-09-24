import { describe, expect, it } from 'vitest'
import { CAPTION, captionOf } from '../../../src/components/agent/captions.js'

describe('what a session row is captioned by', () => {
  it('puts the issue id in mono beside the verb', () => {
    expect(captionOf({ kind: 'editTask', id: 'x-1' })).toEqual({ label: 'Editing', tasks: ['x-1'] })
    expect(captionOf({ kind: 'fixTask', id: 'x-1' })).toEqual({ label: 'Fixing', tasks: ['x-1'] })
  })

  it("names a conflict by the repository's folder and the incoming branch", () => {
    expect(captionOf({ kind: 'resolveConflict', repo: '/p/app', theirs: 'feat' }))
      .toEqual({ label: 'Conflict', tasks: ['app', 'feat'] })
  })

  it('puts a resumed session title inside the label, where prose belongs', () => {
    expect(captionOf({ kind: 'resumeSession', title: 'Move it' }))
      .toEqual({ label: 'Resumed session: Move it', tasks: [] })
    expect(captionOf({ kind: 'resumeSession', title: null })).toEqual({ label: 'Resumed session', tasks: [] })
  })

  it('reads an unknown kind and a missing work as a bare agent', () => {
    expect(captionOf({ kind: 'somethingNew' })).toEqual({ label: CAPTION.bare, tasks: [] })
    expect(captionOf(null)).toEqual({ label: 'Agent', tasks: [] })
  })

  it("captions a run by what it claimed, and by nothing until it has", () => {
    expect(captionOf({ kind: 'run' }, ['x-1', 'x-2'])).toEqual({ label: null, tasks: ['x-1', 'x-2'] })
    expect(captionOf({ kind: 'run' }, [])).toEqual({ label: 'Agent', tasks: [] })
  })

  it('names a row by its title when the worker has one, and keeps the ids beside it', () => {
    expect(captionOf({ kind: 'newTask', text: 'x' }, [], 'Rename the rows'))
      .toEqual({ label: 'Rename the rows', tasks: [] })
    expect(captionOf({ kind: 'editTask', id: 'x-1' }, [], 'Tighten the criteria'))
      .toEqual({ label: 'Tighten the criteria', tasks: ['x-1'] })
    expect(captionOf({ kind: 'newTask', text: 'x' }, [], '   ')).toEqual({ label: 'Creating a task', tasks: [] })
    expect(captionOf({ kind: 'run' }, ['a-1'], 'ignored')).toEqual({ label: null, tasks: ['a-1'] })
  })
})
