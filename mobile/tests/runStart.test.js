import { describe, expect, it } from 'vitest'
import {
  MODES,
  PARALLEL,
  canRun,
  hasPause,
  modeLabel,
  openForm,
  pauseErrors,
  runPayload,
  runSettings,
  startBlock
} from '../src/runStart.js'

const branches = [
  { name: 'develop', missing_in: [] },
  { name: 'main', missing_in: [] },
  { name: 'release', missing_in: ['web'] }
]
const config = {
  state: 'ok',
  defaults: { target_branch: 'main', min_priority: 1, max_parallel_tasks: 4 },
  live_check: { mode: 'browser' }
}

describe('whether a run can be started', () => {
  it('can when the project is set up', () => {
    expect(startBlock(config)).toBe('')
  })

  it('says the project has to be set up on the desktop when it has no configuration', () => {
    expect(startBlock({ state: 'missing' })).toMatch(/not set up/)
  })

  it('carries the parser\'s own words when the file will not parse', () => {
    expect(startBlock({ state: 'broken', message: 'bad toml at line 3' })).toMatch(/bad toml at line 3/)
    expect(startBlock({ state: 'broken' })).toMatch(/will not parse/)
  })

  it('does not start on a configuration that has not been read', () => {
    expect(startBlock(undefined)).not.toBe('')
    expect(startBlock({ state: 'from-the-future' })).not.toBe('')
  })
})

const sources = (over = {}) => ({ config, branches, remembered: null, ...over })
const payloadOf = (form, over = {}) => runPayload(form, { config, branches, ...over })

describe('the scope a run is started over', () => {
  it('is the queue unless a card asked for itself', () => {
    const form = openForm(sources())
    expect(payloadOf(form).scope).toEqual({ kind: 'queue' })
    expect(runPayload(form, { config, branches }, { kind: 'task', id: 'x-1' }).scope).toEqual({ kind: 'task', id: 'x-1' })
    expect(runPayload(form, { config, branches }, { kind: 'epic', id: 'x-2' }).scope).toEqual({ kind: 'epic', id: 'x-2' })
  })
})

describe('the form when it opens', () => {
  it('holds the dialog\'s own opening values for a project nobody has run before', () => {
    expect(openForm(sources())).toEqual({
      branch: 'main',
      mode: 'auto',
      parallel: 4,
      pauseMin: 10,
      pauseMax: 30,
      priority: 1,
      liveCheck: true,
      fileFindings: true,
      notes: ''
    })
  })

  it('falls back to the dialog\'s own numbers where the configuration says nothing', () => {
    const form = openForm({ config: { state: 'ok' }, branches, remembered: null })
    expect(form).toMatchObject({ parallel: 3, priority: 2, branch: 'develop' })
  })

  it('takes what the project was left at from the remembered settings', () => {
    const remembered = {
      targetBranch: 'develop',
      mode: 'supervised',
      minPriority: 3,
      batchPauseMin: 5,
      batchPauseMax: 15,
      liveCheck: false,
      fileFindings: false
    }
    expect(openForm(sources({ remembered }))).toEqual({
      branch: 'develop',
      mode: 'supervised',
      parallel: 4,
      pauseMin: 5,
      pauseMax: 15,
      priority: 3,
      liveCheck: false,
      fileFindings: false,
      notes: ''
    })
  })

  it('takes the floor from the configuration when nothing is remembered, and a remembered 0 over it', () => {
    expect(openForm(sources()).priority).toBe(1)
    expect(openForm(sources({ remembered: { minPriority: 0 } })).priority).toBe(0)
  })

  it('skips a remembered branch that has gone, then the configured one, then the first of the list', () => {
    expect(openForm(sources({ remembered: { targetBranch: 'gone' } })).branch).toBe('main')
    const noDefault = { ...config, defaults: { ...config.defaults, target_branch: undefined } }
    expect(openForm(sources({ config: noDefault, remembered: { targetBranch: 'gone' } })).branch).toBe('develop')
  })

  it('takes the number of tasks at once from the configuration every time, within the dialog\'s range', () => {
    const wide = { ...config, defaults: { ...config.defaults, max_parallel_tasks: 40 } }
    expect(openForm(sources({ config: wide })).parallel).toBe(8)
    const none = { ...config, defaults: { ...config.defaults, max_parallel_tasks: 0 } }
    expect(openForm(sources({ config: none })).parallel).toBe(1)
    expect(openForm(sources({ config: wide, remembered: { parallel: 2, maxParallelTasks: 2 } })).parallel).toBe(8)
  })

  it('turns a remembered Solo into Autopilot, and does not offer Solo', () => {
    expect(openForm(sources({ remembered: { mode: 'solo' } })).mode).toBe('auto')
    expect(MODES.map((m) => m.label)).toEqual(['Autopilot', 'Crew'])
  })

  it('offers One at a time and then N at once up to eight', () => {
    expect(PARALLEL.map((p) => p.label)).toEqual([
      'One at a time', '2 at once', '3 at once', '4 at once', '5 at once', '6 at once', '7 at once', '8 at once'
    ])
  })

  it('switches the live check off where the project declares none, remembered or not', () => {
    const none = { ...config, live_check: { mode: 'none' } }
    expect(openForm(sources({ config: none, remembered: { liveCheck: true } })).liveCheck).toBe(false)
  })

  it('opens with empty notes', () => {
    expect(openForm(sources()).notes).toBe('')
  })
})

describe('the payload of Run', () => {
  it('is the dialog\'s for a form left as it opened', () => {
    expect(runSettings(sources())).toEqual({
      scope: { kind: 'queue' },
      mode: 'auto',
      target_branch: 'main',
      create_target: false,
      min_priority: 1,
      max_parallel_tasks: 4,
      batch_pause_min: 10,
      batch_pause_max: 30,
      live_check: true,
      file_findings: true,
      instructions: null
    })
  })

  it('carries each field as it stands, the selects\' text made into numbers', () => {
    const form = {
      ...openForm(sources()),
      branch: 'develop',
      mode: 'supervised',
      parallel: '6',
      priority: '3',
      liveCheck: false,
      fileFindings: false,
      notes: '  do not commit  '
    }
    expect(payloadOf(form)).toEqual({
      scope: { kind: 'queue' },
      mode: 'supervised',
      target_branch: 'develop',
      create_target: false,
      min_priority: 3,
      max_parallel_tasks: 6,
      batch_pause_min: null,
      batch_pause_max: null,
      live_check: false,
      file_findings: false,
      instructions: 'do not commit'
    })
  })

  it('has a pause between batches in Autopilot only', () => {
    const form = { ...openForm(sources()), pauseMin: '5', pauseMax: '15' }
    expect(hasPause(form)).toBe(true)
    expect(payloadOf(form)).toMatchObject({ batch_pause_min: 5, batch_pause_max: 15 })
    const crew = { ...form, mode: 'supervised' }
    expect(hasPause(crew)).toBe(false)
    expect(payloadOf(crew)).toMatchObject({ batch_pause_min: null, batch_pause_max: null })
  })

  it('refuses a pause that runs backwards, and says why, in Autopilot only', () => {
    const form = { ...openForm(sources()), pauseMin: '30', pauseMax: '10' }
    expect(pauseErrors(form).max).toMatch(/Not less than/)
    expect(canRun(form)).toBe(false)
    expect(payloadOf(form)).toBeNull()
    const crew = { ...form, mode: 'supervised' }
    expect(pauseErrors(crew)).toEqual({})
    expect(canRun(crew)).toBe(true)
  })

  it('may cut a branch that is missing from some repository, or from the list', () => {
    expect(payloadOf({ ...openForm(sources()), branch: 'release' }).create_target).toBe(true)
    expect(payloadOf({ ...openForm(sources()), branch: 'new-one' }).create_target).toBe(true)
  })

  it('has no run to start without a branch', () => {
    expect(runSettings(sources({ branches: [] }))).toBeNull()
  })

  it('has no live check where the project declares none, whatever the switch holds', () => {
    const none = { ...config, live_check: { mode: 'none' } }
    const form = { ...openForm(sources({ config: none })), liveCheck: true }
    expect(payloadOf(form, { config: none }).live_check).toBe(false)
  })

  it('sends empty or blank notes as null', () => {
    expect(payloadOf({ ...openForm(sources()), notes: '' }).instructions).toBeNull()
    expect(payloadOf({ ...openForm(sources()), notes: '  \n ' }).instructions).toBeNull()
  })
})

describe('the names of the modes', () => {
  it('are the desktop\'s', () => {
    expect(modeLabel('auto')).toBe('Autopilot')
    expect(modeLabel('supervised')).toBe('Crew')
    expect(modeLabel('solo')).toBe('Solo')
  })
})
