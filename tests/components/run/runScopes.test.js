import { describe, expect, it } from 'vitest'
import { sameScope, scopeBusyReason, scopeLabel } from '../../../src/components/run/runScopes.js'

const run = (scope, stateKind = 'working') => ({
  token: 1,
  settings: { scope },
  state: { kind: stateKind }
})

describe('what "the same run" means', () => {
  it('two queue runs are the same run, whatever else they chose', () => {
    expect(sameScope({ kind: 'queue' }, { kind: 'queue' })).toBe(true)
  })

  it('a named scope is the same only under the same kind and id', () => {
    expect(sameScope({ kind: 'task', id: 'a' }, { kind: 'task', id: 'a' })).toBe(true)
    expect(sameScope({ kind: 'task', id: 'a' }, { kind: 'task', id: 'b' })).toBe(false)
    expect(sameScope({ kind: 'epic', id: 'x' }, { kind: 'epic', id: 'x' })).toBe(true)
    expect(sameScope({ kind: 'epic', id: 'x' }, { kind: 'epic', id: 'y' })).toBe(false)
  })

  it('different kinds divide the board rather than collide', () => {
    // A queue run beside a task run is the whole point of a project holding
    // several; even the same id under different kinds is two different runs.
    expect(sameScope({ kind: 'queue' }, { kind: 'task', id: 'a' })).toBe(false)
    expect(sameScope({ kind: 'task', id: 'a' }, { kind: 'epic', id: 'a' })).toBe(false)
  })

  it('nothing is the same as a scope that is not there', () => {
    expect(sameScope(null, { kind: 'queue' })).toBe(false)
    expect(sameScope({ kind: 'queue' }, undefined)).toBe(false)
  })

  it('the dialog\'s scope shape and the run\'s meet without ceremony', () => {
    // The dialog carries a title beside the id; only kind and id take part.
    expect(sameScope({ kind: 'task', id: 'a', title: 'Fix it' }, { kind: 'task', id: 'a' })).toBe(true)
  })
})

describe('the words a scope goes by', () => {
  it('matches the worker\'s own vocabulary', () => {
    // RunScope::describe in runs/model.rs composes the same fragments, so the
    // refusal in the dialog and the grey on the play read as one voice.
    expect(scopeLabel({ kind: 'queue' })).toBe('the queue')
    expect(scopeLabel({ kind: 'task', id: 'smetana-1' })).toBe('task smetana-1')
    expect(scopeLabel({ kind: 'epic', id: 'smetana-2' })).toBe('epic smetana-2')
  })
})

describe('why a play is inactive', () => {
  it('a live run of the same scope blocks it, in words that name the scope', () => {
    expect(scopeBusyReason({ kind: 'queue' }, [run({ kind: 'queue' })])).toBe(
      'a run over the queue is already going'
    )
    expect(
      scopeBusyReason({ kind: 'task', id: 'a' }, [run({ kind: 'task', id: 'a' })])
    ).toBe('a run over task a is already going')
  })

  it('another scope\'s run blocks nothing here', () => {
    expect(scopeBusyReason({ kind: 'queue' }, [run({ kind: 'task', id: 'a' })])).toBe('')
    expect(scopeBusyReason({ kind: 'task', id: 'b' }, [run({ kind: 'task', id: 'a' })])).toBe('')
  })

  it('a stopped run is a reason to read the bar, not a reason to refuse', () => {
    expect(scopeBusyReason({ kind: 'queue' }, [run({ kind: 'queue' }, 'stopped')])).toBe('')
  })

  it('an ending this front end has never heard of is still an ending', () => {
    // The rule `running` used to carry for the whole project, kept per scope:
    // an unknown stop reason must not leave a play greyed over a run that is
    // over. The reason lives under state.reason; the state kind is what says
    // "over".
    const over = { token: 1, settings: { scope: { kind: 'queue' } }, state: { kind: 'stopped', reason: { kind: 'ran_out_of_tea' } } }
    expect(scopeBusyReason({ kind: 'queue' }, [over])).toBe('')
  })

  it('a state kind it has never heard of reads as live, the lenient direction', () => {
    // The worker would refuse the duplicate anyway, with the same sentence —
    // greying the play early is the courtesy, so an unknown state errs toward
    // the refusal that is true at the worker.
    expect(scopeBusyReason({ kind: 'queue' }, [run({ kind: 'queue' }, 'somethingLater')])).not.toBe('')
  })

  it('no runs, or none at all, is an open board', () => {
    expect(scopeBusyReason({ kind: 'queue' }, [])).toBe('')
    expect(scopeBusyReason({ kind: 'queue' }, null)).toBe('')
  })
})
