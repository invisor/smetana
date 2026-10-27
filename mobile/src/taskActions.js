/* What the task sheet does once a row is pressed: the writes and the three agent starts.

   The desktop's `DesktopApp.vue` carries these out inside the component (`setTaskStatus`,
   `toggleLock`, `deleteTask`, the three `startAgent` calls); the phone cannot import a `.vue`,
   so they are written again here over the same stores, with the same rules read from the same
   pure modules: the patch is `statusPatch`, the cascade is `lockIds` / `unlockIds`. What is left
   out is what only a window needs (the project-switch guard, the dialog that closes first).

   Every write answers whether it landed. A refusal is swallowed because its sentence already sits
   in `trackerState.lastError`, which is where the sheet reads it; the caller needs only the
   boolean to know whether to close. An issue that is no longer on the board - the desktop
   deleted it between the tap and the write - is not an error: the answer is `false` and nothing
   is sent. */
import { reactive } from 'vue'
import { lockIds, unlockIds } from '@src/components/kanban/lockCascade.js'
import { READY } from '@src/components/kanban/parked.js'
import { statusPatch } from '@src/components/kanban/statusPatch.js'
import { promotedNote } from '@src/components/run/promotedNote.js'
import { startConversation } from '@src/stores/conversation.js'
import { deleteIssue, issueById, trackerState, updateIssue } from '@src/stores/tracker.js'

/* Ids with a write in flight. `sheetRows` greys a card's rows from it (`busy`), and a cascade
   holds every id it will touch from before its first write. */
export const writingIds = reactive(new Set())

export async function setStatus(id, status) {
  const issue = issueById(id)
  if (!issue) return false
  writingIds.add(id)
  try {
    await updateIssue(id, statusPatch(issue.status, status))
    return true
  } catch {
    // the message already sits in trackerState.lastError
    return false
  } finally {
    writingIds.delete(id)
  }
}

/* Block and Unblock with the cascade an epic carries: one write per id, in the order
   `lockCascade.js` settles, every id greyed before the first write goes out and released as its
   own write lands, and the loop stops at the first refusal. */
export async function toggleLock(id) {
  const issue = issueById(id)
  if (!issue) return false
  const locking = issue.status === READY
  if (!locking && issue.status !== 'blocked') return false
  const issues = [...trackerState.issues.values()]
  const ids = locking ? lockIds(issues, id) : unlockIds(issues, id)
  if (!ids.length) return false
  for (const one of ids) writingIds.add(one)
  try {
    for (const one of ids) {
      try {
        await updateIssue(one, locking ? { status: 'blocked' } : { status: READY, append_notes: promotedNote('unblock') })
      } catch {
        // the message already sits in trackerState.lastError
        return false
      } finally {
        writingIds.delete(one)
      }
    }
    return true
  } finally {
    for (const one of ids) writingIds.delete(one)
  }
}

export async function remove(id) {
  if (!issueById(id)) return false
  writingIds.add(id)
  try {
    await deleteIssue(id)
    return true
  } catch {
    // the message already sits in trackerState.lastError
    return false
  } finally {
    writingIds.delete(id)
  }
}

/* An agent session on the issue, for `editTask`, `resolveTask` or `fixTask`: the session's id, or
   null when the issue is gone or the start was refused. */
export async function startOn(project, kind, id) {
  const issue = issueById(id)
  if (!issue || !project) return null
  return startConversation(project, { kind, id, title: issue.title })
}

/* The tracker's last refusal as one line for the sheet, or '' when there is none. The store keeps
   a caption and a description (a desktop draws them as a toast); a phone has no toasts. */
export function refusal() {
  const error = trackerState.lastError
  if (!error) return ''
  return [error.title, error.description].filter(Boolean).join('. ')
}
