import { describe, expect, it } from 'vitest'
import {
  combine,
  diffViews,
  fold,
  mergePatch,
  serverView,
  subtractPatch,
  transportFailure,
  viewFromHalf,
  writeView
} from '../../src/stores/settingsSplit.js'
import { defaults } from '../../src/stores/settings.js'

const store = (over = {}) => ({ ...defaults(), openProjects: ['/a', '/b'], activeProject: '/a', ...over })

describe('the server view', () => {
  it('holds the fields a worker on the server reads and none of the device', () => {
    const view = serverView(store())
    expect(Object.keys(view).sort()).toEqual(
      [
        'agent', 'agentLanguage', 'agentPrompt', 'agentRoles', 'commitLanguage', 'conversationPanel', 'git',
        'lastProject', 'model', 'openProjects', 'projects', 'remote', 'reportLanguage', 'runFailover',
        'subscription', 'taskLanguage'
      ].sort()
    )
    expect(view.git).toEqual({ removeWorktrees: true })
    expect(view.projects).toEqual({ '/a': { runSettings: null, agents: null } })
    expect(JSON.stringify(view)).not.toContain('theme')
  })

  it('is the same for the store and for a snapshot of it', () => {
    const live = store()
    expect(serverView(JSON.parse(JSON.stringify(live)))).toEqual(serverView(live))
  })
})

describe('a patch between two views', () => {
  it('names only what changed', () => {
    const before = serverView(store())
    const after = serverView(store({ agent: 'codex' }))
    after.subscription.pauseAt = 80
    expect(diffViews(before, after)).toEqual({ agent: 'codex', subscription: { pauseAt: 80 } })
  })

  it('is nothing when nothing changed, and null and absent are the same thing', () => {
    const view = serverView(store())
    expect(diffViews(view, serverView(store()))).toBeUndefined()
    const bare = serverView(store())
    delete bare.projects['/a'].agents
    expect(diffViews(view, bare)).toBeUndefined()
  })

  it('replaces a list whole and removes a key that went away', () => {
    const before = serverView(store())
    before.projects['/a'].runSettings = { mode: 'auto', minPriority: 2 }
    const after = serverView(store({ openProjects: ['/a'] }))
    after.projects['/a'].runSettings = { mode: 'auto' }
    expect(diffViews(before, after)).toEqual({
      openProjects: ['/a'],
      projects: { '/a': { runSettings: { minPriority: null } } }
    })
  })

  it('never reads a switch of the open project as the removal of the old one', () => {
    const before = serverView(store())
    const after = serverView(store({ activeProject: '/b' }))
    expect(diffViews(before, after)).toEqual({ lastProject: '/b' })
  })
})

describe('the project open here in the list', () => {
  it('is not an edit when it was only put back after another client took it out', () => {
    const before = serverView(store({ openProjects: ['/b'] }))
    const after = serverView(store({ openProjects: ['/b', '/a'] }))
    expect(diffViews(before, after)).toBeUndefined()
  })

  it('is still an edit when something else about the list changed', () => {
    const before = serverView(store({ openProjects: ['/b'] }))
    const after = serverView(store({ openProjects: ['/b', '/c', '/a'] }))
    expect(diffViews(before, after)).toEqual({ openProjects: ['/b', '/c', '/a'] })
  })
})

describe('a refused patch', () => {
  it('is taken out of what is pending, key by key', () => {
    expect(subtractPatch({ a: 1, b: { c: 2, d: 3 } }, { a: 9, b: { c: 0 } })).toEqual({ b: { d: 3 } })
    expect(subtractPatch({ a: 1 }, { a: 9 })).toBeUndefined()
  })

  it('is told from a connection that was down by the words the shell uses', () => {
    expect(transportFailure('The connection to the server was lost before the request was sent. Nothing was changed.')).toBe(true)
    expect(transportFailure('This app is not paired with the server. Nothing was changed.')).toBe(true)
    expect(transportFailure('The connection to the server was lost before it answered, so the change may or may not have been made.')).toBe(true)
    expect(transportFailure('the settings patch was refused: no')).toBe(false)
    expect(transportFailure('The server does not allow this device to call `settings_patch`.')).toBe(false)
    expect(transportFailure({ kind: 'x' })).toBe(false)
  })
})

describe('merge patch', () => {
  it('merges objects, removes on null and replaces anything else', () => {
    expect(mergePatch({ a: { b: 1, c: 2 }, l: [1, 2], gone: 1 }, { a: { b: 5 }, l: [3], gone: null })).toEqual({
      a: { b: 5, c: 2 },
      l: [3]
    })
  })
})

describe('folding in what the server holds', () => {
  it('keeps a change made here to one field when another client changed another', () => {
    const baseline = serverView(store())
    const current = serverView(store({ agentPrompt: 'be brief' }))
    const incoming = serverView(store({ agent: 'codex' }))
    const { next, baseline: kept, pending } = fold(baseline, current, incoming)
    expect(next.agent).toBe('codex')
    expect(next.agentPrompt).toBe('be brief')
    expect(pending).toEqual({ agentPrompt: 'be brief' })
    expect(kept.agent).toBe('codex')
    expect(kept.agentPrompt).toBe('')
  })

  it('lets this window win on the one field both changed, and sends it', () => {
    const baseline = serverView(store())
    const { next, pending } = fold(baseline, serverView(store({ agent: 'opencode', model: 'x' })), serverView(store({ agent: 'codex' })))
    expect(next.agent).toBe('opencode')
    expect(pending).toEqual({ agent: 'opencode', model: 'x' })
  })

  it('owes nothing when this window had changed nothing', () => {
    const baseline = serverView(store())
    const { pending } = fold(baseline, baseline, serverView(store({ agent: 'codex' })))
    expect(pending).toBeUndefined()
  })

  it('takes back only the fields of the patch the server refused', () => {
    const baseline = serverView(store())
    const current = serverView(store({ agentPrompt: 'typed', model: 'kept' }))
    const { next, pending } = fold(baseline, current, serverView(store()), { drop: { agentPrompt: 'typed' } })
    expect(next.agentPrompt).toBe('')
    expect(next.model).toBe('kept')
    expect(pending).toEqual({ model: 'kept' })
  })

  it('leaves which project is open to this window', () => {
    const baseline = serverView(store())
    const incoming = serverView(store({ activeProject: '/b' }))
    expect(fold(baseline, baseline, incoming).baseline.lastProject).toBe('/a')
  })
})

describe('a broadcast half', () => {
  it('becomes the view for the project open here, and keeps what it lacks', () => {
    const fallback = serverView(store())
    const half = {
      agent: 'codex',
      subscription: { pauseAt: 70 },
      openProjects: ['/a'],
      lastProject: '/b',
      projects: { '/a': { runSettings: { mode: 'solo' }, agents: null, usedAt: 'x' }, '/b': { runSettings: null } }
    }
    const view = viewFromHalf(half, '/a', fallback)
    expect(view.agent).toBe('codex')
    expect(view.subscription).toEqual({ pauseAt: 70, reducedAt: 75 })
    expect(view.agentPrompt).toBe('')
    expect(view.openProjects).toEqual(['/a'])
    expect(view.projects).toEqual({ '/a': { runSettings: { mode: 'solo' }, agents: null } })
  })
})

describe('writing a view into the store', () => {
  it('assigns in place, keeps the open project in the list and leaves the device alone', () => {
    const target = store()
    target.appearance.theme = 'light'
    const roles = target.agentRoles
    const view = serverView(store({ agent: 'codex', openProjects: ['/b'] }))
    view.agentRoles.tasks.agent = 'codex'
    view.lastProject = '/zzz'
    view.projects['/a'].runSettings = { mode: 'solo' }
    writeView(target, view)
    expect(target.agent).toBe('codex')
    expect(target.agentRoles).toBe(roles)
    expect(target.agentRoles.tasks.agent).toBe('codex')
    expect(target.openProjects).toEqual(['/b', '/a'])
    expect(target.activeProject).toBe('/a')
    expect(target.project.runSettings).toEqual({ mode: 'solo' })
    expect(target.appearance.theme).toBe('light')
  })
})

describe('putting the halves together', () => {
  it('takes the device from its file and the server from its answer', () => {
    const device = { ...defaults(), appearance: { theme: 'light', density: 'compact', uiFontSize: 15 }, git: { autoFetch: false, removeWorktrees: true }, agent: 'stale' }
    device.project = { ...defaults().project, selectedTask: 'a-1' }
    const server = {
      ...defaults(),
      agent: 'codex',
      git: { removeWorktrees: false },
      openProjects: ['/a'],
      activeProject: '/a',
      project: { ...defaults().project, runSettings: { mode: 'solo' }, selectedTask: 'not mine' }
    }
    const out = combine(device, server)
    expect(out.appearance.theme).toBe('light')
    expect(out.git).toEqual({ autoFetch: false, removeWorktrees: false })
    expect(out.agent).toBe('codex')
    expect(out.activeProject).toBe('/a')
    expect(out.project.selectedTask).toBe('a-1')
    expect(out.project.runSettings).toEqual({ mode: 'solo' })
  })
})
