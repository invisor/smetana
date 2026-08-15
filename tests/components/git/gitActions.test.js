import { describe, expect, it } from 'vitest'
import { gitActions } from '../../../src/components/git/gitActions.js'

/* A run as `runs.js` holds it: whole objects from the worker, keyed by token,
   never unpacked into flags. */
const live = { token: 1, state: { kind: 'running' } }
const stopped = { token: 2, state: { kind: 'stopped' } }

describe('when the panel may write to a repository', () => {
  it('is live with no run going', () => {
    expect(gitActions([]).allowed).toBe(true)
    expect(gitActions([stopped]).allowed).toBe(true)
  })

  /* Nothing to explain while nothing is refused: a reason beside an allowed
     verdict is what ends up in a tooltip over a live control. */
  it('says nothing when it allows', () => {
    expect(gitActions([stopped]).reason).toBe(null)
  })

  /* A checkout under a batch that is mid-merge is a night's work lost to one
     click, and unlike a person's own session a run is nobody's foreground. */
  it('is refused while a run is going, and says so', () => {
    const verdict = gitActions([live])
    expect(verdict.allowed).toBe(false)
    expect(verdict.reason).toMatch(/run/i)
  })

  /* A state this front end has not heard of must not silently read as one it
     has — the instinct tracker.js follows with bd's statuses. An unknown state
     is treated as live, which costs a disabled button rather than a lost run. */
  it('an unrecognised run state is treated as going', () => {
    expect(gitActions([{ token: 3, state: { kind: 'whatever-comes-next' } }]).allowed).toBe(false)
  })

  /* The same rule one step further down: a run with no state at all, or one
     whose state is not an object, is a run this front end cannot read — which
     is exactly the case the unknown-state rule exists for. */
  it('a run whose state cannot be read at all is treated as going', () => {
    expect(gitActions([{ token: 4 }]).allowed).toBe(false)
    expect(gitActions([{ token: 5, state: null }]).allowed).toBe(false)
    expect(gitActions([{ token: 6, state: 'stopped' }]).allowed).toBe(false)
    expect(gitActions([null]).allowed).toBe(false)
  })

  /* The list itself may not have arrived — `runs.js` fills it on mount, on a
     project switch and on window focus. Nothing known to be going is not the
     same fact as a live run, and blocking on it would leave the panel dead
     until the first read lands. */
  it('an absent list is not a run', () => {
    expect(gitActions(undefined).allowed).toBe(true)
    expect(gitActions(null).allowed).toBe(true)
  })

  /* A stopped run beside a live one changes nothing: the bar keeps a stopped
     run on screen until the project changes, so this pairing is the ordinary
     state of a project somebody has run twice. */
  it('one live run among stopped ones still refuses', () => {
    expect(gitActions([stopped, live]).allowed).toBe(false)
  })

  /* The sentence is about runs and never about the agent sessions a person
     started themselves: those are somebody's foreground, watched in the tab
     next door, and a panel dead for as long as a terminal is open would be
     dead most of the time. Nothing but the runs list reaches this rule, which
     is what makes that true by construction — this pins the wording that says
     so to a person. */
  it('names the run and what a checkout under one would cost', () => {
    const reason = gitActions([live]).reason
    expect(reason).toMatch(/^A run is going in this project\./)
    expect(reason).toMatch(/merge/i)
  })

  /* Two leads working the same board at once is an ordinary night now that a
     project holds several runs, and "a run" over three of them would read as
     though stopping one would free the panel. */
  it('counts the runs when there is more than one', () => {
    const reason = gitActions([live, { token: 7, state: { kind: 'paused' } }]).reason
    expect(reason).toMatch(/^2 runs are going in this project\./)
  })
})
