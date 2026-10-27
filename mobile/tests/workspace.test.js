import { describe, expect, it } from 'vitest'
import { installCore, loadShell, snapshot } from './support/bridge.js'
import { ALLOWED_COMMANDS, ALLOWED_EVENTS } from '../src/allowed.js'
import { newTaskIntent } from '../src/newTask.js'
import { runSettings } from '../src/runStart.js'

/* The whole of what the working screens ask the desktop for, run through the real stores over
   the real bridge modules with the Rust core as the only thing replaced. The point is the
   audit: whatever a screen reaches for, it is on the list. */

const CORE = ['bridge_invoke', 'bridge_listen', 'bridge_unlisten', 'bridge_state', 'take_push_target', 'pair', 'connect', 'unpair']

const RUN = {
  token: 1,
  project: '/work/a',
  transport: 'driven',
  session: 5,
  state: { kind: 'working', iteration: 1 },
  settings: { scope: { kind: 'queue' }, target_branch: 'main' },
  batches: 1,
  stopping: false
}
const desktop = (extra = {}) => ({
  tracker_health: { state: 'ok' },
  tracker_snapshot: snapshot(1, ['smetana-1']),
  tracker_set_project: snapshot(7, ['other-1']),
  settings_load: (args) => ({
    openProjects: ['/work/a', '/work/b'],
    activeProject: args?.project ?? '/work/a',
    kanban: { columns: 'all', alwaysShow: [], interval: 'all', unlimited: [] },
    project: { columnOrder: ['closed', 'open'], runSettings: { mode: 'supervised', targetBranch: 'develop' } },
    conversationPanel: true,
    agent: 'claude'
  }),
  agents_catalog: [{ id: 'claude', label: 'Claude Code', capabilities: {} }],
  terminal_list: [{ id: 2, project: '/work/a', agent: 'claude', state: 'idle', startedAt: '2026-10-15T10:00:00Z', work: { kind: 'bare' } }],
  project_config: { state: 'ok', defaults: { target_branch: 'main', max_parallel_tasks: 2 } },
  run_state: [RUN],
  target_branches: [{ name: 'main', missing_in: [] }, { name: 'develop', missing_in: [] }],
  session_attach: { events: [{ seq: 1, kind: 'turn-start' }], seq: 1, state: 'running', cwd: '/work/a', agent: 'claude' },
  session_list: [],
  terminal_restorable: [],
  session_start: 7,
  // The desktop answers these with nothing; the recorder reads a falsy answer as "not offered".
  session_send: {},
  session_answer: {},
  session_stop: {},
  run_start: { ...RUN, token: 2, session: null },
  run_stop: { ...RUN, stopping: true },
  terminal_attach: { data: 'aGk=', seq: 1 },
  // The updated issue, and nothing for a delete (an empty answer, since the recorder reads a falsy one as not offered).
  tracker_update: (args) => ({ id: args.id, title: 'Fix the board', ...args.patch }),
  tracker_delete: {},
  bridge_listen: null,
  bridge_unlisten: null,
  ...extra
})

const start = async (extra) => {
  const core = installCore(desktop(extra))
  const shell = await loadShell()
  const workspace = await import('../src/workspace.js')
  const settings = await import('../../src/stores/settings.js')
  const stores = {
    attachments: await import('../../src/stores/attachments.js'),
    conversation: await import('../../src/stores/conversation.js'),
    runs: await import('../../src/stores/runs.js'),
    terminals: await import('../../src/stores/terminals.js'),
    tracker: await import('../../src/stores/tracker.js')
  }
  await shell.tracker.initTracker()
  return { core, ...shell, workspace, settings: settings.settings, stores }
}

const names = (core) => core.calls.filter((call) => call.cmd === 'bridge_invoke').map((call) => call.args.cmd)

describe('what the working screens ask the desktop for', () => {
  it('is only the allowed commands and events, across every screen\'s reads and writes', async () => {
    const { core, workspace, stores } = await start()
    await workspace.startWorkspace()
    await workspace.loadRunsFor('/work/b')
    await workspace.chooseProject('/work/b')
    await workspace.refreshFromDesktop()

    await stores.conversation.startConversation('/work/a', newTaskIntent({ text: 'Fix the board' }))
    await stores.conversation.sendMessage(7, 'hello')
    await stores.conversation.answerQuestion(7, 'q1', 'allow')
    await stores.conversation.stopConversation(7)
    await stores.runs.startRun('/work/a', runSettings({ config: stores.runs.runsState.config, branches: [{ name: 'main', missing_in: [] }] }))
    await stores.runs.stopRun(2)
    const { invoke } = await import('../src/bridge/core.js')
    await invoke('terminal_attach', { id: 2 })
    await stores.tracker.updateIssue('smetana-1', { status: 'open' })
    await stores.tracker.deleteIssue('smetana-2')

    const asked = new Set(names(core))
    expect([...asked].filter((name) => !ALLOWED_COMMANDS.includes(name))).toEqual([])
    const direct = new Set(core.calls.filter((call) => call.cmd !== 'bridge_invoke').map((call) => call.cmd))
    expect([...direct].filter((name) => !CORE.includes(name))).toEqual([])
    const listened = core.core('bridge_listen').map((args) => args.event)
    expect(listened.filter((event) => !ALLOWED_EVENTS.includes(event))).toEqual([])

    // The audit is only worth something if the screens really did reach for these.
    for (const name of ['settings_load', 'tracker_set_project', 'agents_catalog', 'project_config', 'run_state', 'target_branches', 'terminal_list', 'session_start', 'session_attach', 'session_send', 'session_answer', 'session_stop', 'run_start', 'run_stop', 'terminal_attach', 'tracker_update', 'tracker_delete']) {
      expect(asked.has(name), name).toBe(true)
    }
    expect(listened).toEqual(expect.arrayContaining(['tracker:delta', 'tracker:health', 'run:state', 'session:events', 'terminal:state', 'terminal:removed']))
  })

  it('never writes the desktop\'s settings or touches its files, git or terminals', async () => {
    const { core, workspace } = await start()
    await workspace.startWorkspace()
    await workspace.chooseProject('/work/b')
    const asked = names(core)
    for (const name of ['settings_save', 'terminal_marks', 'terminal_create', 'terminal_write', 'tracker_access_repair', 'git_head', 'files_list', 'vcs_status', 'drag_drop_space', 'browser_tools']) {
      expect(asked, name).not.toContain(name)
    }
  })
})

describe('reading the desktop', () => {
  it('fills the settings the screens read, and the sessions and runs of the project', async () => {
    const { workspace, settings, stores } = await start()
    await workspace.startWorkspace()
    expect(settings.openProjects).toEqual(['/work/a', '/work/b'])
    expect(settings.activeProject).toBe('/work/a')
    expect(settings.project.columnOrder).toEqual(['closed', 'open'])
    expect(stores.terminals.terminalState.sessions.map((session) => session.id)).toEqual([2])
    expect(stores.runs.runsState.runs.map((run) => run.token)).toEqual([1])
    expect(workspace.workspace.ready).toBe(true)
  })

  it('adopts the session a run is driving, so it is in the agents list', async () => {
    const { workspace, stores } = await start()
    await workspace.startWorkspace()
    expect(stores.conversation.drivenSessions.value.map((session) => session.id)).toEqual([5])
    expect(stores.conversation.drivenSessions.value[0].project).toBe('/work/a')
  })

  it('starts the stores\' listeners once however often the link comes back', async () => {
    const { core, workspace } = await start()
    await workspace.startWorkspace()
    await workspace.startWorkspace()
    const listened = core.core('bridge_listen').map((args) => args.event)
    expect(listened.filter((event) => event === 'run:state')).toHaveLength(1)
    expect(listened.filter((event) => event === 'terminal:state')).toHaveLength(1)
  })

  it('reads the board again when the desktop is found to have another project open', async () => {
    let active = '/work/a'
    const { core, workspace, settings } = await start({
      settings_load: (args) => ({ openProjects: ['/work/a', '/work/b'], activeProject: args?.project ?? active })
    })
    await workspace.startWorkspace()
    const before = names(core).filter((name) => name === 'tracker_snapshot').length
    await workspace.refreshFromDesktop()
    expect(names(core).filter((name) => name === 'tracker_snapshot')).toHaveLength(before)
    active = '/work/b'
    await workspace.refreshFromDesktop()
    expect(settings.activeProject).toBe('/work/b')
    expect(names(core).filter((name) => name === 'tracker_snapshot')).toHaveLength(before + 1)
  })
})

describe('switching the project', () => {
  it('switches the desktop\'s one active project and brings the page to the new one', async () => {
    const { core, workspace, settings, tracker } = await start()
    await workspace.startWorkspace()
    expect(await workspace.chooseProject('/work/b')).toBe(true)
    expect(core.bridged('tracker_set_project')).toEqual([{ path: '/work/b' }])
    expect(core.bridged('settings_load').pop()).toEqual({ project: '/work/b' })
    expect(settings.activeProject).toBe('/work/b')
    expect(tracker.trackerState.issues.has('other-1')).toBe(true)
    expect(workspace.workspace.switching).toBeNull()
  })

  it('stays where it was when the desktop refuses', async () => {
    const { workspace, settings } = await start({
      tracker_set_project: () => {
        throw 'forbidden'
      }
    })
    await workspace.startWorkspace()
    expect(await workspace.chooseProject('/work/b')).toBe(false)
    expect(settings.activeProject).toBe('/work/a')
    expect(workspace.workspace.switching).toBeNull()
  })

  it('takes one switch at a time', async () => {
    const { core, workspace } = await start()
    await workspace.startWorkspace()
    await Promise.all([workspace.chooseProject('/work/b'), workspace.chooseProject('/work/a')])
    expect(core.bridged('tracker_set_project')).toHaveLength(1)
  })
})

describe('talking to an agent', () => {
  it('files a task by starting the agent with the desktop\'s intent and attaching to it', async () => {
    const { core, stores } = await start()
    const id = await stores.conversation.startConversation('/work/a', newTaskIntent({ text: 'Fix the board' }))
    expect(id).toBe(7)
    expect(core.bridged('session_start')).toEqual([{ project: '/work/a', intent: newTaskIntent({ text: 'Fix the board' }) }])
    expect(core.bridged('session_attach')).toEqual([{ id: 7 }])
  })

  it('attaches a picture as bytes through attachment_write and keeps the path it was given', async () => {
    const { core, stores } = await start({
      attachment_write: ({ name }) => ({ path: `/store/p/${name}`, name, bytes: 2, mime: 'image/png', data: 'aGk=' })
    })
    await stores.attachments.attachFiles([new File([new Uint8Array([104, 105])], 'shot.png')])
    expect(core.bridged('attachment_write')).toEqual([{ name: 'shot.png', data: 'aGk=' }])
    expect(stores.attachments.attachmentsState.items.map((item) => item.path)).toEqual(['/store/p/shot.png'])
    stores.attachments.removeAttachment('/store/p/shot.png')
    expect(stores.attachments.attachmentsState.items).toEqual([])
  })

  it('files a task naming the pictures the desktop stored, and never asks it to import a path', async () => {
    const { core, stores } = await start({
      attachment_write: ({ name }) => ({ path: `/store/p/${name}`, name, bytes: 2, mime: 'image/png', data: 'aGk=' })
    })
    await stores.attachments.attachFiles([new File([new Uint8Array([104, 105])], 'shot.png')])
    const attachments = stores.attachments.attachmentsState.items
    await stores.conversation.startConversation('/work/a', newTaskIntent({ text: 'Fix it', attachments }))
    expect(core.bridged('session_start')[0].intent.draft.images).toEqual(['/store/p/shot.png'])
    expect(core.bridged('session_start')[0].intent.draft.files).toEqual([])
    expect(core.bridged('attachment_import')).toEqual([])
    expect(names(core)).not.toContain('attachment_import')
  })

  it('sends a message with no attachments, which the desktop would refuse on a phone', async () => {
    const { core, stores } = await start()
    await stores.conversation.sendMessage(7, 'hello')
    expect(core.bridged('session_send')).toEqual([{ id: 7, text: 'hello', attachments: [] }])
  })

  it('answers a question by its id', async () => {
    const { core, stores } = await start()
    await stores.conversation.answerQuestion(7, 'q1', 'allow')
    expect(core.bridged('session_answer')).toEqual([{ id: 7, question: 'q1', decision: 'allow', answers: null }])
  })

  it('says why an agent could not be started', async () => {
    const { stores } = await start({
      session_start: () => {
        throw { kind: 'spawn', message: 'claude is not installed' }
      }
    })
    expect(await stores.conversation.startConversation('/work/a', { kind: 'bare' })).toBeNull()
    expect(stores.conversation.conversationState.lastError).toMatchObject({ session: null, text: 'claude is not installed' })
  })

  it('can start a conversation only when the desktop has the panel on and the agent can be driven', async () => {
    const { workspace, stores } = await start()
    await workspace.startWorkspace()
    expect(stores.conversation.canDrive('claude')).toBe(true)
    expect(stores.conversation.canDrive('gemini')).toBe(false)
    const off = await start({ settings_load: { openProjects: [], activeProject: null, conversationPanel: false, agent: 'claude' } })
    await off.workspace.startWorkspace()
    expect(off.stores.conversation.canDrive('claude')).toBe(false)
  })
})

describe('runs', () => {
  it('starts the queue with the settings the desktop\'s window opens on, and stops by token', async () => {
    const { core, workspace, stores } = await start()
    await workspace.startWorkspace()
    await workspace.loadRunsFor('/work/a')
    const { gitState } = await import('../../src/stores/git.js')
    const sent = runSettings({
      config: stores.runs.runsState.config,
      branches: gitState.branches,
      remembered: { mode: 'supervised', targetBranch: 'develop' }
    })
    await stores.runs.startRun('/work/a', sent)
    expect(core.bridged('run_start')).toEqual([{ project: '/work/a', settings: sent }])
    expect(sent).toMatchObject({ scope: { kind: 'queue' }, mode: 'supervised', target_branch: 'develop', max_parallel_tasks: 2 })
    await stores.runs.stopRun(2)
    expect(core.bridged('run_stop')).toEqual([{ token: 2 }])
  })

  it('follows a run\'s state events for the project on screen', async () => {
    const { workspace, stores, emit, nextTick } = await start()
    await workspace.startWorkspace()
    await emit('bridge:event', { name: 'run:state', payload: { ...RUN, state: { kind: 'stopped', reason: { kind: 'queue_empty' } } } })
    await nextTick()
    expect(stores.runs.runsState.runs[0].state.kind).toBe('stopped')
  })

  it('does not ring a sound on a run ending: the phone is told by a notification', async () => {
    const { workspace, settings } = await start()
    await workspace.startWorkspace()
    expect(settings.notifications.runFinished).toBe('off')
    expect(settings.notifications.needsAttention).toBe('off')
  })
})

describe('the Runs screen on a project that is not the active one', () => {
  const byProject = (args) => (args?.project === '/work/b' ? { state: 'ok', defaults: { target_branch: 'b-main' } } : { state: 'ok', defaults: { target_branch: 'a-main' } })

  it('keeps the run stores on that project through a cold start and a return from the background', async () => {
    const { core, workspace, stores } = await start({ project_config: byProject })
    await workspace.loadRunsFor('/work/b')
    expect(stores.runs.runsState.project).toBe('/work/b')

    await workspace.startWorkspace()
    expect(stores.runs.runsState.project).toBe('/work/b')
    await workspace.refreshFromDesktop()
    expect(stores.runs.runsState.project).toBe('/work/b')
    expect(stores.runs.runsState.config.defaults.target_branch).toBe('b-main')
    expect(core.bridged('project_config').filter((a) => a.project === '/work/a')).toEqual([])
  })

  it('follows the active project again once the screen is gone', async () => {
    const { workspace, stores } = await start({ project_config: byProject })
    await workspace.loadRunsFor('/work/b')
    workspace.unpinRuns()
    await workspace.startWorkspace()
    expect(stores.runs.runsState.project).toBe('/work/a')
  })

  it('brings the stores back to the active project the moment the screen is gone', async () => {
    const { workspace, stores } = await start({ project_config: byProject })
    await workspace.startWorkspace()
    await workspace.loadRunsFor('/work/b')
    workspace.unpinRuns()
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(stores.runs.runsState.project).toBe('/work/a')
    expect(stores.runs.runsState.config.defaults.target_branch).toBe('a-main')
  })

  it('reads the pinned project again on a return from the background', async () => {
    const { core, workspace } = await start({ project_config: byProject })
    await workspace.loadRunsFor('/work/b')
    const before = core.bridged('run_state').filter((a) => a.project === '/work/b').length
    await workspace.refreshFromDesktop()
    expect(core.bridged('run_state').filter((a) => a.project === '/work/b').length).toBe(before + 1)
  })
})
