import { describe, expect, it } from 'vitest'
import { describeStart, modeLabel, runSettings, startBlock } from '../src/runStart.js'

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

describe('the settings a run is started with', () => {
  it('are the ones the desktop\'s run window opens on, for a project nobody has run before', () => {
    expect(runSettings({ config, branches, remembered: null })).toEqual({
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

  it('run into what the project was left at, ahead of its configured default', () => {
    const settings = runSettings({ config, branches, remembered: { targetBranch: 'develop' } })
    expect(settings.target_branch).toBe('develop')
  })

  it('skips a remembered branch that has gone, then the configured one, then the first of the list', () => {
    expect(runSettings({ config, branches, remembered: { targetBranch: 'gone' } }).target_branch).toBe('main')
    const noDefault = { ...config, defaults: { ...config.defaults, target_branch: undefined } }
    expect(runSettings({ config: noDefault, branches, remembered: { targetBranch: 'gone' } }).target_branch).toBe('develop')
  })

  it('takes the person\'s own pick over all of that', () => {
    expect(runSettings({ config, branches, remembered: { targetBranch: 'develop' }, branch: 'main' }).target_branch).toBe('main')
  })

  it('ignores a pick that is not in the list', () => {
    expect(runSettings({ config, branches, remembered: null, branch: 'nowhere' }).target_branch).toBe('main')
  })

  it('may cut a branch that is missing from some repository', () => {
    const settings = runSettings({ config, branches, remembered: null, branch: 'release' })
    expect(settings.create_target).toBe(true)
  })

  it('has no run to start without a branch', () => {
    expect(runSettings({ config, branches: [], remembered: null })).toBeNull()
  })

  it('keeps what was chosen last for the floor, the mode and the switches', () => {
    const settings = runSettings({
      config,
      branches,
      remembered: { mode: 'supervised', minPriority: 3, liveCheck: false, fileFindings: false }
    })
    expect(settings).toMatchObject({ mode: 'supervised', min_priority: 3, live_check: false, file_findings: false })
  })

  it('turns a remembered Solo into Autopilot, since a queue is not one task', () => {
    expect(runSettings({ config, branches, remembered: { mode: 'solo' } }).mode).toBe('auto')
  })

  it('has a pause between batches for Autopilot only', () => {
    const remembered = { batchPauseMin: 5, batchPauseMax: 15 }
    expect(runSettings({ config, branches, remembered })).toMatchObject({ batch_pause_min: 5, batch_pause_max: 15 })
    expect(runSettings({ config, branches, remembered: { ...remembered, mode: 'supervised' } })).toMatchObject({
      batch_pause_min: null,
      batch_pause_max: null
    })
  })

  it('takes the number of tasks at once from the config every time, within the dialog\'s range', () => {
    const wide = { ...config, defaults: { ...config.defaults, max_parallel_tasks: 40 } }
    expect(runSettings({ config: wide, branches, remembered: null }).max_parallel_tasks).toBe(8)
    const none = { ...config, defaults: { ...config.defaults, max_parallel_tasks: 0 } }
    expect(runSettings({ config: none, branches, remembered: null }).max_parallel_tasks).toBe(1)
    const bare = { state: 'ok' }
    expect(runSettings({ config: bare, branches, remembered: null }).max_parallel_tasks).toBe(3)
  })

  it('has no live check where the project declares none', () => {
    const none = { ...config, live_check: { mode: 'none' } }
    expect(runSettings({ config: none, branches, remembered: { liveCheck: true } }).live_check).toBe(false)
  })

  it('carries no notes: they are for one run alone', () => {
    expect(runSettings({ config, branches, remembered: null }).instructions).toBeNull()
  })
})

describe('the sentence under the press', () => {
  it('says what is about to start', () => {
    const settings = runSettings({ config, branches, remembered: null })
    expect(describeStart(settings, 5)).toBe('Autopilot into main, 4 tasks at once, 5 tasks are ready')
    expect(describeStart({ ...settings, max_parallel_tasks: 1 }, 1)).toBe(
      'Autopilot into main, one task at a time, one task is ready'
    )
  })

  it('does not claim a count it was not given', () => {
    const settings = runSettings({ config, branches, remembered: null })
    expect(describeStart(settings, undefined)).toBe('Autopilot into main, 4 tasks at once')
  })

  it('names the modes the way the desktop does', () => {
    expect(modeLabel('auto')).toBe('Autopilot')
    expect(modeLabel('supervised')).toBe('Crew')
    expect(modeLabel('solo')).toBe('Solo')
  })
})
