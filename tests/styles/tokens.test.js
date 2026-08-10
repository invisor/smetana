import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

/* The stylesheet read as text, which is the only way a test in this project can
   reach it: nothing here renders CSS, and the two files below are where the app
   -wide font size actually happens. `appearance.js` says only what the factor is
   — if these files stop multiplying by it, that factor reaches nothing and
   every other test stays green.

   This is deliberately a check on the *mechanism* and not on the numbers. The
   sizes are the design system's to change; what must not change by accident is
   that each of them is a multiple of `--ui-scale`, and that the space scale is
   not. */
/* From the project root rather than from `import.meta.url`: these files are
   served by Vite, so `import.meta.url` inside a test is an http:// URL and
   `fileURLToPath` refuses it. Vitest runs the workers with the config's root as
   the working directory. */
const read = (name) => readFileSync(resolve(process.cwd(), 'src/styles/tokens', name), 'utf8')

const typography = read('typography.css')
const space = read('space.css')

/* The value of one custom property, as written. Naive on purpose: these files
   are one declaration per token and a parser here would be a second thing to
   get wrong. */
const declared = (css, token) => {
  const found = css.match(new RegExp(`${token}\\s*:\\s*([^;\\n]+)`))
  return found ? found[1].trim() : null
}

const TYPE_STEPS = [
  '--text-2xs',
  '--text-xs',
  '--text-sm',
  '--text-md',
  '--text-lg',
  '--text-xl',
  '--text-2xl',
  '--text-3xl'
]

/* Every height a piece of text sits inside. `--row-h` at 22px in compact with
   22px text in it is the case this list exists for. */
const SCALED_SIZES = [
  '--row-h',
  '--control-h',
  '--control-h-sm',
  '--control-h-lg',
  '--tab-h',
  '--titlebar-h',
  '--scope-bar-h',
  '--icon-sm',
  '--icon-md',
  '--icon-lg'
]

describe('the app-wide font size reaches the stylesheet', () => {
  it('is 1 by default, so a window that sets nothing looks shipped', () => {
    expect(declared(typography, '--ui-scale')).toBe('1')
  })

  it('multiplies every step of the type scale', () => {
    for (const step of TYPE_STEPS) {
      const value = declared(typography, step)
      expect(value, `${step} is defined`).toBeTruthy()
      expect(value, `${step} must scale with --ui-scale`).toMatch(
        /^calc\(\s*\d+(\.\d+)?\s*\*\s*var\(--ui-scale\)\s*\*\s*1px\s*\)$/
      )
    }
  })

  it('multiplies every row and control height, in both densities', () => {
    // Split at the density block so each half is checked on its own: scaling
    // only the comfortable set would leave compact — the tighter of the two, and
    // the one that clips first — behind.
    // The selector where it opens a rule, at the start of a line — the file's
    // own header comment names it too, and splitting there left "comfortable"
    // as the comment and nothing else.
    const at = space.search(/^\[data-density="compact"\]/m)
    expect(at, 'the compact block is still in this file').toBeGreaterThan(0)
    const blocks = { comfortable: space.slice(0, at), compact: space.slice(at) }

    for (const [density, css] of Object.entries(blocks)) {
      for (const token of SCALED_SIZES) {
        const value = declared(css, token)
        // Compact redefines only some of them; what it does redefine must scale.
        if (value === null) continue
        expect(value, `${token} in ${density} must scale with --ui-scale`).toMatch(
          /^calc\(\s*\d+(\.\d+)?\s*\*\s*var\(--ui-scale\)\s*\*\s*1px\s*\)$/
        )
      }
    }

    // And the comfortable block, which is the complete set, defines them all.
    for (const token of SCALED_SIZES) {
      expect(declared(blocks.comfortable, token), `${token} is defined`).toBeTruthy()
    }
  })

  it('leaves the space scale alone', () => {
    /* The rejected option, pinned so it cannot be adopted by accident: padding
       and gaps are the rhythm of the interface, and scaling them with the type
       moves every panel by tens of pixels at the top of the range for no
       legibility gained. Density is the switch for how tight things sit. */
    for (let i = 0; i <= 10; i += 1) {
      const value = declared(space, `--space-${i}`)
      expect(value, `--space-${i} is defined`).toBeTruthy()
      expect(value, `--space-${i} must not scale with the font size`).not.toContain('--ui-scale')
    }
  })
})
