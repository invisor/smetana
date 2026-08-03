/* An issue's type — what the work IS. The board's columns already say what it
   is doing, so the card spends its one badge on the thing the column cannot
   tell you.

   bd ships six: bug, feature, task, epic, chore, decision, with `task` the
   default and custom ones allowed through its types.custom config. Only three
   are coloured (see tokens/color-type.css); everything else, known or custom,
   is neutral. A type that this file has never heard of is an ordinary outcome
   rather than an error: it renders with its own name and the generic glyph.

   Pure, and therefore the part of this that a test can reach — TypeBadge.vue is
   checked by eye like every other component. */

const KNOWN = {
  bug: { glyph: 'bug', colour: 'bug' },
  feature: { glyph: 'sparkles', colour: 'feature' },
  epic: { glyph: 'layers', colour: 'epic' },
  task: { glyph: 'square-check', colour: 'plain' },
  chore: { glyph: 'wrench', colour: 'plain' },
  decision: { glyph: 'milestone', colour: 'plain' }
}

export const TYPES = Object.keys(KNOWN)

export function normalizeType(t) {
  return String(t || '').toLowerCase().trim().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')
}

/* Sentence case, in sans, the way the rest of the interface writes prose — a
   type is a word for a person, not an identifier. A custom `tech-debt` reads
   as "Tech debt". */
export function typeLabel(t) {
  const n = normalizeType(t).replace(/-/g, ' ')
  return n ? n[0].toUpperCase() + n.slice(1) : ''
}

export function typeGlyph(t) {
  return KNOWN[normalizeType(t)]?.glyph ?? 'tag'
}

export function typeColors(t) {
  const n = normalizeType(t)
  const colour = KNOWN[n]?.colour ?? 'plain'
  return {
    key: n,
    known: n in KNOWN,
    /* `plain` is deliberately not a hue: three of the six types and every
       custom one share it, so the coloured three keep their meaning. */
    plain: colour === 'plain',
    fg: `var(--type-${colour}-fg)`,
    bg: `var(--type-${colour}-bg)`
  }
}
