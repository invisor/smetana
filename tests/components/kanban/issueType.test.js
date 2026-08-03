import { describe, expect, it } from 'vitest'
import { TYPES, normalizeType, typeColors, typeGlyph, typeLabel } from '../../../src/components/kanban/issueType.js'

describe('the type vocabulary', () => {
  it('covers everything bd creates', () => {
    // bd create -t: bug|feature|task|epic|chore|decision, default task.
    expect(TYPES.sort()).toEqual(['bug', 'chore', 'decision', 'epic', 'feature', 'task'])
  })

  it('reads as prose, not as an identifier', () => {
    expect(typeLabel('bug')).toBe('Bug')
    expect(typeLabel('tech-debt')).toBe('Tech debt')
    expect(typeLabel(null)).toBe('')
  })

  it('normalizes the way status does, so a stray case or space cannot split a type in two', () => {
    expect(normalizeType('  Bug ')).toBe('bug')
    expect(normalizeType('Tech Debt')).toBe('tech-debt')
  })
})

describe('colour and glyph', () => {
  /* The point of the palette: bd's default type is task, so a board where
     everything is coloured is a board where nothing stands out. */
  it('three types carry a hue and the rest do not', () => {
    expect(typeColors('bug').plain).toBe(false)
    expect(typeColors('feature').plain).toBe(false)
    expect(typeColors('epic').plain).toBe(false)
    expect(typeColors('task').plain).toBe(true)
    expect(typeColors('chore').plain).toBe(true)
    expect(typeColors('decision').plain).toBe(true)
  })

  it('a custom type is an ordinary outcome: neutral colours and the generic glyph', () => {
    const c = typeColors('tech-debt')
    expect(c.known).toBe(false)
    expect(c.plain).toBe(true)
    expect(c.fg).toBe('var(--type-plain-fg)')
    expect(typeGlyph('tech-debt')).toBe('tag')
  })

  it('every colour is a token reference — no component may hardcode one', () => {
    for (const t of [...TYPES, 'tech-debt']) {
      const c = typeColors(t)
      expect(c.fg).toMatch(/^var\(--type-[a-z]+-fg\)$/)
      expect(c.bg).toMatch(/^var\(--type-[a-z]+-bg\)$/)
    }
  })
})
