import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { RAIL_CONTROL_MAX } from '../../src/views/panelWidths.js'

/* The stylesheets read as text, which is the only way a test in this project can
   reach them: nothing here renders CSS, and these two files are where the
   app-wide font size actually happens. `appearance.js` says only what the factor
   is — if these files stop multiplying by it, that factor reaches nothing and
   every other test stays green.

   Deliberately a check on the *mechanism* and not on the numbers. The sizes are
   the design system's to change; what must not change by accident is that each
   size is a multiple of `--ui-scale` and that the space scale is not.

   The lists are **derived from the files** rather than written out here. That is
   the point: a fixed list of "which tokens exist" would be a third copy of it,
   and the test whose job is to police a newly added step would be the one thing
   blind to a newly added step. The one place a set is pinned is which tokens the
   compact block redefines, and it is pinned precisely because *deletion* is the
   silent failure there — a height dropped from that block does not error, it
   quietly inherits the comfortable value and a 22px row becomes 28px.

   `EXPRESSION` matches one spelling and not the family of equivalents. An honest
   rewrite — `calc(10px * var(--ui-scale))` — will fail this test even though it
   behaves identically. That is the intended direction of error: refusing a
   correct edit costs one conversation, while accepting a shape this test cannot
   reason about costs the guarantee. Nothing here is semantically pinned; if the
   spelling changes on purpose, change it here too. */
const read = (name) => readFileSync(resolve(process.cwd(), 'src/styles/tokens', name), 'utf8')

const EXPRESSION = /^calc\(\s*\d+(\.\d+)?\s*\*\s*var\(--ui-scale\)\s*\*\s*1px\s*\)$/

const stripComments = (css) => css.replace(/\/\*[\s\S]*?\*\//g, '')

/* Every `--token: value` in a chunk of CSS, in source order. Naive on purpose:
   these files are one declaration per token with no nesting, and a real parser
   here would be a second thing to get wrong. Comments go first so that a token
   *named* in prose is never mistaken for one declared. */
const declarations = (css) => {
  const found = new Map()
  for (const [, name, value] of stripComments(css).matchAll(/(--[\w-]+)\s*:\s*([^;}\n]+)/g)) {
    found.set(name, value.trim())
  }
  return found
}

/* Each rule in the file, keyed by selector. */
const rules = (css) => {
  const found = new Map()
  for (const [, selector, body] of stripComments(css).matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    found.set(selector.trim(), declarations(body))
  }
  return found
}

const typography = rules(read('typography.css')).get(':root')
const space = rules(read('space.css'))
const comfortable = space.get(':root')
const compact = space.get('[data-density="compact"]')

/* What a token has to be named to count as a box that holds text: a height —
   including the `-sm`/`-lg` variants of one — or one of the icon sizes. Derived
   from the name so a `--foo-h` added tomorrow is covered without anybody
   remembering to add it here.

   The size suffixes are not decoration on that pattern. Requiring the name to
   *end* in `-h` is what the first version did, and it silently dropped
   `--control-h-sm` and `--control-h-lg` — two tokens sitting in the file right
   now, one of them the very token the collapsed rail's button is drawn at. The
   baseline passed either way, because both are correctly plugged in; it passed
   without looking at them, which is the failure that matters in a guard. */
const HOLDS_TEXT = /^--(.+-h(-(sm|lg))?|icon-(sm|md|lg))$/

/* The one exception, named with its reason. `--log-line-h` is the line box for
   `--text-code-size`, which is pinned by the editor setting and which the
   app-wide factor deliberately does not reach — scaling the box while the text
   inside it stays put is the opposite of what the rest of this exists for. */
const NOT_SCALED = new Set(['--log-line-h'])

/* Which tokens the compact block redefines. Pinned as a set, since a deletion
   here is silent — see the header. */
const COMPACT_TOKENS = [
  '--space-1',
  '--space-2',
  '--space-3',
  '--space-4',
  '--space-5',
  '--space-6',
  '--space-7',
  '--space-8',
  '--space-9',
  '--space-10',
  '--row-h',
  '--control-h',
  '--control-h-sm',
  '--control-h-lg',
  '--tab-h',
  '--titlebar-h',
  '--scope-bar-h',
  '--card-pad',
  '--panel-pad',
  '--log-line-h',
  '--tree-indent'
]

describe('the app-wide font size reaches the stylesheet', () => {
  it('is 1 by default, so a window that sets nothing looks shipped', () => {
    expect(typography.get('--ui-scale')).toBe('1')
  })

  it('multiplies every step of the type scale', () => {
    /* A step is a `--text-…` that states a size of its own; the semantic aliases
       (`--text-ui-size: var(--text-sm)`) state a step instead and inherit its
       scaling. Derived, so a ninth step added without the factor fails here. */
    const steps = [...typography].filter(
      ([name, value]) => name.startsWith('--text-') && !value.startsWith('var(')
    )

    expect(steps.length, 'the type scale is still written in this file').toBeGreaterThanOrEqual(8)
    for (const [name, value] of steps) {
      expect(value, `${name} must scale with --ui-scale`).toMatch(EXPRESSION)
    }
  })

  it('multiplies every row, control and icon size, in both densities', () => {
    for (const [density, block] of [
      ['comfortable', comfortable],
      ['compact', compact]
    ]) {
      const boxes = [...block].filter(([name]) => HOLDS_TEXT.test(name) && !NOT_SCALED.has(name))
      /* A floor, like the type scale's, and for the same reason: a pattern that
         quietly stops matching most of the set would otherwise leave this test
         green while checking almost nothing. Comfortable holds the complete set;
         compact redefines a subset of it. */
      const floor = density === 'comfortable' ? 8 : 1
      expect(boxes.length, `${density} still defines boxes that hold text`).toBeGreaterThanOrEqual(
        floor
      )
      for (const [name, value] of boxes) {
        expect(value, `${name} in ${density} must scale with --ui-scale`).toMatch(EXPRESSION)
      }
    }
  })

  it('the compact block redefines exactly the tokens it used to', () => {
    /* Deleting a height from here passes every other check in this file and
       changes the app: compact silently inherits the comfortable value, so a
       22px row becomes 28px with nothing anywhere to say so. */
    expect([...compact.keys()].sort()).toEqual([...COMPACT_TOKENS].sort())
  })

  it('the rail caps its button at the shipped small control, not at a number of its own', () => {
    /* `RAIL_CONTROL_MAX` exists so the collapsed rail's expand button stops
       growing at the rail's edge, and it is set to what that button measures
       today — `--control-h-sm` at the shipped size, in comfortable. That was a
       copy of the token's value with nothing holding the two together, and this
       is the join: the assertion lives here because this is the file that
       already reads the stylesheet, and `panelWidths.js` is pure with no imports
       of its own, so nothing is dragged anywhere by importing it.

       Equality rather than a floor, and it is the stronger statement on purpose.
       Below the token, the cap would shrink the button for everybody at the
       shipped size to fix something that only happens at the top of the range;
       above it, the cap would let the button grow past what the rail was
       measured to hold — and if `--control-h-sm` is ever raised beyond the rail
       itself, this and the fits-in-the-rail test in `panelWidths.test.js`
       disagree on purpose, which is a decision somebody has to make rather than
       a number to nudge. */
    const shipped = comfortable.get('--control-h-sm')
    const base = Number(shipped.match(/\d+(\.\d+)?/)[0])
    expect(base, '--control-h-sm still states a size of its own').toBeGreaterThan(0)
    expect(RAIL_CONTROL_MAX).toBe(base)
  })

  it('leaves the space scale alone', () => {
    /* The rejected option, pinned so it cannot be adopted by accident: padding
       and gaps are the rhythm of the interface, and scaling them with the type
       moves every panel by tens of pixels at the top of the range for no
       legibility gained. Density is the switch for how tight things sit. */
    for (const [density, block] of [
      ['comfortable', comfortable],
      ['compact', compact]
    ]) {
      const spaces = [...block].filter(([name]) => name.startsWith('--space-'))
      expect(spaces.length, `${density} still defines the space scale`).toBeGreaterThan(0)
      for (const [name, value] of spaces) {
        expect(value, `${name} in ${density} must not scale with the font size`).not.toContain(
          '--ui-scale'
        )
      }
    }
  })
})
