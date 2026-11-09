export const KANBAN_SYNC_DEFAULTS = Object.freeze({
  enabled: false,
  pullCommand: 'bd dolt pull',
  pushCommand: 'bd dolt push',
  pullIntervalMinutes: 5
})

export function syncDraft(value) {
  return { ...KANBAN_SYNC_DEFAULTS, ...value }
}

export function syncErrors(value) {
  if (!value.enabled) return {}
  const errors = {}
  if (!String(value.pullCommand ?? '').trim()) errors.pullCommand = 'Enter a pull command.'
  if (!String(value.pushCommand ?? '').trim()) errors.pushCommand = 'Enter a push command.'
  const minutes = Number(value.pullIntervalMinutes)
  if (!Number.isInteger(minutes) || minutes < 1 || minutes > 1440 || value.pullIntervalMinutes === '') {
    errors.pullIntervalMinutes = 'Enter a whole number from 1 to 1440.'
  }
  return errors
}

export function sameSync(a, b) {
  const left = syncDraft(a)
  const right = syncDraft(b)
  return Object.keys(KANBAN_SYNC_DEFAULTS).every((key) => left[key] === right[key])
}
