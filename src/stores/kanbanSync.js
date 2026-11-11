import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { connection, onReconnect } from './connection.js'

export const kanbanSyncState = reactive({ projects: {}, keys: {}, hiddenEvents: {} })
let initialization = null
const eventSerial = {}
let openProjects = []
let openTarget = null
let presenceRevision = 0

export async function publishOpenProjects(projects) {
  openProjects = [...projects]
  openTarget = connection.target
  const revision = ++presenceRevision
  try {
    await invoke('kanban_sync_presence', { projects: openProjects, revision })
  } catch (error) {
    console.warn('[kanban sync] could not publish open projects:', error)
  }
}

export function initKanbanSync() {
  if (initialization) return initialization
  initialization = listen('kanban:sync', ({ payload }) => {
    if (payload?.project) {
      eventSerial[payload.project] = (eventSerial[payload.project] ?? 0) + 1
      kanbanSyncState.projects[payload.project] = payload
    }
  }).then(() => {
    onReconnect(() => {
      if (connection.target === openTarget) publishOpenProjects(openProjects)
      for (const project of Object.keys(kanbanSyncState.projects)) readKanbanSync(project)
    })
  }).catch((error) => {
    initialization = null
    console.warn('[kanban sync] could not subscribe:', error)
  })
  return initialization
}

export async function readKanbanSync(project) {
  if (!project) return
  const startedAt = { ...eventSerial }
  try {
    const state = await invoke('kanban_sync_state', { project })
    if (state?.project) {
      kanbanSyncState.keys[project] = state.project
      if ((startedAt[state.project] ?? 0) === (eventSerial[state.project] ?? 0) || !kanbanSyncState.projects[state.project]) {
        kanbanSyncState.projects[state.project] = state
      }
    }
  } catch (error) {
    console.warn('[kanban sync] could not read current state:', error)
  }
}

export async function pullKanbanSync(project) {
  if (!project) return
  try {
    await invoke('kanban_sync_pull', { project })
  } catch (error) {
    // The server publishes project-addressed state, including failures. The
    // board remains available; a rejected request does not change tracker health.
    console.warn('[kanban sync] pull failed:', error)
  }
}

export function syncProblems(project) {
  const state = kanbanSyncState.projects[kanbanSyncState.keys[project] ?? project]
  return [state?.pullError, state?.pushError].filter(Boolean)
}

// The server's revisions survive a missed event and arrive with the next state
// read. Its session id makes a server restart distinct from the old process.
export function syncNotices(project) {
  const key = kanbanSyncState.keys[project] ?? project
  const state = kanbanSyncState.projects[key]
  return syncProblems(project).map((problem) => ({
    ...problem,
    event: `${key}:${state?.serverSessionId ?? ''}:${problem.operation}:${state?.[`${problem.operation}FailureRevision`] ?? 0}`
  })).filter((problem) => kanbanSyncState.hiddenEvents[key]?.[problem.operation] !== problem.event)
}

export function dismissSyncNotice(project, problem) {
  const key = kanbanSyncState.keys[project] ?? project
  kanbanSyncState.hiddenEvents[key] = {
    ...kanbanSyncState.hiddenEvents[key],
    [problem.operation]: problem.event
  }
}
