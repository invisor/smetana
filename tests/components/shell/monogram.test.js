import { describe, expect, it } from 'vitest'
import { monogram } from '../../../src/components/shell/monogram.js'

describe('monogram', () => {
  it('takes the first letter of each of the first two segments', () => {
    expect(monogram('holiday-curb')).toBe('hc')
    expect(monogram('beads-viewer')).toBe('bv')
    expect(monogram('my_side_project')).toBe('ms')
    expect(monogram('notes app')).toBe('na')
  })

  it('takes the first two characters of a name with one segment', () => {
    expect(monogram('smetana')).toBe('sm')
    expect(monogram('notes')).toBe('no')
  })

  it('lowercases whatever it took', () => {
    expect(monogram('Smetana')).toBe('sm')
    expect(monogram('Holiday-Curb')).toBe('hc')
  })

  it('keeps digits, which are as much of a name as letters are', () => {
    expect(monogram('2fa-server')).toBe('2s')
    expect(monogram('v2')).toBe('v2')
  })

  it('gives one character for a one-character name', () => {
    expect(monogram('x')).toBe('x')
  })

  it('never comes back empty, whatever it is handed', () => {
    expect(monogram('...')).toBe('··')
    expect(monogram('')).toBe('··')
    expect(monogram(null)).toBe('··')
    expect(monogram(undefined)).toBe('··')
  })
})
