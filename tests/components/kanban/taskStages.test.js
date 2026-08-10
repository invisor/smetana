import { describe, expect, it } from 'vitest'
import {
  cascade,
  DEFAULT_STAGE,
  STAGES,
  stageUnder
} from '../../../src/components/kanban/taskStages.js'

const POSITIONS = ['auto', 'on', 'off']

describe('stageUnder', () => {
  it('hands the choice over only under an On parent', () => {
    expect(stageUnder('on', 'off')).toEqual({ value: 'off', interactive: true })
  })

  it('reads as its parent everywhere else, rather than as a placeholder', () => {
    // The screen has to state what will be sent: a greyed control reading
    // "Auto" under a settled Off would claim there was still a judgement to
    // make.
    expect(stageUnder('off', 'on')).toEqual({ value: 'off', interactive: false })
    expect(stageUnder('auto', 'on')).toEqual({ value: 'auto', interactive: false })
  })
})

describe('cascade', () => {
  /* All nine combinations of the two parent positions, against what each child
     shows and whether it is interactive. The child's own remembered choice is
     the third position each time, so the table also pins that a stale choice
     under a settled parent is never what reaches the prompt. */
  const expected = {
    // Brainstorming Auto: nothing below it is anybody's to choose.
    'auto/auto': { spec: ['auto', false], plan: ['auto', false] },
    'auto/on': { spec: ['auto', false], plan: ['auto', false] },
    'auto/off': { spec: ['auto', false], plan: ['auto', false] },
    // Brainstorming On: Spec is live, and Plan follows what Spec shows.
    'on/auto': { spec: ['auto', true], plan: ['auto', false] },
    'on/on': { spec: ['on', true], plan: ['on', true] },
    'on/off': { spec: ['off', true], plan: ['off', false] },
    // Brainstorming Off: no discussion, so no design and no plan.
    'off/auto': { spec: ['off', false], plan: ['off', false] },
    'off/on': { spec: ['off', false], plan: ['off', false] },
    'off/off': { spec: ['off', false], plan: ['off', false] }
  }

  for (const [combination, want] of Object.entries(expected)) {
    const [brainstorm, spec] = combination.split('/')
    it(`draws Spec and Plan for brainstorming ${brainstorm} over spec ${spec}`, () => {
      // The plan's own remembered choice is On throughout: wherever the table
      // says the plan is not interactive, that On is exactly what must not
      // survive.
      const got = cascade(brainstorm, spec, 'on')
      expect(got.spec).toEqual({ value: want.spec[0], interactive: want.spec[1] })
      expect(got.plan).toEqual({ value: want.plan[0], interactive: want.plan[1] })
    })
  }

  it('lets the plan be chosen only with both stages above it On', () => {
    expect(cascade('on', 'on', 'off').plan).toEqual({ value: 'off', interactive: true })
    expect(cascade('on', 'on', 'auto').plan).toEqual({ value: 'auto', interactive: true })
  })
})

describe('the vocabulary', () => {
  it('offers exactly Auto, On and Off, in that order', () => {
    expect(STAGES.map((option) => option.value)).toEqual(POSITIONS)
    expect(STAGES.map((option) => option.label)).toEqual(['Auto', 'On', 'Off'])
  })

  it('opens on a position the vocabulary actually has', () => {
    expect(POSITIONS).toContain(DEFAULT_STAGE)
  })
})
