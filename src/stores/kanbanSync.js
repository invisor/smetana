import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { onReconnect } from './connection.js'

export const kanbanSyncState = reactive({ projects: {}, keys: {} })
let initialization = null
let eventSerial = 0

export function initKanbanSync() {
  if (initialization) return initialization
  initialization = listen('kanban:sync', ({ payload }) => {
    if (payload?.project) {
      eventSerial += 1
      kanbanSyncState.projects[payload.project] = payload
    }
  }).then(() => {
    onReconnect(() => {
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
  const startedAt = eventSerial
  try {
    const state = await invoke('kanban_sync_state', { project })
    if (state?.project) {
      kanbanSyncState.keys[project] = state.project
      if (startedAt === eventSerial || !kanbanSyncState.projects[state.project]) {
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

export function syncProblem(project) {
  const state = kanbanSyncState.projects[kanbanSyncState.keys[project] ?? project]
  return state?.pullError ?? state?.pushError ?? null
}
