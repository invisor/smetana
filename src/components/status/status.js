/* ---- deterministic colour for user-defined statuses -----------------------
   1. normalise: lowercase, trim, collapse every run of non-alphanumerics to "-"
   2. hash with FNV-1a (32-bit) - stable across runs, processes and platforms
   3. slot = hash % 12 -> --status-gen-<slot>-{fg,bg,border}
   The 12 generated hues sit outside a +/-18 deg guard band around every
   reserved semantic hue and are held below needs-you chroma, so a generated
   status can never be mistaken for a reserved one or out-shout the ladder.
   Colour is never alone: generated statuses also render a 2-letter code. */
export const RESERVED = ['blocked', 'ready', 'running', 'needs-you', 'done', 'failed']
const GEN_SLOTS = 12

export function normalizeStatus(s) {
  return String(s || '').toLowerCase().trim().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')
}

export function hashStatus(s) {
  let h = 0x811c9dc5
  const n = normalizeStatus(s)
  for (let i = 0; i < n.length; i++) {
    h ^= n.charCodeAt(i)
    h = Math.imul(h, 0x01000193) >>> 0
  }
  return h >>> 0
}

export function statusSlot(s) {
  return hashStatus(s) % GEN_SLOTS
}

export function statusColors(s) {
  const n = normalizeStatus(s)
  if (RESERVED.indexOf(n) >= 0) {
    return {
      reserved: true,
      key: n,
      fg: `var(--status-${n}-fg)`,
      bg: `var(--status-${n}-bg)`,
      border: `var(--status-${n}-border)`
    }
  }
  const i = statusSlot(n)
  return {
    reserved: false,
    key: n,
    slot: i,
    fg: `var(--status-gen-${i}-fg)`,
    bg: `var(--status-gen-${i}-bg)`,
    border: `var(--status-gen-${i}-border)`
  }
}

export function statusCode(s) {
  const parts = normalizeStatus(s).split('-').filter(Boolean)
  if (parts.length > 1) return (parts[0][0] + parts[1][0]).toUpperCase()
  return normalizeStatus(s).slice(0, 2).toUpperCase()
}

/* attention level - drives loudness everywhere */
export function attentionLevel(s) {
  const n = normalizeStatus(s)
  if (n === 'needs-you' || n === 'failed') return 'loud'
  if (n === 'running') return 'live'
  if (n === 'done') return 'quiet'
  return 'live'
}

export const STATUS_GLYPH = {
  blocked: 'lock',
  ready: 'circle-dashed',
  running: 'loader-circle',
  'needs-you': 'triangle-alert',
  done: 'check',
  failed: 'x'
}
