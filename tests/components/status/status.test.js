import { describe, expect, it } from 'vitest'
import {
  RESERVED,
  STATUS_GLYPH,
  attentionLevel,
  hashStatus,
  normalizeStatus,
  statusCode,
  statusColors,
  statusSlot
} from '../../../src/components/status/status.js'

describe('normalizeStatus', () => {
  it('опускает регистр и обрезает края', () => {
    expect(normalizeStatus('  Needs You  ')).toBe('needs-you')
  })

  it('схлопывает любую серию не-буквенно-цифровых в один дефис', () => {
    expect(normalizeStatus('in___progress // now')).toBe('in-progress-now')
  })

  it('снимает ведущие и хвостовые дефисы', () => {
    expect(normalizeStatus('--ready--')).toBe('ready')
  })

  it('пустое и отсутствующее дают пустую строку', () => {
    expect(normalizeStatus('')).toBe('')
    expect(normalizeStatus(null)).toBe('')
    expect(normalizeStatus(undefined)).toBe('')
  })
})

describe('hashStatus', () => {
  /* Этот тест защищает не хеш, а пользователей: «безобидная» правка FNV-1a
     перекрасила бы разом все пользовательские статусы во всех проектах.
     Значения сняты с текущей реализации и меняться не должны. */
  it('стабилен на фиксированной выборке', () => {
    expect(hashStatus('awaiting-review')).toBe(2045313954)
    expect(statusSlot('awaiting-review')).toBe(6)
    expect(hashStatus('triage')).toBe(166983937)
    expect(statusSlot('triage')).toBe(1)
    expect(hashStatus('deploy')).toBe(1557350270)
    expect(statusSlot('deploy')).toBe(2)
    expect(hashStatus('needs-review')).toBe(1866091121)
    expect(statusSlot('needs-review')).toBe(5)
  })

  it('нормализация не влияет на хеш', () => {
    expect(hashStatus('awaiting-review')).toBe(hashStatus('Awaiting Review'))
    expect(statusSlot('awaiting-review')).toBe(statusSlot('  awaiting__review '))
  })

  it('слот всегда в пределах двенадцати', () => {
    const names = ['triage', 'awaiting-review', 'на-проверке', 'x', 'deploy', 'qa', 'спринт-3']
    for (const name of names) {
      const slot = statusSlot(name)
      expect(slot).toBeGreaterThanOrEqual(0)
      expect(slot).toBeLessThan(12)
      expect(Number.isInteger(slot)).toBe(true)
    }
  })
})

describe('statusColors', () => {
  it('зарезервированный получает свои токены и признак reserved', () => {
    expect(statusColors('needs-you')).toEqual({
      reserved: true,
      key: 'needs-you',
      fg: 'var(--status-needs-you-fg)',
      bg: 'var(--status-needs-you-bg)',
      border: 'var(--status-needs-you-border)'
    })
  })

  it('все шесть зарезервированных узнаются', () => {
    for (const name of RESERVED) {
      expect(statusColors(name).reserved).toBe(true)
      expect(STATUS_GLYPH[name]).toBeTruthy()
    }
  })

  it('пользовательский получает генерируемый слот', () => {
    const colors = statusColors('Awaiting Review')
    expect(colors.reserved).toBe(false)
    expect(colors.key).toBe('awaiting-review')
    expect(colors.fg).toBe(`var(--status-gen-${colors.slot}-fg)`)
    expect(colors.bg).toBe(`var(--status-gen-${colors.slot}-bg)`)
    expect(colors.border).toBe(`var(--status-gen-${colors.slot}-border)`)
  })
})

describe('statusCode', () => {
  it('из двух слов берёт по первой букве каждого', () => {
    expect(statusCode('awaiting-review')).toBe('AR')
  })

  it('из одного слова берёт две первые буквы', () => {
    expect(statusCode('triage')).toBe('TR')
  })

  it('из трёх слов берёт первые два', () => {
    expect(statusCode('waiting-for-review')).toBe('WF')
  })
})

describe('attentionLevel', () => {
  it('needs-you и failed кричат', () => {
    expect(attentionLevel('needs-you')).toBe('loud')
    expect(attentionLevel('failed')).toBe('loud')
  })

  it('running живой, done тихий', () => {
    expect(attentionLevel('running')).toBe('live')
    expect(attentionLevel('done')).toBe('quiet')
  })

  it('незнакомый статус живой, а не тихий: спрятать неизвестное хуже, чем показать', () => {
    expect(attentionLevel('awaiting-review')).toBe('live')
    expect(attentionLevel('')).toBe('live')
  })
})
