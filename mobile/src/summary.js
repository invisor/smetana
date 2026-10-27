/* What the home screen shows, and the copy of it kept for when the desktop
   cannot be reached.

   Pure: the storage is handed in, so a test needs no DOM. The copy is the last
   thing the phone read — a project's name and how many tasks it has — and it is
   shown read-only under "Workstation unreachable", which is the only reason it
   is kept at all. It also holds the desktop's look (theme and density), so that a
   launch with the desktop out of reach opens in the last look it had. */
import { basename } from '@src/paths.js'

const KEY = 'smetana.companion.summary'

/* The active project's folder name, or null when the desktop has none open. */
export function projectName(activeProject) {
  return activeProject ? basename(activeProject) : null
}

/* How many tasks to say. bd's own lock issues are bookkeeping and are not
   tasks; `isLock` is the desktop front end's own test for them. */
export function taskCount(issues, isLock) {
  let count = 0
  for (const issue of issues) if (!isLock(issue)) count += 1
  return count
}

export function saveSummary(storage, summary) {
  try {
    storage.setItem(KEY, JSON.stringify({ ...summary, savedAt: Date.now() }))
  } catch {
    // A full or refused store only costs the copy.
  }
}

export function loadSummary(storage) {
  try {
    const parsed = JSON.parse(storage.getItem(KEY) ?? 'null')
    if (parsed && typeof parsed.tasks === 'number') return parsed
  } catch {
    // Damaged: as if there were none.
  }
  return null
}

/* The look kept in the summary, or null when there is none or it is damaged. */
export function savedLook(summary) {
  const look = summary?.look
  if (look && typeof look.theme === 'string' && typeof look.density === 'string') {
    return { theme: look.theme, density: look.density }
  }
  return null
}

export function forgetSummary(storage) {
  try {
    storage.removeItem(KEY)
  } catch {
    // Nothing to do about it.
  }
}
