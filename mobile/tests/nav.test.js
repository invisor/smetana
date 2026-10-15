import { describe, expect, it } from 'vitest'
import { START, back, canGoBack, current, open, parseTarget, routeForTarget, selectTab } from '../src/nav.js'

describe('moving between screens', () => {
  it('starts on the board with nothing opened over it', () => {
    expect(current(START)).toEqual({ screen: 'board' })
    expect(canGoBack(START)).toBe(false)
  })

  it('opens a screen over the tab and goes back to the tab', () => {
    const opened = open(START, { screen: 'task', id: 'bd-1' })
    expect(current(opened)).toEqual({ screen: 'task', id: 'bd-1' })
    expect(canGoBack(opened)).toBe(true)
    expect(back(opened)).toEqual(START)
  })

  it('goes back one screen at a time', () => {
    const two = open(open(START, { screen: 'task', id: 'bd-1' }), { screen: 'task', id: 'bd-2' })
    expect(current(back(two))).toEqual({ screen: 'task', id: 'bd-1' })
  })

  it('does not go back from a tab', () => {
    expect(back(START)).toBe(START)
  })

  it('a tab pressed again returns to its own screen', () => {
    const opened = open(selectTab(START, 'agents'), { screen: 'conversation', id: 1, driven: true })
    expect(current(selectTab(opened, 'agents'))).toEqual({ screen: 'agents' })
  })

  it('an unknown tab changes nothing', () => {
    expect(selectTab(START, 'settings')).toBe(START)
  })

  it('never changes the route it was given', () => {
    const before = JSON.stringify(START)
    open(START, { screen: 'task', id: 'x' })
    selectTab(START, 'runs')
    expect(JSON.stringify(START)).toBe(before)
  })

  it('the project a notification named does not outlive the Runs tab', () => {
    const runs = { tab: 'runs', project: '/work/a', stack: [] }
    expect(selectTab(runs, 'runs').project).toBe('/work/a')
    expect(selectTab(runs, 'board').project).toBeNull()
  })
})

describe('what a push notification names', () => {
  it('reads a run, with a path that has colons and spaces in it', () => {
    expect(parseTarget('run:/work/smetana')).toEqual({ kind: 'run', project: '/work/smetana' })
    expect(parseTarget('run:C:\\Users\\me\\my project')).toEqual({ kind: 'run', project: 'C:\\Users\\me\\my project' })
  })

  it('reads a driven session and a terminal session as different things', () => {
    expect(parseTarget('session:3')).toEqual({ kind: 'session', id: 3 })
    expect(parseTarget('terminal:3')).toEqual({ kind: 'terminal', id: 3 })
  })

  it.each([
    [''],
    ['run:'],
    ['run'],
    ['session:'],
    ['session:abc'],
    ['session:3x'],
    ['session:-1'],
    ['terminal:'],
    ['terminal:1.5'],
    ['terminal/3'],
    ['agent:3'],
    ['Session:3'],
    [null],
    [undefined],
    [7]
  ])('leaves the app where it is for %j', (target) => {
    expect(parseTarget(target)).toBeNull()
    expect(routeForTarget(target)).toBeNull()
  })

  it('opens the Runs tab on the project a run notification names', () => {
    expect(routeForTarget('run:/work/smetana')).toEqual({ tab: 'runs', project: '/work/smetana', stack: [] })
  })

  it('opens the conversation of a session over the Agents tab', () => {
    const route = routeForTarget('session:7')
    expect(route.tab).toBe('agents')
    expect(current(route)).toEqual({ screen: 'conversation', id: 7, driven: true })
    expect(current(back(route))).toEqual({ screen: 'agents' })
  })

  it('opens the read-only view of a terminal session, never a conversation with the same number', () => {
    const route = routeForTarget('terminal:7')
    expect(route.tab).toBe('agents')
    expect(current(route)).toEqual({ screen: 'conversation', id: 7, driven: false })
  })
})
