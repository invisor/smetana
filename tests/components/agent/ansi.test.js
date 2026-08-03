import { describe, expect, it } from 'vitest'
import { parseAnsi } from '../../../src/components/agent/ansi.js'

const ESC = '\u001b'

describe('parseAnsi', () => {
  it('текст без escape отдаётся одним куском без оформления', () => {
    expect(parseAnsi('обычная строка')).toEqual([
      { text: 'обычная строка', color: null, bold: false, dim: false }
    ])
  })

  it('пустой вход не даёт ни одного куска', () => {
    expect(parseAnsi('')).toEqual([])
  })

  it('обычные цвета 30–37 становятся токенами', () => {
    expect(parseAnsi(`${ESC}[31mкрасный`)).toEqual([
      { text: 'красный', color: 'var(--ansi-red)', bold: false, dim: false }
    ])
    expect(parseAnsi(`${ESC}[36mголубой`)[0].color).toBe('var(--ansi-cyan)')
  })

  it('яркие цвета 90–97 становятся своими токенами', () => {
    expect(parseAnsi(`${ESC}[92mяркий`)[0].color).toBe('var(--ansi-bright-green)')
  })

  it('bold и dim накапливаются', () => {
    const parts = parseAnsi(`${ESC}[1m${ESC}[2mтусклый жирный`)
    expect(parts[0].bold).toBe(true)
    expect(parts[0].dim).toBe(true)
  })

  it('сброс 0 снимает всё разом', () => {
    const parts = parseAnsi(`${ESC}[1;31mярко${ESC}[0mобычно`)
    expect(parts[0]).toEqual({ text: 'ярко', color: 'var(--ansi-red)', bold: true, dim: false })
    expect(parts[1]).toEqual({ text: 'обычно', color: null, bold: false, dim: false })
  })

  it('22 снимает жирность и тусклость, но не цвет', () => {
    const parts = parseAnsi(`${ESC}[1;31mярко${ESC}[22mтонко`)
    expect(parts[1]).toEqual({ text: 'тонко', color: 'var(--ansi-red)', bold: false, dim: false })
  })

  it('39 снимает цвет, но не жирность', () => {
    const parts = parseAnsi(`${ESC}[1;31mярко${ESC}[39mбесцветно`)
    expect(parts[1]).toEqual({ text: 'бесцветно', color: null, bold: true, dim: false })
  })

  it('пустой код читается как 0', () => {
    const parts = parseAnsi(`${ESC}[31mкрасный${ESC}[mсброшено`)
    expect(parts[1].color).toBe(null)
  })

  it('последовательность в конце строки не рождает пустой кусок', () => {
    expect(parseAnsi(`текст${ESC}[0m`)).toEqual([
      { text: 'текст', color: null, bold: false, dim: false }
    ])
  })
})
