import { describe, expect, it } from 'vitest'
import { parseAnsi } from '../../../src/components/agent/ansi.js'

const ESC = '\u001b'

describe('parseAnsi', () => {
  it('text with no escapes comes back as one part with no styling', () => {
    expect(parseAnsi('a plain line')).toEqual([
      { text: 'a plain line', color: null, bold: false, dim: false }
    ])
  })

  it('empty input gives no parts at all', () => {
    expect(parseAnsi('')).toEqual([])
  })

  it('the ordinary colours 30–37 become tokens', () => {
    expect(parseAnsi(`${ESC}[31mred`)).toEqual([
      { text: 'red', color: 'var(--ansi-red)', bold: false, dim: false }
    ])
    expect(parseAnsi(`${ESC}[36mcyan`)[0].color).toBe('var(--ansi-cyan)')
  })

  it('the bright colours 90–97 become their own tokens', () => {
    expect(parseAnsi(`${ESC}[92mbright`)[0].color).toBe('var(--ansi-bright-green)')
  })

  it('bold and dim accumulate', () => {
    const parts = parseAnsi(`${ESC}[1m${ESC}[2mdim bold`)
    expect(parts[0].bold).toBe(true)
    expect(parts[0].dim).toBe(true)
  })

  it('a 0 reset clears everything at once', () => {
    const parts = parseAnsi(`${ESC}[1;31mloud${ESC}[0mplain`)
    expect(parts[0]).toEqual({ text: 'loud', color: 'var(--ansi-red)', bold: true, dim: false })
    expect(parts[1]).toEqual({ text: 'plain', color: null, bold: false, dim: false })
  })

  it('22 clears bold and dim but not the colour', () => {
    const parts = parseAnsi(`${ESC}[1;31mloud${ESC}[22mthin`)
    expect(parts[1]).toEqual({ text: 'thin', color: 'var(--ansi-red)', bold: false, dim: false })
  })

  it('39 clears the colour but not the boldness', () => {
    const parts = parseAnsi(`${ESC}[1;31mloud${ESC}[39mcolourless`)
    expect(parts[1]).toEqual({ text: 'colourless', color: null, bold: true, dim: false })
  })

  it('an empty code reads as 0', () => {
    const parts = parseAnsi(`${ESC}[31mred${ESC}[mreset`)
    expect(parts[1].color).toBe(null)
  })

  it('a sequence at the end of a line does not produce an empty part', () => {
    expect(parseAnsi(`text${ESC}[0m`)).toEqual([
      { text: 'text', color: null, bold: false, dim: false }
    ])
  })
})
