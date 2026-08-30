import { describe, expect, it } from 'vitest'
import { stemRange } from '../../../src/components/files/renameName.js'

describe('stemRange', () => {
  it('selects the name without its extension', () => {
    expect(stemRange('report.md')).toEqual([0, 6])
  })

  it('selects the whole name when there is no extension', () => {
    expect(stemRange('notes')).toEqual([0, 5])
  })

  it('selects the whole name of a dotfile, whose leading dot is not a separator', () => {
    expect(stemRange('.gitignore')).toEqual([0, 10])
  })

  it('splits on the last dot, so a double extension keeps only its last part', () => {
    expect(stemRange('archive.tar.gz')).toEqual([0, 11])
  })

  it('selects nothing at all of an empty name, rather than throwing', () => {
    expect(stemRange('')).toEqual([0, 0])
    expect(stemRange()).toEqual([0, 0])
  })
})
