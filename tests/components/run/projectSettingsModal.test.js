import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

/* Vue components have no test runner in this project. This source-level check
   pins the layout contract that cannot move into `projectDefaults.js`: one
   responsive grid column for every project-default control, and an explicit
   fill on Select's inline-flex root as well as its wrapper. */
const source = readFileSync(
  resolve(process.cwd(), 'src/components/run/ProjectSettingsModal.vue'),
  'utf8'
)

describe('the Project settings defaults column', () => {
  it('gives all four controls one shrinkable right-hand column and makes each fill it', () => {
    expect(source).toContain("gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 170px)'")
    expect(source).toContain("const controlStyle = { width: '100%', minWidth: 0 }")
    expect(source).toContain(
      "const fieldControlStyle = { display: 'flex', flexDirection: 'column', alignItems: 'end', minWidth: 0 }"
    )
    expect(source).not.toContain('selectControlStyle')
    expect(source).not.toContain('numberControlStyle')

    const labels = ['Target branch', 'Minimum priority', 'Max parallel tasks', 'Review passes']
    for (const [index, label] of labels.entries()) {
      const start = source.indexOf(label)
      const end = index + 1 < labels.length ? source.indexOf(labels[index + 1], start) : source.indexOf('</template>', start)
      const row = source.slice(start, end)

      // One application sizes the wrapper; the other reaches Select's
      // inline-flex root (or Input's root) so every visible control fills the
      // common grid column instead of sizing itself from its value.
      expect(row.match(/:style="controlStyle"/g)).toHaveLength(2)
    }
  })
})
