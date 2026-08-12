import { describe, expect, it } from 'vitest'
import { workingKey } from '../../../src/components/run/configFreshness.js'

const session = (id, state, project = '/a') => ({ id, state, project })

describe('when the run configuration may have changed', () => {
  it('a working session is in the key and a settled one is not', () => {
    expect(workingKey([session(1, 'running')], '/a')).not.toBe(
      workingKey([session(1, 'idle')], '/a')
    )
    // Every way of stopping is the same way as far as the file is concerned.
    for (const state of ['idle', 'needs-you', 'exited']) {
      expect(workingKey([session(1, state)], '/a')).toBe(workingKey([], '/a'))
    }
  })

  it('starting counts as working, so coming up is not read as having stopped', () => {
    expect(workingKey([session(1, 'starting')], '/a')).toBe(
      workingKey([session(1, 'running')], '/a')
    )
    expect(workingKey([session(1, 'starting')], '/a')).not.toBe(workingKey([], '/a'))
  })

  it('a session leaving the list moves the key the same as one that exits', () => {
    const working = workingKey([session(1, 'running')], '/a')
    // Removed by hand, or dropped when loadSessions replaces the array.
    expect(workingKey([], '/a')).not.toBe(working)
    expect(workingKey([session(1, 'exited')], '/a')).toBe(workingKey([], '/a'))
  })

  it('a state this front end has never heard of reads as not working', () => {
    // Erring toward one more read of a small file, never toward a mark that
    // will not clear.
    expect(workingKey([session(1, 'hibernating')], '/a')).toBe(workingKey([], '/a'))
  })

  it('another project cannot move this project key', () => {
    const before = workingKey([session(1, 'running', '/a')], '/a')
    const after = workingKey([session(1, 'running', '/a'), session(2, 'running', '/b')], '/a')
    expect(after).toBe(before)
    // And the other way: a setup finishing in B is B key that moves.
    expect(workingKey([session(2, 'running', '/b')], '/b')).not.toBe(
      workingKey([session(2, 'idle', '/b')], '/b')
    )
  })

  it('the same sessions in a different order are the same key', () => {
    // loadSessions and the state events fill the array from different ends;
    // a reordering that changed the key would re-read the file for nothing.
    expect(workingKey([session(1, 'running'), session(2, 'running')], '/a')).toBe(
      workingKey([session(2, 'running'), session(1, 'running')], '/a')
    )
  })

  it('one session settling among several still moves the key', () => {
    const both = [session(1, 'running'), session(2, 'running')]
    const one = [session(1, 'running'), session(2, 'idle')]
    expect(workingKey(one, '/a')).not.toBe(workingKey(both, '/a'))
  })

  it('with no project there is nothing to be fresh about', () => {
    expect(workingKey([session(1, 'running')], null)).toBe('')
    expect(workingKey(null, '/a')).toBe('')
    expect(workingKey(undefined, null)).toBe('')
  })
})
