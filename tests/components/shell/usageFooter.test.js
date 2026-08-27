import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import {
  usageAgentLabel,
  usageSegments,
  usageTooltip
} from '../../../src/components/shell/usageFooter.js'

/* A reading in `runs::usage::AgentUsage`'s own shape, the way the command
   answers it. The numbers and the reset strings are `claude.rs`'s fixture
   output, so what a test asserts is what a person would see. */
const reading = (usage, band = 'reduced') => ({ state: 'read', agent: 'claude', usage, band })

const BOTH = {
  sessionPct: 10,
  sessionReset: 'Aug 7 at 8pm (Europe/Moscow)',
  weekPct: 78,
  weekReset: 'Aug 11 at 5:59pm (Europe/Moscow)'
}

describe('usageAgentLabel', () => {
  it('names whoever answered the probe, not whoever is selected in settings', () => {
    expect(usageAgentLabel(reading(BOTH))).toBe('Claude Code')
    expect(usageAgentLabel({ state: 'unsupported', agent: 'codex' })).toBe('Codex')
  })

  /* An id this build has no label for is a hand-edited file or a Rust newer
     than this bundle. It is drawn as it stands: dressing it up as one of ours
     would put a name over another agent's allowance, and dropping it would
     leave the strip claiming there was nobody to ask. */
  it('draws an id it has no label for as it stands', () => {
    expect(usageAgentLabel({ state: 'read', agent: 'aider', usage: BOTH, band: 'normal' })).toBe('aider')
  })

  /* Nothing read yet, and no agent on this machine, are the two answers with
     nobody to name. Both take the bare word rather than borrowing the selected
     agent's label, which would be the app claiming a reading it has not got. */
  it('says the bare word when there is nobody to name', () => {
    expect(usageAgentLabel(null)).toBe('Agent')
    expect(usageAgentLabel({ state: 'unsupported', agent: null })).toBe('Agent')
    expect(usageAgentLabel({ state: 'moonshot' })).toBe('Agent')
  })
})

describe('usageSegments', () => {
  it('draws both halves of a reading, session first', () => {
    expect(usageSegments(reading(BOTH))).toEqual([
      { name: 'Session', value: '10%' },
      { name: 'Week', value: '78%' }
    ])
  })

  /* Either line the harness prints can go missing — one of them reworded, a
     build that prints the other alone — and the half that was read still has to
     be shown. Refusing the pair over it would throw away a reading that is
     perfectly good (smetana-7rp). */
  it('dashes the half that was not read and draws the half that was', () => {
    expect(usageSegments(reading({ sessionPct: 92, sessionReset: null, weekPct: null, weekReset: null })))
      .toEqual([
        { name: 'Session', value: '92%' },
        { name: 'Week', value: '—' }
      ])
    expect(usageSegments(reading({ sessionPct: null, sessionReset: null, weekPct: 4, weekReset: null })))
      .toEqual([
        { name: 'Session', value: '—' },
        { name: 'Week', value: '4%' }
      ])
  })

  /* The other direction of the same rule, and the one that is easy to get
     backwards: a fresh week really does print zero, and zero is a number. */
  it('draws a real zero as a percentage and never as a dash', () => {
    expect(usageSegments(reading({ sessionPct: 0, sessionReset: null, weekPct: 0, weekReset: null })))
      .toEqual([
        { name: 'Session', value: '0%' },
        { name: 'Week', value: '0%' }
      ])
  })

  const dashes = [
    { name: 'Session', value: '—' },
    { name: 'Week', value: '—' }
  ]

  /* Everything that is not a reading draws the same two dashes, and the strip
     stays where it is: hiding it would move the height of the working area on
     even ground, and a person who never sees the strip cannot discover that the
     app reads this at all. */
  it('draws two dashes for everything that is not a reading', () => {
    expect(usageSegments(null)).toEqual(dashes)
    expect(usageSegments({ state: 'unsupported', agent: 'codex' })).toEqual(dashes)
    expect(usageSegments({ state: 'unsupported', agent: null })).toEqual(dashes)
    expect(usageSegments({ state: 'unreadable', agent: 'claude' })).toEqual(dashes)
    expect(usageSegments(reading({ sessionPct: null, weekPct: null }))).toEqual(dashes)
  })

  /* A state this build has never heard of reads as "nothing was read" rather
     than silently as one it does know — the safe direction, since two dashes
     promise nothing about the allowance. Note the numbers are deliberately
     there and deliberately ignored. */
  it('draws two dashes for a state this build has never heard of', () => {
    expect(usageSegments({ state: 'throttled', agent: 'claude', usage: BOTH })).toEqual(dashes)
  })
})

describe('usageTooltip', () => {
  it('carries both reset strings verbatim, then the sentence about the band', () => {
    expect(usageTooltip(reading(BOTH))).toBe(
      'Session resets Aug 7 at 8pm (Europe/Moscow) · '
        + 'Week resets Aug 11 at 5:59pm (Europe/Moscow) · '
        + 'A run would take fewer tasks per batch at this level.'
    )
  })

  /* A fresh allowance prints no reset at all, which is an ordinary reading and
     not a gap — inventing one would be worse than the missing half. */
  it('leaves out a reset the harness did not print', () => {
    expect(usageTooltip(reading({ sessionPct: 0, sessionReset: null, weekPct: 3, weekReset: 'Aug 11 at 5:59pm' }, 'normal')))
      .toBe('Week resets Aug 11 at 5:59pm · A run would take a full batch at this level.')
  })

  /* The band is Rust's word, and a band this build has never heard of says
     nothing about a run rather than guessing which of the three it meant. The
     percentages are unaffected — they are the part that does not depend on
     knowing. */
  it('offers no sentence about a run for a band it has never heard of', () => {
    const odd = reading(BOTH, 'throttled')
    expect(usageTooltip(odd)).toBe(
      'Session resets Aug 7 at 8pm (Europe/Moscow) · Week resets Aug 11 at 5:59pm (Europe/Moscow)'
    )
    expect(usageSegments(odd)).toEqual([
      { name: 'Session', value: '10%' },
      { name: 'Week', value: '78%' }
    ])
  })

  /* Empty is a real answer, and the component has to be ready for it: a reading
     in a band this build cannot name, printing no reset times, leaves nothing
     true to say. */
  it('is empty when an unknown band prints no reset times either', () => {
    expect(usageTooltip(reading({ sessionPct: 10, weekPct: 78 }, 'throttled'))).toBe('')
  })

  /* The two states with nothing to read differ in what a person can do about
     them, so they differ in what is said. */
  it('says why there is nothing to read, in the vocabulary the settings block uses', () => {
    expect(usageTooltip({ state: 'unsupported', agent: 'codex' }))
      .toMatch(/does not report/)
    expect(usageTooltip({ state: 'unsupported', agent: null }))
      .toMatch(/No agent is installed/)
    expect(usageTooltip(null)).toMatch(/has not been read yet/)
    expect(usageTooltip({ state: 'unreadable', agent: 'claude' }))
      .toMatch(/could not be read/)
    expect(usageTooltip({ state: 'throttled', agent: 'claude' }))
      .toMatch(/could not be read/)
    expect(usageTooltip(reading({ sessionPct: null, weekPct: null })))
      .toMatch(/could not be read/)
  })

  /* `invoke` refusing is the channel rather than an answer — the command is
     infallible in Rust — so it is a fifth reason there is nothing on the strip,
     and the one the caller has no other line for. What it must not draw is the
     sentence for an attempt that never happened: the reading is cleared by a
     refusal, and "not read yet" over one that happened and failed sends
     somebody looking in the wrong place. */
  it('says the reading failed, not that nobody has asked, when invoke refuses', () => {
    const tip = usageTooltip(null, false, 'the worker is not answering')
    expect(tip).toBe('The allowance could not be read: the worker is not answering')
    expect(tip).not.toMatch(/has not been read yet/)
  })

  /* The refusal beats a reading still on screen and beats `busy` with it, the
     way it does in the settings block: one attempt is described once, and the
     refusal is the account of it. */
  it('lets a refusal beat everything else that would have been said', () => {
    const tip = usageTooltip(reading(BOTH), true, 'the worker is not answering')
    expect(tip).toMatch(/The allowance could not be read: the worker is not answering$/)
    expect(tip).not.toMatch(/Reading what is left/)
    expect(tip).not.toMatch(/fewer tasks per batch/)
  })

  /* A probe on its way is admitted to in the hint and nowhere else: the numbers
     on the strip stay where they are, which is the one deliberate difference
     from the settings block. */
  it('says a reading is under way while a probe is out, and keeps the reset strings', () => {
    const tip = usageTooltip(reading(BOTH), true)
    expect(tip).toMatch(/Session resets Aug 7 at 8pm/)
    expect(tip).toMatch(/Reading what is left of the allowance/)
    expect(tip).not.toMatch(/fewer tasks per batch/)
  })
})

/* The thresholds behind the band are decided in `src-tauri/src/runs/usage.rs`
   and travel in the answer as a name. A second copy of them in the front end
   would be one that drifts from the first with nothing on screen to say it
   has. */
describe('the module', () => {
  it('spells no threshold anywhere', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/components/shell/usageFooter.js'), 'utf8')
    expect(source).not.toMatch(/\b75\b/)
    expect(source).not.toMatch(/\b90\b/)
  })
})
